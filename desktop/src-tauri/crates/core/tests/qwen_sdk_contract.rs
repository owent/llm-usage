//! Genuine 0.25.0 SDK file and native ChatRecord; synthetic boundary controls are explicit.
mod common;
use common::{summary, temp_storage, TempDir};
use llm_usage_core::adapters::{framework::*, otel::OtelAdapter, qwen::QwenAdapter};
use llm_usage_core::{jobs::TriggerKind, storage::Storage};
use std::path::Path;
const NOW: i64 = 1_800_000_000_000;
const SDK: &str = include_str!("fixtures/otel/real-qwen-0.25.0-sdk/telemetry.json");
const NATIVE: &str = include_str!("fixtures/otel/real-qwen-0.25.0-sdk/native.jsonl");

fn scan(
    storage: &Storage,
    adapter: &dyn SourceAdapter,
    path: &Path,
    now: i64,
    host: Option<&str>,
    max: Option<u64>,
    cap: Option<usize>,
) -> SourceRunReport {
    let mut limits = ScanLimits::default();
    limits.jsonl.max_lines = max;
    if let Some(cap) = cap {
        limits.jsonl.max_line_bytes = cap;
        limits.jsonl.chunk_bytes = 7;
    }
    let reports = run_adapter_scan(
        storage,
        adapter,
        &DiscoverContext {
            manual_roots: vec![path.into()],
            ..Default::default()
        },
        &RunConfig {
            timezone: "UTC".into(),
            now_ms: now,
            limits,
            trigger: TriggerKind::Manual,
            origin_host_id: host.map(str::to_string),
            run_id_prefix: format!("run-{now}"),
        },
    )
    .unwrap();
    assert_eq!(reports.len(), 1);
    reports.into_iter().next().unwrap()
}

fn files(dir: &TempDir) -> (std::path::PathBuf, std::path::PathBuf) {
    let native = dir.path().join(".qwen");
    let sdk = dir.path().join("sdk.json");
    let chats = native.join("tmp/project/chats");
    std::fs::create_dir_all(&chats).unwrap();
    std::fs::write(chats.join("native.jsonl"), NATIVE).unwrap();
    std::fs::write(&sdk, SDK).unwrap();
    (native, sdk)
}

