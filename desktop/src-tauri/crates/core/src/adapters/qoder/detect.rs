//! Recognize candidate Qoder files; scanning verifies the CLI envelope and each record version.

use crate::adapters::framework::DetectOutcome;
use crate::adapters::jsonl::FileProbe;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::time::{Duration, Instant};
use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
};

pub const QODER_FORMAT: &str = "qoder-session-jsonl";

const DETECT_HEAD_BYTES: usize = 64 * 1024;
const CACHE_LIMIT: usize = 4096;

struct CachedProbe {
    fingerprint: FileProbe,
    native: bool,
    used: Instant,
    probe_id: u64,
}

#[derive(Default)]
struct ProbeState {
    latest: u64,
    active: usize,
}

struct ActiveProbe {
    key: String,
    id: u64,
}

impl Drop for ActiveProbe {
    fn drop(&mut self) {
        let mut states = probe_ids().lock().unwrap();
        if let Some(state) = states.get_mut(&self.key) {
            state.active -= 1;
            if state.active == 0 {
                states.remove(&self.key);
            }
        }
    }
}

fn probe_ids() -> &'static Mutex<BTreeMap<String, ProbeState>> {
    static IDS: OnceLock<Mutex<BTreeMap<String, ProbeState>>> = OnceLock::new();
    IDS.get_or_init(Default::default)
}

fn next_probe(key: &str) -> ActiveProbe {
    static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut states = probe_ids().lock().unwrap();
    let state = states.entry(key.to_string()).or_default();
    state.latest = state.latest.max(id);
    state.active += 1;
    ActiveProbe {
        key: key.to_string(),
        id,
    }
}

fn latest_probe_id(probe: &ActiveProbe) -> u64 {
    probe_ids()
        .lock()
        .unwrap()
        .get(&probe.key)
        .map_or(probe.id, |state| state.latest)
}

fn cache() -> &'static Mutex<BTreeMap<String, CachedProbe>> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, CachedProbe>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

#[cfg(test)]
thread_local! {
    static PROBE_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PROBE_HOOK: std::cell::RefCell<Option<Box<dyn Fn()>>> = const { std::cell::RefCell::new(None) };
    static CACHE_WRITE_HOOK: std::cell::RefCell<Option<Box<dyn Fn()>>> = const { std::cell::RefCell::new(None) };
}

pub(crate) fn native_record(path: &Path) -> bool {
    if path.extension().is_none_or(|e| e != "jsonl") {
        return false;
    }
    let key = crate::adapters::framework::normalize_path(path);
    let Ok(fingerprint) = super::super::jsonl::probe_file(path) else {
        return false;
    };
    if let Some(cached) = cache().lock().unwrap().get_mut(&key) {
        if cached.fingerprint == fingerprint {
            cached.used = Instant::now();
            return cached.native;
        }
    }
    let probe = next_probe(&key);
    let Ok((native, complete)) = probe_native(path) else {
        return false;
    };
    // Do not cache interrupted/unstable reads or retain the cache lock during source I/O.
    let stable = complete
        && super::super::run_policy::check().is_ok()
        && super::super::jsonl::probe_file(path).ok() == Some(fingerprint);
    #[cfg(test)]
    CACHE_WRITE_HOOK.with(|hook| {
        if let Some(hook) = &*hook.borrow() {
            hook();
        }
    });
    let mut entries = cache().lock().unwrap();
    if let Some(cached) = entries
        .get(&key)
        .filter(|cached| cached.probe_id > probe.id)
    {
        return cached.native;
    }
    if latest_probe_id(&probe) > probe.id {
        drop(entries);
        let Ok((native, complete)) = probe_native(path) else {
            return false;
        };
        // A newer probe may publish while this uncached retry reads the source.
        return cache()
            .lock()
            .unwrap()
            .get(&key)
            .filter(|cached| cached.probe_id > probe.id)
            .map_or(complete && native, |cached| cached.native);
    }
    if !stable {
        return false;
    }
    if entries.len() >= CACHE_LIMIT && !entries.contains_key(&key) {
        if let Some(oldest) = entries
            .iter()
            .min_by_key(|(_, value)| value.used)
            .map(|(key, _)| key.clone())
        {
            entries.remove(&oldest);
        }
    }
    entries.insert(
        key,
        CachedProbe {
            fingerprint,
            native,
            used: Instant::now(),
            probe_id: probe.id,
        },
    );
    native
}

