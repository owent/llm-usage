mod common;
use common::*;
use llm_usage_core::{
    adapters::{built_in_adapters, framework::*},
    storage::Storage,
};
use rusqlite::{params, Connection};
use std::path::Path;

const NOW: i64 = 1_794_797_500_000;
fn sample(path: &Path) -> Connection {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let c = Connection::open(path).unwrap();
    c.execute_batch("PRAGMA journal_mode=WAL;
        CREATE TABLE chat_session(session_id TEXT,version TEXT);
        CREATE TABLE chat_record(request_id TEXT,session_id TEXT,gmt_create INTEGER,gmt_modified INTEGER,extra TEXT,chat_task TEXT,mode TEXT,session_type TEXT);
        CREATE TABLE chat_message(id TEXT,session_id TEXT,request_id TEXT,role TEXT);
        INSERT INTO chat_session VALUES('s','2');").unwrap();
    for id in ["a", "b"] {
        c.execute("INSERT INTO chat_record VALUES(?1,'s',?2,?2,'{\"modelConfig\":{\"key\":\"qmodel_latest\"}}','FREE_INPUT','chat','assistant')",params![id,NOW-100]).unwrap();
        c.execute(
            "INSERT INTO chat_message VALUES(?1,'s',?1,'assistant')",
            [id],
        )
        .unwrap();
    }
    c
}
fn run(storage: &Storage, ctx: &DiscoverContext, now: i64, max: Option<u64>) {
    for adapter in built_in_adapters() {
        let mut limits = ScanLimits::default();
        limits.jsonl.max_lines = max;
        let reports = run_adapter_scan(
            storage,
            adapter.as_ref(),
            ctx,
            &RunConfig {
                now_ms: now,
                timezone: "UTC".into(),
                trigger: llm_usage_core::jobs::TriggerKind::Manual,
                limits,
                origin_host_id: None,
                run_id_prefix: format!("cn-{}-{now}", adapter.adapter_id()),
            },
        )
        .unwrap();
        assert!(
            reports
                .iter()
                .all(|r| r.finish != llm_usage_core::jobs::RunStatus::Failed),
            "{reports:?}"
        );
    }
}
fn count(storage: &Storage, sql: &str) -> i64 {
    storage.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn cn_vscode_default_discovery_wal_and_observations_keep_tokens_unknown() {
    let dir = TempDir::new("cn-native-shape");
    let db = dir
        .path()
        .join(".lingma/vscode/sharedClientCache/cache/db/local.db");
    let c = sample(&db);
    let (_data, storage) = temp_storage("cn-discovery");
    let ctx = DiscoverContext {
        home_dir: Some(dir.path().into()),
        ..Default::default()
    };
    for now in [NOW, NOW + 1] {
        run(&storage, &ctx, now, None);
    }
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 2);
    assert_eq!(count(&storage,"SELECT COUNT(*) FROM usage_events WHERE agent='qoder' AND record_kind='usage_observation' AND host_application='vscode' AND quality_bucket='unknown' AND input_total IS NULL AND output_total IS NULL AND total_tokens IS NULL AND provider_id IS NULL AND cost_amount_minor IS NULL AND model_raw='qmodel_latest' AND conflict=0"),2);
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM usage_events WHERE observed_at_ms!=1794797500000"
        ),
        0
    );
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM source_files"), 1);
    assert_eq!(count(&storage,"SELECT COUNT(*) FROM source_files WHERE json_extract(format_status,'$.basis')='latest_fallback' AND json_extract(format_status,'$.found_version') IS NULL"),1);
    c.execute("UPDATE chat_record SET gmt_modified=?1, extra='{\"modelConfig\":{\"key\":\"qwen4-max-preview\"}}' WHERE request_id='a'",[NOW+2]).unwrap();
    run(&storage, &ctx, NOW + 3, None);
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 2);
    assert_eq!(count(&storage,"SELECT COUNT(*) FROM usage_events WHERE model_raw='qwen4-max-preview' AND source_revision=1794797500002"),1);
}

#[test]
fn cn_manual_parent_discovery_old_misrouting_pagination_and_invalid_rows() {
    let dir = TempDir::new("cn-recovery");
    let db = dir
        .path()
        .join(".lingma/vscode/sharedClientCache/cache/db/local.db");
    let c = sample(&db);
    c.execute(
        "UPDATE chat_record SET request_id=NULL WHERE request_id='a'",
        [],
    )
    .unwrap();
    let (_data, storage) = temp_storage("cn-old-owner");
    let ctx = DiscoverContext {
        manual_roots: vec![dir.path().join(".lingma")],
        ..Default::default()
    };
    run(&storage, &ctx, NOW, Some(1));
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 0);
    run(&storage, &ctx, NOW + 1, Some(1));
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 1);
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM diagnostics WHERE code='qoder_cn_row_invalid'"
        ),
        1
    );
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM source_files WHERE status='degraded'"
        ),
        1
    );
    storage.conn().execute_batch("UPDATE source_instances SET parser_version='old-probe'; UPDATE ingestion_checkpoints SET cursor_value='{\"offset\":999999}',parse_context=NULL; INSERT INTO diagnostics(batch_id,instance_id,code,message,created_ms) SELECT 'old',instance_id,'usage_fields_unverified','old probe',1 FROM source_instances WHERE format='qoder';").unwrap();
    run(&storage, &ctx, NOW + 2, None);
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 1);
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM diagnostics WHERE code='usage_fields_unverified'"
        ),
        1
    );
    for (i, root) in [
        db.clone(),
        db.parent().unwrap().into(),
        dir.path().join(".lingma/vscode"),
        dir.path().join(".lingma"),
    ]
    .into_iter()
    .enumerate()
    {
        run(
            &storage,
            &DiscoverContext {
                manual_roots: vec![root],
                ..Default::default()
            },
            NOW + 3 + i as i64,
            None,
        );
    }
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 1);
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM source_files"), 1);
}