#[test]
fn real_sdk_spans_match_cli_and_replace_native_in_both_orders_without_duplicates() {
    for sdk_first in [false, true] {
        let dir = TempDir::new("qwen-sdk-real");
        let (native, sdk) = files(&dir);
        let (_db, storage) = temp_storage("qwen-sdk-real");
        let sdk_adapter = OtelAdapter::new();
        let native_adapter = QwenAdapter::new();
        assert!(
            matches!(sdk_adapter.detect(&sdk).unwrap(),DetectOutcome::Supported{format,basis:llm_usage_core::domain::VersionBasis::KnownVersion,..} if format=="qwen-code-sdk-json-stream")
        );
        if sdk_first {
            scan(
                &storage,
                &sdk_adapter,
                &sdk,
                NOW,
                Some("test-host"),
                None,
                None,
            );
        } else {
            scan(
                &storage,
                &native_adapter,
                &native,
                NOW,
                Some("test-host"),
                None,
                None,
            );
        }
        if sdk_first {
            scan(
                &storage,
                &native_adapter,
                &native,
                NOW + 1,
                Some("test-host"),
                None,
                None,
            );
        } else {
            scan(
                &storage,
                &sdk_adapter,
                &sdk,
                NOW + 1,
                Some("test-host"),
                None,
                None,
            );
        }
        let totals = summary(&storage, "2026-10-05", "2026-10-05").totals;
        assert_eq!(
            (
                totals.call_count,
                totals.input_total_known,
                totals.output_total_known,
                totals.total_tokens_known
            ),
            (2, Some(15865), Some(558), Some(16423))
        );
        assert_eq!(
            storage
                .conn()
                .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            3
        );
        assert_eq!(storage.conn().query_row("SELECT COUNT(*) FROM usage_events WHERE exclusion_reason='qwen_sdk_session_authority'",[],|r|r.get::<_,i64>(0)).unwrap(),1);
        let fields:(i64,i64,i64)=storage.conn().query_row("SELECT SUM(input_cache_read),SUM(input_cache_write IS NULL),SUM(parent_session_id IS NULL) FROM usage_events WHERE attribution_status='verified'",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
        assert_eq!(fields, (3, 2, 2));
        assert_eq!(storage.conn().query_row("SELECT COUNT(*) FROM usage_events WHERE call_category='auxiliary' AND total_tokens=6195",[],|r|r.get::<_,i64>(0)).unwrap(),1);
        assert!(summary(&storage, "2026-10-05", "2026-10-05").periods[0].partial_history);
        scan(
            &storage,
            &sdk_adapter,
            &sdk,
            NOW + 2,
            Some("test-host"),
            None,
            None,
        );
        scan(
            &storage,
            &native_adapter,
            &native,
            NOW + 3,
            Some("test-host"),
            None,
            None,
        );
        let duplicate = dir.path().join("duplicate.json");
        std::fs::write(&duplicate, SDK).unwrap();
        scan(
            &storage,
            &sdk_adapter,
            &duplicate,
            NOW + 4,
            Some("test-host"),
            None,
            None,
        );
        assert_eq!(summary(&storage, "2026-10-05", "2026-10-05").totals, totals);
        assert_eq!(storage.conn().query_row("SELECT COUNT(*) FROM usage_events WHERE exclusion_reason='qwen_sdk_span_duplicate'",[],|r|r.get::<_,i64>(0)).unwrap(),2);
    }
}

#[test]
fn object_budget_half_object_and_cap_recover_at_complete_boundaries() {
    let dir = TempDir::new("qwen-sdk-boundaries");
    let path = dir.path().join("sdk.json");
    let values: Vec<serde_json::Value> = serde_json::Deserializer::from_str(SDK)
        .into_iter()
        .collect::<Result<_, _>>()
        .unwrap();
    let calls: Vec<_> = values
        .iter()
        .filter(|v| v["name"] == "qwen-code.llm_request")
        .collect();
    let first = serde_json::to_string_pretty(calls[0]).unwrap();
    let second = serde_json::to_string_pretty(calls[1]).unwrap();
    std::fs::write(&path, format!("{first}\n{}", &second[..second.len() / 2])).unwrap();
    let (_db, storage) = temp_storage("qwen-sdk-boundaries");
    let adapter = OtelAdapter::new();
    let half = scan(
        &storage,
        &adapter,
        &path,
        NOW,
        Some("test-host"),
        None,
        None,
    );
    assert_eq!(half.files[0].status, "pending");
    assert_eq!(
        summary(&storage, "2026-10-05", "2026-10-05")
            .totals
            .call_count,
        1
    );
    std::fs::write(&path, format!("{first}\n{second}\n")).unwrap();
    scan(
        &storage,
        &adapter,
        &path,
        NOW + 1,
        Some("test-host"),
        Some(1),
        Some(7),
    );
    assert_eq!(
        summary(&storage, "2026-10-05", "2026-10-05")
            .totals
            .call_count,
        1
    );
    let resumed = scan(
        &storage,
        &adapter,
        &path,
        NOW + 2,
        Some("test-host"),
        Some(1),
        Some(8192),
    );
    assert_eq!(resumed.files[0].status, "budget_exhausted");
    let completed = scan(
        &storage,
        &adapter,
        &path,
        NOW + 3,
        Some("test-host"),
        None,
        Some(8192),
    );
    assert_eq!(completed.files[0].status, "complete");
    assert_eq!(
        summary(&storage, "2026-10-05", "2026-10-05")
            .totals
            .total_tokens_known,
        Some(16423)
    );
}

#[test]
fn unverified_version_or_host_stays_isolated_and_nonzero_thoughts_do_not_guess_total() {
    for (host, unknown_version) in [(None, false), (Some("test-host"), true)] {
        let dir = TempDir::new("qwen-sdk-isolated");
        let path = dir.path().join("sdk.json");
        std::fs::write(
            &path,
            if unknown_version {
                SDK.replace("0.25.0", "0.999.0")
            } else {
                SDK.to_string()
            },
        )
        .unwrap();
        let (_db, storage) = temp_storage("qwen-sdk-isolated");
        scan(&storage, &OtelAdapter::new(), &path, NOW, host, None, None);
        assert_eq!(
            summary(&storage, "2026-10-05", "2026-10-05")
                .totals
                .call_count,
            0
        );
        assert_eq!(
            storage
                .conn()
                .query_row(
                    "SELECT COUNT(*) FROM usage_events WHERE attribution_status='excluded'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            2
        );
    }
    let dir = TempDir::new("qwen-sdk-thoughts");
    let path = dir.path().join("sdk.json");
    std::fs::write(
        &path,
        SDK.replace("\"thoughts_token_count\": 0", "\"thoughts_token_count\": 1"),
    )
    .unwrap();
    let (_db, storage) = temp_storage("qwen-sdk-thoughts");
    scan(
        &storage,
        &OtelAdapter::new(),
        &path,
        NOW,
        Some("test-host"),
        None,
        None,
    );
    assert_eq!(
        storage
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM usage_events WHERE total_tokens IS NULL",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        2
    );
}

#[test]
fn sealed_native_partition_blocks_sdk_even_after_native_details_are_removed() {
    let dir = TempDir::new("qwen-sdk-sealed");
    let (native, sdk) = files(&dir);
    let (_db, storage) = temp_storage("qwen-sdk-sealed");
    scan(
        &storage,
        &QwenAdapter::new(),
        &native,
        NOW,
        Some("test-host"),
        None,
        None,
    );
    storage
        .conn()
        .execute("UPDATE daily_usage SET sealed=1", [])
        .unwrap();
    storage
        .conn()
        .execute("DELETE FROM usage_events", [])
        .unwrap();
    let before = summary(&storage, "2026-10-05", "2026-10-05").totals;
    scan(
        &storage,
        &OtelAdapter::new(),
        &sdk,
        NOW + 1,
        Some("test-host"),
        None,
        None,
    );
    assert_eq!(summary(&storage, "2026-10-05", "2026-10-05").totals, before);
    assert_eq!(storage.conn().query_row("SELECT COUNT(*) FROM usage_events WHERE exclusion_reason='qwen_native_partition_sealed'",[],|r|r.get::<_,i64>(0)).unwrap(),2);
}

#[test]
fn sealed_sdk_partition_blocks_a_new_export_copy_after_details_are_removed() {
    let dir = TempDir::new("qwen-sdk-sealed-copy");
    let (_, sdk) = files(&dir);
    let (_db, storage) = temp_storage("qwen-sdk-sealed-copy");
    scan(
        &storage,
        &OtelAdapter::new(),
        &sdk,
        NOW,
        Some("test-host"),
        None,
        None,
    );
    storage
        .conn()
        .execute("UPDATE daily_usage SET sealed=1", [])
        .unwrap();
    storage
        .conn()
        .execute("DELETE FROM usage_events", [])
        .unwrap();
    let before = summary(&storage, "2026-10-05", "2026-10-05").totals;
    let duplicate = dir.path().join("new-copy.json");
    std::fs::write(&duplicate, SDK).unwrap();
    scan(
        &storage,
        &OtelAdapter::new(),
        &duplicate,
        NOW + 1,
        Some("test-host"),
        None,
        None,
    );
    assert_eq!(summary(&storage, "2026-10-05", "2026-10-05").totals, before);
    assert_eq!(storage.conn().query_row("SELECT COUNT(*) FROM usage_events WHERE exclusion_reason='qwen_sdk_partition_sealed'", [], |r| r.get::<_, i64>(0)).unwrap(), 2);
}

#[test]
fn real_retention_prevents_native_and_sdk_copies_from_reviving_archived_usage() {
    let dir = TempDir::new("qwen-sdk-retention");
    let (native, sdk) = files(&dir);
    let (db, storage) = temp_storage("qwen-sdk-retention");
    scan(
        &storage,
        &QwenAdapter::new(),
        &native,
        NOW,
        Some("test-host"),
        None,
        None,
    );
    scan(
        &storage,
        &OtelAdapter::new(),
        &sdk,
        NOW + 1,
        Some("test-host"),
        None,
        None,
    );
    let before = summary(&storage, "2026-10-05", "2026-10-05").totals;
    let result = llm_usage_core::retention::enforce_retention(
        &storage,
        "UTC",
        NOW + 2,
        &llm_usage_core::retention::RetentionPolicy {
            detail_days: 7,
            diagnostics_days: 7,
            hard_max_days: None,
        },
    )
    .unwrap();
    assert_eq!(result.deleted_events, 3);
    assert!(result.sealed_days.contains(&"2026-10-05".to_string()));
    drop(storage);
    let storage = Storage::open(&db.db_path()).unwrap();
    let duplicate = dir.path().join("late-sdk.json");
    std::fs::write(&duplicate, SDK).unwrap();
    scan(
        &storage,
        &OtelAdapter::new(),
        &duplicate,
        NOW + 3,
        Some("test-host"),
        None,
        None,
    );
    let late_native = dir.path().join("late/.qwen");
    std::fs::create_dir_all(late_native.join("tmp/project/chats")).unwrap();
    std::fs::write(late_native.join("tmp/project/chats/native.jsonl"), NATIVE).unwrap();
    scan(
        &storage,
        &QwenAdapter::new(),
        &late_native,
        NOW + 4,
        Some("test-host"),
        None,
        None,
    );
    let after = summary(&storage, "2026-10-05", "2026-10-05").totals;
    assert_eq!(
        (
            after.call_count,
            after.input_total_known,
            after.output_total_known,
            after.total_tokens_known
        ),
        (
            before.call_count,
            before.input_total_known,
            before.output_total_known,
            before.total_tokens_known
        )
    );
    assert_eq!(
        storage
            .conn()
            .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn bad_objects_do_not_mask_valid_calls_or_mark_the_policy_complete() {
    let dir = TempDir::new("qwen-sdk-bad-objects");
    let path = dir.path().join("sdk.json");
    let mut values: Vec<serde_json::Value> = serde_json::Deserializer::from_str(SDK)
        .into_iter()
        .collect::<Result<_, _>>()
        .unwrap();
    let first = values
        .iter_mut()
        .find(|v| v["name"] == "qwen-code.llm_request")
        .unwrap();
    first["attributes"]["gen_ai.usage.input_tokens"] = "invalid".into();
    let text = values
        .iter()
        .map(|v| serde_json::to_string_pretty(v).unwrap() + "\n")
        .collect::<String>()
        + "{\"synthetic\": invalid}\n";
    std::fs::write(&path, text).unwrap();
    let (_db, storage) = temp_storage("qwen-sdk-bad-objects");
    let adapter = OtelAdapter::new();
    scan(
        &storage,
        &adapter,
        &path,
        NOW,
        Some("test-host"),
        None,
        None,
    );
    assert_eq!(
        summary(&storage, "2026-10-05", "2026-10-05")
            .totals
            .call_count,
        2
    );
    assert_eq!(
        storage
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM usage_events WHERE input_total IS NULL",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    scan(
        &storage,
        &adapter,
        &path,
        NOW + 1,
        Some("test-host"),
        None,
        None,
    );
    let context: String = storage
        .conn()
        .query_row("SELECT parse_context FROM ingestion_checkpoints", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&context).unwrap()["policy_version"],
        0
    );
    assert_eq!(
        storage
            .conn()
            .query_row("SELECT status FROM source_files", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "degraded"
    );
    assert_eq!(
        storage
            .conn()
            .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
}

#[test]
fn authority_rolls_back_with_checkpoint_and_does_not_cross_hosts_or_local_days() {
    let dir = TempDir::new("qwen-sdk-scope");
    let (native, sdk) = files(&dir);
    let (_db, storage) = temp_storage("qwen-sdk-scope");
    scan(
        &storage,
        &QwenAdapter::new(),
        &native,
        NOW,
        Some("host-a"),
        None,
        None,
    );
    let before = summary(&storage, "2026-10-05", "2026-10-05").totals;
    storage.conn().execute_batch("CREATE TRIGGER abort_qwen_checkpoint BEFORE INSERT ON ingestion_checkpoints BEGIN SELECT RAISE(ABORT,'synthetic transaction failure'); END;").unwrap();
    let failed = scan(
        &storage,
        &OtelAdapter::new(),
        &sdk,
        NOW + 1,
        Some("host-a"),
        None,
        None,
    );
    assert!(failed.error.is_some());
    assert_eq!(summary(&storage, "2026-10-05", "2026-10-05").totals, before);
    assert_eq!(
        storage
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM settings WHERE key='qwen_sdk_scopes_v1'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    storage
        .conn()
        .execute_batch("DROP TRIGGER abort_qwen_checkpoint;")
        .unwrap();
    let other_host = dir.path().join("host-b.json");
    std::fs::write(&other_host, SDK).unwrap();
    scan(
        &storage,
        &OtelAdapter::new(),
        &other_host,
        NOW + 2,
        Some("host-b"),
        None,
        None,
    );
    assert_eq!(
        summary(&storage, "2026-10-05", "2026-10-05")
            .totals
            .call_count,
        3
    );
    assert_eq!(storage.conn().query_row("SELECT COUNT(*) FROM usage_events WHERE exclusion_reason='qwen_sdk_session_authority'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    let other = dir.path().join("other-day.json");
    std::fs::write(
        &other,
        SDK.replace("1791206786", "1791293186")
            .replace("1791206801", "1791293201")
            .replace("1791206814", "1791293214"),
    )
    .unwrap();
    scan(
        &storage,
        &OtelAdapter::new(),
        &other,
        NOW + 3,
        Some("host-a"),
        None,
        None,
    );
    assert_eq!(
        summary(&storage, "2026-10-05", "2026-10-05")
            .totals
            .call_count,
        3
    );
    assert_eq!(
        summary(&storage, "2026-10-06", "2026-10-06")
            .totals
            .call_count,
        2
    );
}
