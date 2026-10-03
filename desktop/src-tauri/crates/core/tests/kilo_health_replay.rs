//! Synthetic mutations of the sanitized 7.8.1 fixture exercise health and consumed cursors.
mod common;
use common::*;
use llm_usage_core::storage::Storage;

const NOW: i64 = 1_800_000_000_000;

fn health(storage: &Storage) -> String {
    storage
        .conn()
        .query_row("SELECT status FROM source_files", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn independent_snapshot_mismatch_keeps_valid_usage_and_replays_consumed_old_health() {
    let dir = TempDir::new("kilo-health-replay");
    let root = build_kilo_db_from_fixture(&dir, "session-7.8.1-k3.sanitized.json");
    let path = root.join(".local/share/kilo/kilo.db");
    let source = rusqlite::Connection::open(&path).unwrap();
    source.execute_batch("UPDATE session SET tokens_input=0,tokens_output=0,tokens_reasoning=0,tokens_cache_read=0,tokens_cache_write=0").unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let (_db, storage) = temp_storage("kilo-health-replay");
    run_kilo(&storage, &root, NOW);
    assert_eq!(health(&storage), "active");
    let before = summary(&storage, "2026-01-01", "2027-12-31").totals;
    assert_eq!(
        (before.call_count, before.total_tokens_known),
        (34, Some(3_189_308))
    );
    let audit: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code='reconcile_mismatch'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(audit > 0);
    // The byte-identical source has already been consumed by the old health policy.
    storage.conn().execute_batch("UPDATE source_instances SET parser_version='kilo-message-tokens-3'; UPDATE source_files SET status='degraded'").unwrap();
    let reports = run_kilo(&storage, &root, NOW + 1);
    assert_eq!(
        reports[0].files[0].events, 34,
        "parser upgrade must invalidate the consumed watermark"
    );
    assert_eq!(health(&storage), "active");
    assert_eq!(summary(&storage, "2026-01-01", "2027-12-31").totals, before);
    let outcome = reports[0].outcome.as_ref().unwrap();
    assert_eq!(
        (outcome.added, outcome.updated, outcome.conflicts),
        (0, 0, 0)
    );
    let audit_after: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code='reconcile_mismatch'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(audit_after >= audit, "audit history remains available");
    let revision = storage.data_revision().unwrap();
    run_kilo(&storage, &root, NOW + 2);
    assert_eq!(storage.data_revision().unwrap(), revision);
    assert_eq!(
        std::fs::read(path).unwrap(),
        bytes,
        "Agent database is never written by scanning"
    );
}

#[test]
fn real_record_errors_survive_new_windows_and_heal_when_the_row_is_corrected_or_deleted() {
    for defect in [
        "bad_json",
        "missing_role",
        "bad_tokens",
        "contradictory_total",
    ] {
        let dir = TempDir::new("kilo-record-health");
        let root = build_kilo_db_from_fixture(&dir, "session-7.8.1-k3.sanitized.json");
        let source = rusqlite::Connection::open(root.join(".local/share/kilo/kilo.db")).unwrap();
        let (id,original):(String,String)=source.query_row("SELECT id,data FROM message WHERE json_extract(data,'$.role')='assistant' ORDER BY time_updated,id LIMIT 1",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
        let mut data: serde_json::Value = serde_json::from_str(&original).unwrap();
        let invalid = match defect {
            "bad_json" => "{".to_string(),
            "missing_role" => {
                data.as_object_mut().unwrap().remove("role");
                data.to_string()
            }
            "bad_tokens" => {
                data["tokens"]["input"] = serde_json::json!("invalid");
                data.to_string()
            }
            _ => {
                data["tokens"]["total"] = serde_json::json!(1);
                data.to_string()
            }
        };
        source
            .execute(
                "UPDATE message SET data=?1 WHERE id=?2",
                rusqlite::params![invalid, id],
            )
            .unwrap();
        let (_db, storage) = temp_storage("kilo-record-health");
        let reports = run_kilo(&storage, &root, NOW);
        assert!(!reports[0].files.is_empty(), "{defect}: {reports:?}");
        assert!(reports[0].outcome.is_some(), "{defect}: {reports:?}");
        assert_eq!(health(&storage), "degraded", "{defect}");
        let high: i64 = source
            .query_row("SELECT MAX(time_updated) FROM message", [], |r| r.get(0))
            .unwrap();
        source.execute("UPDATE message SET time_updated=?1 WHERE id=(SELECT id FROM message WHERE id!=?2 ORDER BY time_updated DESC LIMIT 1)",rusqlite::params![high+120_000,id]).unwrap();
        run_kilo(&storage, &root, NOW + 1);
        assert_eq!(
            health(&storage),
            "degraded",
            "new valid rows cannot mask {defect}"
        );
        source
            .execute(
                "UPDATE message SET data=?1,time_updated=?2 WHERE id=?3",
                rusqlite::params![original, high + 240_000, id],
            )
            .unwrap();
        run_kilo(&storage, &root, NOW + 2);
        assert_eq!(
            health(&storage),
            "active",
            "corrected row must clear {defect}"
        );
        assert_eq!(
            summary(&storage, "2026-01-01", "2027-12-31")
                .totals
                .call_count,
            34
        );
        source
            .execute(
                "UPDATE message SET data='{',time_updated=?1 WHERE id=?2",
                rusqlite::params![high + 360_000, id],
            )
            .unwrap();
        run_kilo(&storage, &root, NOW + 3);
        assert_eq!(health(&storage), "degraded");
        source
            .execute("DELETE FROM message WHERE id=?1", [id])
            .unwrap();
        run_kilo(&storage, &root, NOW + 4);
        assert_eq!(
            health(&storage),
            "active",
            "removed source error does not invalidate retained history"
        );
    }
}

#[test]
fn snapshot_audit_does_not_certify_an_unverified_message_version() {
    let dir = TempDir::new("kilo-unverified-audit");
    let root = build_kilo_db_from_fixture(&dir, "session-7.8.1-k3.sanitized.json");
    let source = rusqlite::Connection::open(root.join(".local/share/kilo/kilo.db")).unwrap();
    source.execute_batch("UPDATE session SET version='7.3.42',tokens_input=0,tokens_output=0,tokens_reasoning=0,tokens_cache_read=0,tokens_cache_write=0").unwrap();
    let (_db, storage) = temp_storage("kilo-unverified-audit");
    run_kilo(&storage, &root, NOW);
    assert_eq!(health(&storage), "active_compat");
    assert_eq!(
        summary(&storage, "2026-01-01", "2027-12-31")
            .totals
            .total_tokens_known,
        Some(3_189_308)
    );
    let verified: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE parse_basis!='latest_fallback'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        verified, 0,
        "valid shape and snapshot audit never certify an unknown version"
    );
}
