mod common;
use common::*;
use llm_usage_core::jobs::RunStatus;

#[test]
fn failed_same_size_replacement_keeps_probe_generation_cursor_and_replays_after_recovery() {
    fn contents(response: &str) -> Vec<u8> {
        let original =
            reconstruct_codex_jsonl(&codex_fixture("rollout-single-call.sanitized.json"));
        let mut bytes = Vec::new();
        for line in original.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
            let mut record: serde_json::Value = serde_json::from_slice(line).unwrap();
            if record["type"] == "token_usage_record" {
                record["payload"]["response_id"] = response.into();
            }
            bytes.extend(serde_json::to_vec(&record).unwrap());
            bytes.push(b'\n');
        }
        bytes
    }
    let source = TempDir::new("scan-commit-rollback-source");
    let first = contents("response-A");
    let second = contents("response-B");
    assert_eq!(first.len(), second.len());
    let root = codex_root_with_file(&source, "rollout-rollback.jsonl", &first);
    let file = root.join("sessions/2026/09/24/rollout-rollback.jsonl");
    let (_db, storage) = temp_storage("scan-commit-rollback");
    run_codex(&storage, &root, 1_800_000_000_000);
    let revision = storage.data_revision().unwrap();
    let prior: (String,i64,String) = storage.conn().query_row("SELECT f.content_hash,f.generation,c.cursor_value FROM source_files f JOIN ingestion_checkpoints c ON c.scope_key=f.file_identity", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    storage.conn().execute_batch("CREATE TRIGGER reject_scan BEFORE INSERT ON usage_events BEGIN SELECT RAISE(ABORT,'synthetic scan write failure'); END").unwrap();
    std::fs::write(&file, &second).unwrap();
    let report = run_codex(&storage, &root, 1_800_000_001_000);
    assert_eq!(report[0].finish, RunStatus::Failed);
    assert!(report[0].outcome.is_none());
    let after: (String,i64,String) = storage.conn().query_row("SELECT f.content_hash,f.generation,c.cursor_value FROM source_files f JOIN ingestion_checkpoints c ON c.scope_key=f.file_identity", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(prior, after);
    assert_eq!(storage.data_revision().unwrap(), revision);
    assert_eq!(
        storage
            .conn()
            .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    storage
        .conn()
        .execute_batch("DROP TRIGGER reject_scan")
        .unwrap();
    let report = run_codex(&storage, &root, 1_800_000_002_000);
    assert_eq!(report[0].finish, RunStatus::Succeeded);
    assert_eq!(report[0].outcome.as_ref().unwrap().added, 1);
    assert_eq!(
        storage
            .conn()
            .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(std::fs::read(&file).unwrap(), second);
    let revision = storage.data_revision().unwrap();
    run_codex(&storage, &root, 1_800_000_003_000);
    assert_eq!(storage.data_revision().unwrap(), revision);
}