fn probe_native(path: &Path) -> std::io::Result<(bool, bool)> {
    #[cfg(test)]
    {
        PROBE_COUNT.with(|count| count.set(count.get() + 1));
        PROBE_HOOK.with(|hook| {
            if let Some(hook) = &*hook.borrow() {
                hook();
            }
        });
    }
    let file = super::super::run_policy::checked_file(path)?;
    // User/tool content may precede the first identifying assistant. Inspect complete
    // lines within the normal 32 MiB window, retaining at most one 8 MiB line.
    let mut reader = BufReader::with_capacity(64 * 1024, file.take(32 * 1024 * 1024));
    let deadline = Instant::now() + Duration::from_millis(200);
    let mut bytes = Vec::new();
    let mut first = true;
    while Instant::now() < deadline {
        bytes.clear();
        (&mut reader)
            .take(super::super::jsonl::DEFAULT_MAX_LINE_BYTES as u64 + 1)
            .read_until(b'\n', &mut bytes)?;
        if bytes.len() > super::super::jsonl::DEFAULT_MAX_LINE_BYTES {
            return Ok((false, true));
        }
        if bytes.is_empty() {
            return Ok((false, true));
        }
        if bytes.last() != Some(&b'\n') {
            return Ok((false, false));
        }
        let line = if first {
            bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes)
        } else {
            &bytes
        };
        first = false;
        let Ok(row) = serde_json::from_slice::<serde_json::Value>(line) else {
            continue;
        };
        if row["type"] == "assistant"
            && row["entrypoint"] == "cli"
            && row["userType"] == "external"
            && row["isSidechain"].is_boolean()
            && row["message"]["role"] == "assistant"
            && ["sessionId", "uuid", "version", "cwd"]
                .iter()
                .all(|k| row[*k].as_str().is_some_and(|v| !v.trim().is_empty()))
            && row["message"]["id"]
                .as_str()
                .is_some_and(|v| !v.trim().is_empty())
            && (row["modelSource"] == "custom"
                || row["message"]["usage"]["billable"].is_boolean()
                || ["credits", "original_credits"].iter().any(|key| {
                    row["message"]["usage"][*key]
                        .as_f64()
                        .is_some_and(f64::is_finite)
                }))
        {
            return Ok((true, true));
        }
    }
    Ok((false, false))
}

pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    // Temporary lock/timeout/removal after enumeration is Pending; retry detection next run.
    let Some(head) = crate::adapters::framework::read_detect_head(path, DETECT_HEAD_BYTES)? else {
        return Ok(DetectOutcome::Pending);
    };
    let text = String::from_utf8_lossy(&head);
    let text = text.trim_start_matches('\u{feff}').trim_start();
    if text.trim().is_empty() {
        return Ok(DetectOutcome::Pending);
    }
    // Recognize JSONL sessions or state.json objects as Qoder session files,
    // and let the scanner validate each native CLI record independently.
    // A session_id substring match also requires an opening brace; otherwise unrelated
    // files containing "session_id" could be incorrectly claimed.
    let looks_jsonl = text.starts_with('{')
        && (text.contains("\"sessionId\"") || text.contains("\"session_id\""));
    let looks_state =
        text.contains("\"modelRequests\"") || text.contains("\"compact_token_usage_json\"");
    if looks_jsonl || looks_state || native_record(path) {
        Ok(DetectOutcome::Supported {
            format: QODER_FORMAT.to_string(),
            format_version: None,
            basis: VersionBasis::LatestFallback,
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "missing Qoder session fingerprint".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::framework::*;
    use crate::jobs::{RunStatus, TriggerKind};
    use crate::storage::Storage;
    use std::{
        cell::{Cell, RefCell},
        path::{Path, PathBuf},
        rc::Rc,
        sync::{Arc, Barrier, MutexGuard},
    };

    static CACHE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct CacheIsolation {
        _guard: MutexGuard<'static, ()>,
    }

    impl CacheIsolation {
        fn new() -> Self {
            let guard = CACHE_TEST_LOCK
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            cache().lock().unwrap().clear();
            PROBE_COUNT.with(|count| count.set(0));
            PROBE_HOOK.with(|hook| *hook.borrow_mut() = None);
            CACHE_WRITE_HOOK.with(|hook| *hook.borrow_mut() = None);
            Self { _guard: guard }
        }
    }

    impl Drop for CacheIsolation {
        fn drop(&mut self) {
            PROBE_HOOK.with(|hook| *hook.borrow_mut() = None);
            CACHE_WRITE_HOOK.with(|hook| *hook.borrow_mut() = None);
            cache().lock().unwrap().clear();
        }
    }

    struct Files(PathBuf);
    impl Files {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../../build/qoder-detect")
                .join(format!(
                    "{}-{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos(),
                    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                ));
            std::fs::create_dir_all(root.join("projects/project")).unwrap();
            Self(root)
        }
        fn transcript(&self) -> PathBuf {
            self.0.join("projects/project/session.jsonl")
        }
    }
    impl Drop for Files {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn fixture() -> String {
        include_str!("../../../tests/fixtures/qoder/source-cli-1.1.76/session.jsonl").into()
    }

    fn cached(path: &Path) -> Option<(FileProbe, bool)> {
        let key = crate::adapters::framework::normalize_path(path);
        cache()
            .lock()
            .unwrap()
            .get(&key)
            .map(|entry| (entry.fingerprint, entry.native))
    }

    #[test]
    fn native_probe_caches_negative_results_and_invalidates_same_size_tail_replacement() {
        let _cache = CacheIsolation::new();
        let files = Files::new();
        let file = files.transcript();
        let prefix = format!(
            "{{\"type\":\"user\",\"text\":\"{}\"}}\n",
            "x".repeat(96 * 1024)
        );
        let native = fixture();
        let foreign = native.replace("\"entrypoint\":\"cli\"", "\"entrypoint\":\"ide\"");
        std::fs::write(&file, prefix.clone() + &foreign).unwrap();
        let before = super::super::super::jsonl::probe_file(&file).unwrap();
        let modified = std::fs::metadata(&file).unwrap().modified().unwrap();
        for _ in 0..8 {
            assert!(!native_record(&file));
        }
        PROBE_COUNT.with(|count| {
            assert_eq!(
                count.get(),
                1,
                "unchanged foreign files must not repeat deep detection"
            )
        });
        std::fs::write(&file, prefix + &native).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        let after = super::super::super::jsonl::probe_file(&file).unwrap();
        assert_eq!(
            (before.len, before.mtime_ms, before.head_hash),
            (after.len, after.mtime_ms, after.head_hash)
        );
        assert_ne!(before.tail_hash, after.tail_hash);
        assert!(native_record(&file));
        assert!(native_record(&file));
        PROBE_COUNT.with(|count| assert_eq!(count.get(), 2));
        std::fs::write(&file, "{}\n").unwrap();
        assert!(!native_record(&file));
        PROBE_COUNT.with(|count| assert_eq!(count.get(), 3));
    }

    #[test]
    fn cancelled_probe_does_not_cache_a_negative_classification() {
        let _cache = CacheIsolation::new();
        let files = Files::new();
        let file = files.transcript();
        std::fs::write(&file, fixture()).unwrap();
        {
            let _cancelled =
                super::super::super::run_policy::enter(None, Some(std::sync::Arc::new(|| false)));
            assert!(!native_record(&file));
        }
        assert!(native_record(&file));
        PROBE_COUNT.with(|count| assert_eq!(count.get(), 1));
    }

    #[test]
    fn cancellation_during_deep_probe_does_not_cache_a_negative_classification() {
        let _cache = CacheIsolation::new();
        let files = Files::new();
        let file = files.transcript();
        std::fs::write(&file, fixture()).unwrap();
        let cancelled = Rc::new(RefCell::new(None));
        PROBE_HOOK.with(|hook| {
            let cancelled = cancelled.clone();
            *hook.borrow_mut() = Some(Box::new(move || {
                *cancelled.borrow_mut() = Some(super::super::super::run_policy::enter(
                    None,
                    Some(Arc::new(|| false)),
                ));
            }));
        });
        assert!(!native_record(&file));
        PROBE_COUNT.with(|count| assert_eq!(count.get(), 1));
        assert!(cached(&file).is_none());
        PROBE_HOOK.with(|hook| *hook.borrow_mut() = None);
        cancelled.borrow_mut().take();
        assert!(native_record(&file));
        assert!(native_record(&file));
        PROBE_COUNT.with(|count| assert_eq!(count.get(), 2));
    }

    #[test]
    fn unterminated_tail_does_not_cache_a_negative_classification() {
        let _cache = CacheIsolation::new();
        let files = Files::new();
        let file = files.transcript();
        let foreign = fixture().replace("\"entrypoint\":\"cli\"", "\"entrypoint\":\"ide\"");
        let mut incomplete = foreign.as_bytes().to_vec();
        assert_eq!(incomplete.pop(), Some(b'\n'));
        std::fs::write(&file, incomplete).unwrap();
        assert!(!native_record(&file));
        assert!(cached(&file).is_none());
        assert!(!native_record(&file));
        PROBE_COUNT.with(|count| assert_eq!(count.get(), 2));
        std::fs::write(&file, foreign).unwrap();
        assert!(!native_record(&file));
        assert_eq!(cached(&file).map(|entry| entry.1), Some(false));
        assert!(!native_record(&file));
        PROBE_COUNT.with(|count| assert_eq!(count.get(), 3));
    }

    #[test]
    fn replacement_during_probe_does_not_cache_negative_for_the_old_fingerprint() {
        let _cache = CacheIsolation::new();
        let files = Files::new();
        let file = files.transcript();
        let native = fixture();
        let foreign = native.replace("\"entrypoint\":\"cli\"", "\"entrypoint\":\"ide\"");
        std::fs::write(&file, &native).unwrap();
        let before = super::super::super::jsonl::probe_file(&file).unwrap();
        let modified = std::fs::metadata(&file).unwrap().modified().unwrap();
        PROBE_HOOK.with(|hook| {
            let file = file.clone();
            let foreign = foreign.clone();
            *hook.borrow_mut() = Some(Box::new(move || {
                std::fs::write(&file, &foreign).unwrap();
            }));
        });
        assert!(!native_record(&file));
        assert!(cached(&file).is_none());
        PROBE_HOOK.with(|hook| *hook.borrow_mut() = None);
        std::fs::write(&file, native).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        assert_eq!(
            super::super::super::jsonl::probe_file(&file).unwrap(),
            before
        );
        assert!(native_record(&file));
        assert!(native_record(&file));
        PROBE_COUNT.with(|count| assert_eq!(count.get(), 2));
    }

    #[test]
    fn older_concurrent_probe_uses_newer_classification_without_overwriting_cache() {
        let _cache = CacheIsolation::new();
        let files = Files::new();
        let file = files.transcript();
        let native = fixture();
        let foreign = native.replace("\"entrypoint\":\"cli\"", "\"entrypoint\":\"ide\"");
        std::fs::write(&file, native).unwrap();
        let writing = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let worker = {
            let file = file.clone();
            let writing = writing.clone();
            let release = release.clone();
            std::thread::spawn(move || {
                CACHE_WRITE_HOOK.with(|hook| {
                    *hook.borrow_mut() = Some(Box::new(move || {
                        writing.wait();
                        release.wait();
                    }));
                });
                native_record(&file)
            })
        };
        writing.wait();
        std::fs::write(&file, foreign).unwrap();
        assert!(!native_record(&file));
        let current = super::super::super::jsonl::probe_file(&file).unwrap();
        release.wait();
        assert!(!worker.join().unwrap());
        let cached = cached(&file).expect("the newer complete probe must remain cached");
        assert_eq!(cached.0, current);
        assert!(!cached.1);
        assert!(!native_record(&file));
    }

    #[test]
    fn unrelated_probe_does_not_repeat_an_older_stable_probe() {
        let _cache = CacheIsolation::new();
        let files = Files::new();
        let file = files.transcript();
        let other = files.0.join("projects/project/other.jsonl");
        std::fs::write(&file, fixture()).unwrap();
        std::fs::write(&other, "{}\n").unwrap();
        let writing = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let worker = {
            let file = file.clone();
            let writing = writing.clone();
            let release = release.clone();
            std::thread::spawn(move || {
                CACHE_WRITE_HOOK.with(|hook| {
                    *hook.borrow_mut() = Some(Box::new(move || {
                        writing.wait();
                        release.wait();
                    }));
                });
                let native = native_record(&file);
                let count = PROBE_COUNT.with(|count| count.get());
                (native, count)
            })
        };
        writing.wait();
        assert!(!native_record(&other));
        assert!(cached(&other).is_some());
        release.wait();
        assert_eq!(worker.join().unwrap(), (true, 1));
        assert!(native_record(&file));
        assert!(!native_record(&other));
    }

    #[test]
    fn older_probe_cannot_override_a_newer_unstable_result() {
        let _cache = CacheIsolation::new();
        let files = Files::new();
        let file = files.transcript();
        let native = fixture();
        let mut foreign = native
            .replace("\"entrypoint\":\"cli\"", "\"entrypoint\":\"ide\"")
            .into_bytes();
        assert_eq!(foreign.pop(), Some(b'\n'));
        std::fs::write(&file, native).unwrap();
        let writing = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let worker = {
            let file = file.clone();
            let writing = writing.clone();
            let release = release.clone();
            std::thread::spawn(move || {
                CACHE_WRITE_HOOK.with(|hook| {
                    *hook.borrow_mut() = Some(Box::new(move || {
                        writing.wait();
                        release.wait();
                    }));
                });
                native_record(&file)
            })
        };
        writing.wait();
        std::fs::write(&file, foreign).unwrap();
        assert!(!native_record(&file));
        assert!(cached(&file).is_none());
        release.wait();
        assert!(!worker.join().unwrap());
        assert!(cached(&file).is_none());
        assert!(!native_record(&file));
    }

    fn superseded_probe_with_retry_hook(
        file: &Path,
        on_retry: impl Fn(&Path) + Send + 'static,
    ) -> (bool, usize, Option<bool>) {
        std::fs::write(file, fixture()).unwrap();
        let writing = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let worker = {
            let file = file.to_path_buf();
            let writing = writing.clone();
            let release = release.clone();
            std::thread::spawn(move || {
                CACHE_WRITE_HOOK.with(|hook| {
                    *hook.borrow_mut() = Some(Box::new(move || {
                        writing.wait();
                        release.wait();
                    }));
                });
                let unlocked = Rc::new(Cell::new(None));
                PROBE_HOOK.with(|hook| {
                    let unlocked = unlocked.clone();
                    let file = file.clone();
                    *hook.borrow_mut() = Some(Box::new(move || {
                        if PROBE_COUNT.with(|count| count.get()) == 2 {
                            let available = cache().try_lock().is_ok();
                            unlocked.set(Some(available));
                            // Leave a lock regression observable without deadlocking or poisoning it.
                            if available {
                                CACHE_WRITE_HOOK.with(|hook| *hook.borrow_mut() = None);
                                on_retry(&file);
                            }
                        }
                    }));
                });
                let native = native_record(&file);
                (
                    native,
                    PROBE_COUNT.with(|count| count.get()),
                    unlocked.get(),
                )
            })
        };
        writing.wait();
        let foreign = fixture().replace("\"entrypoint\":\"cli\"", "\"entrypoint\":\"ide\"");
        std::fs::write(file, foreign.trim_end_matches('\n')).unwrap();
        let newer = native_record(file);
        release.wait();
        let result = worker.join().unwrap();
        assert!(!newer);
        assert!(probe_ids().lock().unwrap().is_empty());
        result
    }

    #[test]
    fn superseded_retry_allows_unrelated_detection_without_caching_its_own_result() {
        let _cache = CacheIsolation::new();
        let files = Files::new();
        let file = files.transcript();
        let other = files.0.join("projects/project/other.jsonl");
        std::fs::write(&other, "{}\n").unwrap();
        let result = superseded_probe_with_retry_hook(&file, {
            let other = other.clone();
            move |file| {
                assert!(!native_record(&other));
                std::fs::write(file, fixture()).unwrap();
            }
        });
        assert_eq!(result, (true, 3, Some(true)));
        assert!(cached(&file).is_none());
        assert_eq!(cached(&other).map(|entry| entry.1), Some(false));
        assert!(native_record(&file));
        assert_eq!(cached(&file).map(|entry| entry.1), Some(true));
    }

    #[test]
    fn superseded_retry_prefers_a_newer_cache_entry_written_during_its_read() {
        let _cache = CacheIsolation::new();
        let files = Files::new();
        let file = files.transcript();
        let result = superseded_probe_with_retry_hook(&file, |file| {
            std::fs::write(file, fixture()).unwrap();
            assert!(native_record(file));
            let foreign = fixture().replace("\"entrypoint\":\"cli\"", "\"entrypoint\":\"ide\"");
            std::fs::write(file, foreign).unwrap();
        });
        assert_eq!(result, (true, 3, Some(true)));
        assert_eq!(cached(&file).map(|entry| entry.1), Some(true));
        assert!(!native_record(&file));
        assert_eq!(cached(&file).map(|entry| entry.1), Some(false));
    }

    #[test]
    fn superseded_retry_read_error_leaves_no_cache_or_active_probe() {
        let _cache = CacheIsolation::new();
        let files = Files::new();
        let file = files.transcript();
        let result = superseded_probe_with_retry_hook(&file, |file| {
            std::fs::remove_file(file).unwrap();
        });
        assert_eq!(result, (false, 2, Some(true)));
        assert!(cached(&file).is_none());
        std::fs::write(&file, fixture()).unwrap();
        assert!(native_record(&file));
    }

    #[test]
    fn lru_evicts_least_recently_used_at_capacity_and_reprobes_the_evicted_entry() {
        let _cache = CacheIsolation::new();
        let files = Files::new();
        let paths: Vec<PathBuf> = (0..=CACHE_LIMIT)
            .map(|index| {
                files
                    .0
                    .join(format!("projects/project/session-{index:05}.jsonl"))
            })
            .collect();
        for path in paths.iter().take(CACHE_LIMIT) {
            std::fs::write(path, "{}\n").unwrap();
            assert!(!native_record(path));
        }
        PROBE_COUNT.with(|count| assert_eq!(count.get(), CACHE_LIMIT));
        assert!(!native_record(&paths[0]));
        PROBE_COUNT.with(|count| assert_eq!(count.get(), CACHE_LIMIT));
        std::fs::write(&paths[CACHE_LIMIT], "{}\n").unwrap();
        assert!(!native_record(&paths[CACHE_LIMIT]));
        PROBE_COUNT.with(|count| assert_eq!(count.get(), CACHE_LIMIT + 1));
        assert_eq!(cache().lock().unwrap().len(), CACHE_LIMIT);
        assert!(cached(&paths[0]).is_some());
        assert!(cached(&paths[1]).is_none());
        assert!(cached(&paths[CACHE_LIMIT]).is_some());
        assert!(!native_record(&paths[1]));
        PROBE_COUNT.with(|count| assert_eq!(count.get(), CACHE_LIMIT + 2));
        assert!(!native_record(&paths[0]));
        PROBE_COUNT.with(|count| assert_eq!(count.get(), CACHE_LIMIT + 2));
        assert_eq!(cache().lock().unwrap().len(), CACHE_LIMIT);
        assert!(probe_ids().lock().unwrap().is_empty());
    }

    #[test]
    fn manual_routing_and_incremental_windows_probe_once_outside_the_writer() {
        let _cache = CacheIsolation::new();
        struct AuditedStorage {
            storage: Storage,
            held: Rc<Cell<bool>>,
        }
        impl StorageAccess for AuditedStorage {
            fn with_storage<T>(
                &self,
                operation: impl FnOnce(&Storage) -> Result<T, CoreError>,
            ) -> Result<T, CoreError> {
                self.held.set(true);
                let result = operation(&self.storage);
                self.held.set(false);
                result
            }
        }
        let files = Files::new();
        std::fs::write(files.transcript(), fixture()).unwrap();
        let held = Rc::new(Cell::new(false));
        PROBE_HOOK.with(|hook| {
            let held = held.clone();
            *hook.borrow_mut() = Some(Box::new(move || {
                assert!(
                    !held.get(),
                    "deep source detection must not hold the writer"
                )
            }));
        });
        let access = AuditedStorage {
            storage: Storage::open(&files.0.join("test.db")).unwrap(),
            held,
        };
        let ctx = DiscoverContext {
            manual_roots: vec![files.0.clone()],
            ..Default::default()
        };
        for now in 0..8 {
            let mut limits = ScanLimits::default();
            limits.jsonl.max_lines = Some(1);
            let reports = run_adapter_scan_filtered_access(
                &access,
                &super::super::QoderAdapter::new(),
                &ctx,
                &RunConfig {
                    timezone: "UTC".into(),
                    now_ms: 1_800_000_000_000 + now,
                    limits,
                    trigger: TriggerKind::Manual,
                    run_id_prefix: format!("cache-{now}"),
                    origin_host_id: None,
                },
                &InstanceFilter::default(),
                &RunControl {
                    deadline: None,
                    allowed: &|| true,
                },
            )
            .unwrap();
            assert!(
                reports
                    .iter()
                    .all(|report| report.finish != RunStatus::Failed),
                "{reports:?}"
            );
        }
        PROBE_COUNT.with(|count| {
            assert_eq!(
                count.get(),
                1,
                "routing, detection and bounded parser windows share the fingerprint cache"
            )
        });
        let calls: i64 = access
            .storage
            .conn()
            .query_row("SELECT COUNT(*) FROM usage_events", [], |row| row.get(0))
            .unwrap();
        assert!(calls > 0);
    }
}
