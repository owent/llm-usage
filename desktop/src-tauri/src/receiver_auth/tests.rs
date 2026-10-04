use super::*;
use std::collections::BTreeMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};

#[derive(Default)]
pub(crate) struct MemoryStore {
    pub values: Mutex<BTreeMap<String, Vec<u8>>>,
    pub writes: AtomicUsize,
    random_counter: AtomicUsize,
}
impl Store for MemoryStore {
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.values.lock().unwrap().get(name).cloned())
    }
    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        self.values
            .lock()
            .unwrap()
            .insert(name.into(), bytes.to_vec());
        Ok(())
    }
    fn delete(&self, name: &str) -> Result<(), String> {
        self.values.lock().unwrap().remove(name);
        Ok(())
    }
    fn random(&self, bytes: &mut [u8]) -> Result<(), String> {
        let n = self.random_counter.fetch_add(1, Ordering::SeqCst) as u8;
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = (i as u8).wrapping_add(n);
        }
        Ok(())
    }
}
pub(crate) fn root(name: &str) -> std::path::PathBuf {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../build/plan-continuation/auth-tests")
        .join(format!(
            "{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&root).unwrap();
    root
}
#[test]
fn credentials_bind_app_source_config_and_route_and_revoke_without_restart() {
    let store = MemoryStore::default();
    let app = root("binding");
    let other = root("other-app");
    let config = app.join("claude/settings.json");
    let a = issue(&store, &app, &config, Family::Claude).unwrap();
    let b = issue(&store, &app, &app.join("codex/config.toml"), Family::Codex).unwrap();
    assert!(a.id != b.id && a.secret != b.secret);
    assert!(authorize(&store, &app, &a.header(), "/v1/logs") == Some(Family::Claude));
    assert!(authorize(&store, &other, &a.header(), "/v1/logs").is_none());
    assert!(authorize(&store, &app, &a.header(), "/v1/traces").is_none());
    assert!(authorize(&store, &app, &a.header(), "/v1/traces/supplemental").is_none());
    let mut wrong = a.header();
    wrong.pop();
    wrong.push(if a.secret.ends_with('a') { 'b' } else { 'a' });
    assert!(authorize(&store, &app, &wrong, "/v1/logs").is_none());
    for header in [
        "",
        "Bearer ../target",
        "Basic abc",
        "Bearer abc.def",
        "Bearer receiver-fixture",
    ] {
        assert!(authorize(&store, &app, header, "/v1/logs").is_none());
    }
    revoke(&store, &a).unwrap();
    assert!(authorize(&store, &app, &a.header(), "/v1/logs").is_none());
    assert!(authorize(&store, &app, &b.header(), "/v1/logs") == Some(Family::Codex));
    revoke(&store, &a).unwrap();
    revoke(&store, &b).unwrap();
    assert!(store.values.lock().unwrap().is_empty());
    std::fs::remove_dir_all(app).unwrap();
    std::fs::remove_dir_all(other).unwrap();
}
#[test]
fn unreadable_or_corrupt_credentials_fail_closed_and_cannot_delete_other_entries() {
    let store = MemoryStore::default();
    let app = root("corrupt");
    let a = issue(&store, &app, &app.join("settings.json"), Family::Claude).unwrap();
    store
        .values
        .lock()
        .unwrap()
        .insert(format!("{PREFIX}{}", a.id), b"invalid".to_vec());
    assert!(authorize(&store, &app, &a.header(), "/v1/logs").is_none());
    assert!(revoke(&store, &a).is_err());
    assert_eq!(store.values.lock().unwrap().len(), 1);
    std::fs::remove_dir_all(app).unwrap();
}
#[test]
fn family_filter_cannot_claim_other_client_logs() {
    assert!(Family::Claude.accepts_record(&serde_json::json!({"name":"claude_code.api_request"})));
    assert!(!Family::Claude.accepts_record(&serde_json::json!({"name":"codex.api_request"})));
    assert!(!Family::Codex.accepts_record(&serde_json::json!({"name":"unknown"})));
}
#[cfg(windows)]
#[test]
#[ignore = "native Windows credential store; run explicitly, creates and deletes one owned random entry"]
fn windows_credential_store_roundtrip() {
    let app = root("native-vault");
    let binding = issue(
        &SystemStore,
        &app,
        &app.join("synthetic/config.toml"),
        Family::Codex,
    )
    .unwrap();
    struct Cleanup(Binding);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = revoke(&SystemStore, &self.0);
        }
    }
    let cleanup = Cleanup(binding);
    // A new store value reads the persisted credential; no in-memory listener cache is involved.
    assert!(authorize(&SystemStore, &app, &cleanup.0.header(), "/v1/logs") == Some(Family::Codex));
    assert!(authorize(&SystemStore, &app, &cleanup.0.header(), "/v1/traces").is_none());
    revoke(&SystemStore, &cleanup.0).unwrap();
    assert!(authorize(&SystemStore, &app, &cleanup.0.header(), "/v1/logs").is_none());
    assert!(SystemStore
        .read(&format!("{PREFIX}{}", cleanup.0.id))
        .unwrap()
        .is_none());
    drop(cleanup);
    std::fs::remove_dir_all(app).unwrap();
}
