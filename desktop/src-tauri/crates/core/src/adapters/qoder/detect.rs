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
}

fn cache() -> &'static Mutex<BTreeMap<String, CachedProbe>> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, CachedProbe>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

#[cfg(test)]
thread_local! {
    static PROBE_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PROBE_HOOK: std::cell::RefCell<Option<Box<dyn Fn()>>> = const { std::cell::RefCell::new(None) };
}

pub(crate) fn native_record(path: &Path) -> bool {
    if path.extension().map_or(true, |e| e != "jsonl") {
        return false;
    }
    let Ok(fingerprint) = super::super::jsonl::probe_file(path) else {
        return false;
    };
    let key = crate::adapters::framework::normalize_path(path);
    if let Some(cached) = cache().lock().unwrap().get_mut(&key) {
        if cached.fingerprint == fingerprint {
            cached.used = Instant::now();
            return cached.native;
        }
    }
    let Ok((native, complete)) = probe_native(path) else {
        return false;
    };
    // Do not cache interrupted/unstable reads or retain the cache lock during source I/O.
    if complete
        && super::super::run_policy::check().is_ok()
        && super::super::jsonl::probe_file(path).ok() == Some(fingerprint)
    {
        let mut entries = cache().lock().unwrap();
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
            },
        );
    }
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
        if bytes.len() > super::super::jsonl::DEFAULT_MAX_LINE_BYTES || bytes.last() != Some(&b'\n')
        {
            return Ok((false, true));
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
    use std::{cell::Cell, path::PathBuf, rc::Rc};

    struct Files(PathBuf);
    impl Files {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "qoder-routing-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir_all(root.join("projects/project")).unwrap();
            PROBE_COUNT.with(|count| count.set(0));
            Self(root)
        }
        fn transcript(&self) -> PathBuf {
            self.0.join("projects/project/session.jsonl")
        }
    }
    impl Drop for Files {
        fn drop(&mut self) {
            PROBE_HOOK.with(|hook| *hook.borrow_mut() = None);
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn fixture() -> String {
        include_str!("../../../tests/fixtures/qoder/source-cli-1.1.76/session.jsonl").into()
    }

    #[test]
    fn native_probe_caches_negative_results_and_invalidates_same_size_tail_replacement() {
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
    fn manual_routing_and_incremental_windows_probe_once_outside_the_writer() {
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