#[test]
fn cn_database_and_cli_transcript_in_the_same_manual_directory_are_both_read() {
    let dir = TempDir::new("cn-cli-mixed-root");
    let _c = sample(&dir.path().join("local.db"));
    std::fs::write(
        dir.path().join("session.jsonl"),
        include_str!("fixtures/qoder/source-cli-1.1.76/session.jsonl"),
    )
    .unwrap();
    let (_data, storage) = temp_storage("cn-cli-mixed");
    let ctx = DiscoverContext {
        manual_roots: vec![dir.path().into()],
        ..Default::default()
    };
    for now in [NOW, NOW + 1] {
        run(&storage, &ctx, now, None);
    }
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 6);
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM usage_events WHERE record_kind='usage_observation'"
        ),
        2
    );
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM source_files"), 2);
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM source_instances WHERE format='qoder'"
        ),
        1
    );
}

#[test]
fn cn_database_old_owner_and_transaction_failure_do_not_lose_records_or_diagnostics() {
    let dir = TempDir::new("cn-ownership");
    let db = dir.path().join("custom-name.bin");
    let _c = sample(&db);
    let (_data, storage) = temp_storage("cn-rollback");
    let ctx = DiscoverContext {
        manual_roots: vec![db.clone()],
        ..Default::default()
    };
    storage.conn().execute_batch("CREATE TRIGGER reject_cn BEFORE INSERT ON ingestion_checkpoints BEGIN SELECT RAISE(ABORT,'controlled rollback'); END;").unwrap();
    let adapter = llm_usage_core::adapters::qoder::QoderAdapter::new();
    let result = run_adapter_scan(
        &storage,
        &adapter,
        &ctx,
        &RunConfig {
            now_ms: NOW,
            timezone: "UTC".into(),
            trigger: llm_usage_core::jobs::TriggerKind::Manual,
            limits: ScanLimits::default(),
            origin_host_id: None,
            run_id_prefix: "cn-rollback".into(),
        },
    );
    assert!(
        result.is_err()
            || result
                .unwrap()
                .iter()
                .any(|r| r.finish == llm_usage_core::jobs::RunStatus::Failed)
    );
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 0);
    assert_eq!(
        count(&storage, "SELECT COUNT(*) FROM ingestion_checkpoints"),
        0
    );
    storage
        .conn()
        .execute_batch("DROP TRIGGER reject_cn;")
        .unwrap();
    run(&storage, &ctx, NOW + 1, None);
    storage.conn().execute_batch("DELETE FROM usage_events; DELETE FROM daily_usage; DELETE FROM period_usage;
        INSERT INTO source_instances(instance_id,agent,format,location_hint,locality_basis,attribution_status,parser_version,created_at_ms,updated_at_ms) VALUES('codex@old-cn','codex','codex','old','local_filesystem','verified','old',1,1);
        UPDATE source_files SET instance_id='codex@old-cn';
        UPDATE ingestion_checkpoints SET instance_id='codex@old-cn';
        INSERT INTO diagnostics(batch_id,instance_id,code,message,created_ms) VALUES('old-cn','codex@old-cn','unknown_format','retained',1);").unwrap();
    run(&storage, &ctx, NOW + 2, None);
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM usage_events WHERE agent='qoder'"
        ),
        2
    );
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM source_files WHERE instance_id LIKE 'qoder@%'"
        ),
        1
    );
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM diagnostics WHERE instance_id='codex@old-cn'"
        ),
        1
    );
}

#[test]
#[ignore = "read-only local Qoder CN acceptance; set QODER_CN_NATIVE_DB explicitly"]
fn cn_read_only_local_acceptance() {
    let db = std::path::PathBuf::from(
        std::env::var_os("QODER_CN_NATIVE_DB").expect("QODER_CN_NATIVE_DB"),
    );
    let (_data, storage) = temp_storage("cn-real-local");
    let ctx = DiscoverContext {
        manual_roots: vec![db],
        ..Default::default()
    };
    run(&storage, &ctx, NOW, None);
    let first = count(
        &storage,
        "SELECT COUNT(*) FROM usage_events WHERE agent='qoder' AND record_kind='usage_observation'",
    );
    assert!(first > 0);
    run(&storage, &ctx, NOW + 1, None);
    assert_eq!(first, count(&storage, "SELECT COUNT(*) FROM usage_events"));
    assert_eq!(count(&storage,"SELECT COUNT(*) FROM usage_events WHERE total_tokens IS NOT NULL OR input_total IS NOT NULL OR output_total IS NOT NULL OR provider_id IS NOT NULL OR cost_amount_minor IS NOT NULL OR conflict!=0"),0);
    println!("Qoder CN native observations: {first}; repeat reads unchanged; token/cost unknown");
}
