//! Optional Windows notifications for registered enabled local sources.
use llm_usage_core::{error::CoreError, storage::Storage};
use std::collections::BTreeSet;

#[derive(Default)]
pub struct FileWatch {
    #[cfg(windows)]
    handles: std::collections::BTreeMap<String, Notification>,
    #[cfg(windows)]
    failed: std::collections::BTreeMap<String, i64>,
    pending: BTreeSet<String>,
    quiet_since: Option<i64>,
}
impl FileWatch {
    fn debounce(&mut self, changed: BTreeSet<String>, now: i64) -> BTreeSet<String> {
        if !changed.is_empty() {
            self.pending.extend(changed);
            self.quiet_since = Some(now);
        }
        if self
            .quiet_since
            .is_some_and(|at| now.saturating_sub(at) >= 2000)
        {
            self.quiet_since = None;
            std::mem::take(&mut self.pending)
        } else {
            BTreeSet::new()
        }
    }
    pub fn tick(
        &mut self,
        storage: &Storage,
        enabled: bool,
        manual_roots: Option<&[String]>,
        now: i64,
    ) -> Result<BTreeSet<String>, CoreError> {
        if !enabled {
            *self = Self::default();
            return Ok(BTreeSet::new());
        }
        #[cfg(windows)]
        {
            use std::{collections::BTreeMap, path::Path};
            let mut roots: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
            let mut stmt=storage.conn().prepare("SELECT s.instance_id,s.location_hint FROM source_instances s LEFT JOIN extraction_schedules e ON e.instance_id=s.instance_id AND e.scope='source' AND e.enabled=1 WHERE s.enabled=1 AND s.locality_basis='local_filesystem' AND s.attribution_status='verified' AND s.location_hint IS NOT NULL AND (e.rule_kind IS NULL OR e.rule_kind='interval')")?;
            for record in
                stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            {
                let (instance, location) = record?;
                let location = location.replace('\\', "/");
                if location.starts_with("//") {
                    continue;
                }
                if manual_roots.is_some_and(|allowed| {
                    !allowed.iter().any(|raw| {
                        let root = raw.replace('\\', "/").trim_end_matches('/').to_lowercase();
                        let candidate = location.to_lowercase();
                        candidate == root || candidate.starts_with(&(root + "/"))
                    })
                }) {
                    continue;
                }
                let path = Path::new(&location);
                let directory = if path.is_file() {
                    path.parent()
                } else {
                    Some(path)
                };
                let Some(directory) = directory.filter(|p| p.is_dir()) else {
                    continue;
                };
                let key = directory
                    .to_string_lossy()
                    .replace('\\', "/")
                    .to_lowercase();
                roots.entry(key).or_default().insert(instance);
            }
            self.handles.retain(|path, _| roots.contains_key(path));
            self.failed.retain(|path, _| roots.contains_key(path));
            for path in roots.keys() {
                if self.handles.contains_key(path)
                    || self.handles.len() >= 32
                    || self
                        .failed
                        .get(path)
                        .is_some_and(|last| now.saturating_sub(*last) < 30000)
                {
                    continue;
                }
                match Notification::open(path) {
                    Ok(handle) => {
                        self.handles.insert(path.clone(), handle);
                        self.failed.remove(path);
                    }
                    Err(_) => {
                        self.failed.insert(path.clone(), now);
                        let _=storage.conn().execute("INSERT INTO diagnostics(code,message,created_ms) VALUES('file_watch_unavailable','directory notification unavailable; interval polling remains active',?1)",[crate::scanner::now_ms()]);
                    }
                }
            }
            let mut changed = BTreeSet::new();
            let mut broken = Vec::new();
            for (path, handle) in &self.handles {
                match handle.changed() {
                    Ok(true) => changed.extend(roots[path].iter().cloned()),
                    Ok(false) => {}
                    Err(_) => broken.push(path.clone()),
                }
            }
            for path in broken {
                self.handles.remove(&path);
                self.failed.insert(path, now);
            }
            // Drop queued instances when a source is disabled or changes to a
            // calendar rule during the quiet period.
            let eligible: BTreeSet<_> = roots.values().flatten().cloned().collect();
            self.pending.retain(|id| eligible.contains(id));
            Ok(self.debounce(changed, now))
        }
        #[cfg(not(windows))]
        {
            let _ = (storage, manual_roots);
            Ok(self.debounce(BTreeSet::new(), now))
        }
    }
}
#[cfg(windows)]
struct Notification(windows::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl Notification {
    fn open(path: &str) -> windows::core::Result<Self> {
        use windows::{core::PCWSTR, Win32::Storage::FileSystem::*};
        let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
        unsafe {
            FindFirstChangeNotificationW(
                PCWSTR(wide.as_ptr()),
                true,
                FILE_NOTIFY_CHANGE_FILE_NAME
                    | FILE_NOTIFY_CHANGE_DIR_NAME
                    | FILE_NOTIFY_CHANGE_SIZE
                    | FILE_NOTIFY_CHANGE_LAST_WRITE,
            )
            .map(Self)
        }
    }
    fn changed(&self) -> windows::core::Result<bool> {
        use windows::Win32::{
            Foundation::{WAIT_OBJECT_0, WAIT_TIMEOUT},
            Storage::FileSystem::FindNextChangeNotification,
            System::Threading::WaitForSingleObject,
        };
        unsafe {
            match WaitForSingleObject(self.0, 0) {
                WAIT_OBJECT_0 => {
                    FindNextChangeNotification(self.0)?;
                    Ok(true)
                }
                WAIT_TIMEOUT => Ok(false),
                _ => Err(windows::core::Error::from_thread()),
            }
        }
    }
}
#[cfg(windows)]
impl Drop for Notification {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Storage::FileSystem::FindCloseChangeNotification(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_changes_merge_after_two_quiet_seconds() {
        let mut watch = FileWatch::default();
        assert!(watch.debounce(BTreeSet::from(["a".into()]), 0).is_empty());
        assert!(watch
            .debounce(BTreeSet::from(["a".into(), "b".into()]), 1500)
            .is_empty());
        assert!(watch.debounce(BTreeSet::new(), 3499).is_empty());
        assert_eq!(
            watch.debounce(BTreeSet::new(), 3500),
            BTreeSet::from(["a".into(), "b".into()])
        );
        assert!(watch.debounce(BTreeSet::new(), 6000).is_empty());
    }
    #[cfg(windows)]
    #[test]
    fn native_notification_observes_a_flushed_file_and_releases_on_pause() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/plan-execution/file-watch")
            .join(format!("{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let storage = Storage::open(&root.join("test.sqlite")).unwrap();
        let source = root.join("source");
        std::fs::create_dir_all(&source).unwrap();
        storage.conn().execute("INSERT OR REPLACE INTO source_instances(instance_id,agent,locality_basis,attribution_status,enabled,health,location_hint,created_at_ms,updated_at_ms) VALUES('a','codex','local_filesystem','verified',1,'ok',?1,0,0)",[source.to_string_lossy().as_ref()]).unwrap();
        let mut watch = FileWatch::default();
        assert!(watch.tick(&storage, true, None, 0).unwrap().is_empty());
        assert_eq!(watch.handles.len(), 1);
        std::fs::write(source.join("changed.jsonl"), b"{}\n").unwrap();
        let clock = std::time::Instant::now();
        let mut ready = BTreeSet::new();
        for _ in 0..500 {
            ready = watch
                .tick(&storage, true, None, clock.elapsed().as_millis() as i64)
                .unwrap();
            if !ready.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(ready, BTreeSet::from(["a".into()]));
        watch.tick(&storage, false, None, 2101).unwrap();
        assert!(watch.handles.is_empty());
        assert!(watch.pending.is_empty());
        let excluded = vec![root.join("other").to_string_lossy().into_owned()];
        watch.tick(&storage, true, Some(&excluded), 2200).unwrap();
        assert!(watch.handles.is_empty());
    }
}
