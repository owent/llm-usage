//! Kilo Code CLI pipeline tests using redacted session-7.4.9-family and
//! session-7.4.8-edges datasets extracted from local sessions.
//! Read, parse, normalize, commit, and query against independent manual expectations
//! in tests/fixtures/kilo/*._expectations.md without changing calculation rules.

mod common;

use common::*;

// Manual totals from _expectations.md:
// 7.4.9-family: five sessions (one main, four subagents), 58 messages (51 assistant, 7 user).
// Individual sums: input=121774, output=8876, reasoning=45302,
// cache_read=2137472, cache_write=0; derived_total=2313424.
// Canonical total input = input + read + write = 2259246.
// Output including reasoning = 54178; complete total = 2313424.
// Session detail sums 1984963/39997/167085/61589/59790 all match snapshots.
// One unfinished message lacks tokens.total: missing_total_msgs=1.
// 7.4.8-edges: one session, 16 messages (13 assistant, 3 user); models k2p7 twice
// and glm-5.2 eleven times. Two error messages have zero buckets and lack total.
// Sums: input=33638, output=1535, reasoning=4403, cache_read=272896, write=0.
// Derived total=312472; canonical input=306534, output=5938.

const NOW: i64 = 1_800_000_000_000;

#[test]
fn archived_sessions_before_and_after_collection_keep_the_same_usage() {
    let dir = TempDir::new("kilo-archived");
    let root = build_kilo_db_from_fixture(&dir, "session-7.4.9-family.sanitized.json");
    let path = root.join(".local/share/kilo/kilo.db");
    let source = rusqlite::Connection::open(path).unwrap();
    source
        .execute("UPDATE session SET time_archived=?1", [NOW])
        .unwrap();
    let (_db, storage) = temp_storage("kilo-archived");
    run_kilo(&storage, &root, NOW);
    let before = summary(&storage, "2026-01-01", "2026-12-31").totals;
    assert_eq!(before.call_count, 51);
    assert_eq!(before.total_tokens_known, Some(2_313_424));
    source
        .execute("UPDATE session SET time_archived=NULL", [])
        .unwrap();
    run_kilo(&storage, &root, NOW + 1);
    source
        .execute("UPDATE session SET time_archived=?1", [NOW + 2])
        .unwrap();
    run_kilo(&storage, &root, NOW + 2);
    assert_eq!(summary(&storage, "2026-01-01", "2026-12-31").totals, before);
}

#[test]
fn contract_family_fixture_full_pipeline_matches_expectations() {
    let dir = TempDir::new("kilo-family");
    let root = build_kilo_db_from_fixture(&dir, "session-7.4.9-family.sanitized.json");
    let (_db, storage) = temp_storage("kilo-family");
    let reports = run_kilo(&storage, &root, NOW);

    let report = &reports[0];
    assert_eq!(report.files.len(), 1, "一个 kilo.db 一个文件目标");
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].records_seen, 58, "全部消息行（含 user）");
    assert_eq!(report.files[0].events, 51, "只有 assistant 记 model_call");
    let outcome = report.outcome.as_ref().unwrap();
    assert_eq!((outcome.added, outcome.updated, outcome.errors), (51, 0, 0));

    // Check UTC sums over the selected year containing the July/August samples.
    let summary = summary(&storage, "2026-01-01", "2026-12-31");
    assert_eq!(summary.totals.call_count, 51);
    assert_eq!(summary.totals.input_total_known, Some(2_259_246));
    assert_eq!(summary.totals.output_total_known, Some(54_178));
    assert_eq!(summary.totals.cache_read_known, Some(2_137_472));
    assert_eq!(summary.totals.cache_write_known, Some(0));
    assert_eq!(summary.totals.total_tokens_known, Some(2_313_424));

    let conn = storage.conn();
    // Main session: 37 primary; child sessions with parent_id: 14 sub_agent.
    let (primary, sub): (i64, i64) = conn
        .query_row(
            "SELECT SUM(call_category = 'primary'), SUM(call_category = 'sub_agent') \
             FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((primary, sub), (37, 14));

    // Each session's registered version selects known_version; do not infer from the database maximum.
    let bases: std::collections::BTreeSet<String> = {
        let mut stmt = conn
            .prepare("SELECT DISTINCT parse_basis FROM usage_events")
            .unwrap();
        stmt.query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    assert_eq!(
        bases,
        std::iter::once("known_version".to_string()).collect(),
        "7.4.9 注册在 VERIFIED_VERSION_IMPLS"
    );

    // Check stable kilo:msg:<message.id> identity and the selected token fields.
    type RowRow = (
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<String>,
        Option<String>,
        String,
        String,
        String,
    );
    let row: RowRow = conn
        .query_row(
            "SELECT input_total, input_cache_read, input_uncached, output_total, \
             output_reasoning, total_tokens, provider_id, model_raw, session_id, \
             model_attribution, time_basis \
             FROM usage_events WHERE source_record_key = 'kilo:msg:anon-8'",
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                    r.get(10)?,
                ))
            },
        )
        .unwrap();
    // anon-8: input=20040, output=51, reasoning=477, caches=0, total=20568.
    assert_eq!(row.0, Some(20_040), "input_total = input + cr + cw");
    assert_eq!(row.1, Some(0));
    assert_eq!(row.2, Some(20_040), "input_uncached = 互斥口径的 input 桶");
    assert_eq!(row.3, Some(528), "output_total = output + reasoning");
    assert_eq!(row.4, Some(477));
    assert_eq!(row.5, Some(20_568));
    assert_eq!(row.6.as_deref(), Some("zhipuai-coding-plan"));
    assert_eq!(row.7.as_deref(), Some("glm-5.2"));
    assert_eq!(row.8, "anon-1");
    assert_eq!(row.9, "request_field");
    assert_eq!(row.10, "source_completion");

    // Check parent_session_id and sub_agent classification for child-session messages;
    // anon-47 is an assistant message in child session anon-3.
    let (category, parent): (String, Option<String>) = conn
        .query_row(
            "SELECT call_category, parent_session_id FROM usage_events \
             WHERE source_record_key = 'kilo:msg:anon-47'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(category, "sub_agent");
    assert_eq!(parent.as_deref(), Some("anon-1"));

    // Unfinished messages lack source_total and use partial lifecycle/source_start time.
    let (source_total, lifecycle, time_basis): (Option<i64>, String, String) = conn
        .query_row(
            "SELECT source_total, lifecycle, time_basis FROM usage_events \
             WHERE source_record_key = 'kilo:msg:anon-13'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(source_total, None, "missing_total_msgs=1：缺 total 不补零");
    assert_eq!(lifecycle, "partial", "无 finish/error 的未完成消息");
    assert_eq!(time_basis, "source_start", "无 completed 退 created");
    let partial_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE lifecycle = 'partial'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(partial_count, 1);

    // All five cumulative snapshots match independently calculated session detail sums.
    let mut details: Vec<i64> = report
        .reconciliations
        .iter()
        .map(|r| r.detail_sum)
        .collect();
    details.sort_unstable();
    assert_eq!(
        details,
        vec![39_997, 59_790, 61_589, 167_085, 1_984_963],
        "逐会话 detail_sum 与 _expectations.md 一致"
    );
    assert!(report
        .reconciliations
        .iter()
        .all(|r| r.verdict == "matched" && r.carried_sum == 0));

    // Event time comes from data.time.completed in epoch milliseconds.
    let occurred: i64 = conn
        .query_row(
            "SELECT occurred_at_ms FROM usage_events \
             WHERE source_record_key = 'kilo:msg:anon-8'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(occurred, 1_784_190_965_520);

    // Every record in this redacted native dataset parses without diagnostics.
    let diags: i64 = conn
        .query_row("SELECT COUNT(*) FROM diagnostics", [], |r| r.get(0))
        .unwrap();
    assert_eq!(diags, 0);
    assert_eq!(report.files[0].diagnostics, 0);
    let _ = dir;
}

#[test]
fn contract_edges_fixture_error_messages_and_model_switch() {
    let dir = TempDir::new("kilo-edges");
    let root = build_kilo_db_from_fixture(&dir, "session-7.4.8-edges.sanitized.json");
    let (_db, storage) = temp_storage("kilo-edges");
    let reports = run_kilo(&storage, &root, NOW);

    let report = &reports[0];
    assert_eq!(report.files[0].records_seen, 16);
    assert_eq!(report.files[0].events, 13);
    let outcome = report.outcome.as_ref().unwrap();
    assert_eq!(outcome.added, 13);

    let summary = summary(&storage, "2026-01-01", "2026-12-31");
    assert_eq!(summary.totals.call_count, 13);
    assert_eq!(summary.totals.input_total_known, Some(306_534));
    assert_eq!(summary.totals.output_total_known, Some(5_938));
    assert_eq!(summary.totals.cache_read_known, Some(272_896));
    assert_eq!(summary.totals.total_tokens_known, Some(312_472));

    let conn = storage.conn();
    // Preserve per-message models: k2p7 twice and glm-5.2 eleven times.
    let models: std::collections::BTreeMap<String, i64> = {
        let mut stmt = conn
            .prepare("SELECT model_raw, COUNT(*) FROM usage_events GROUP BY model_raw")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    assert_eq!(
        models,
        std::collections::BTreeMap::from([("glm-5.2".to_string(), 11), ("k2p7".to_string(), 2),])
    );

    // Keep two error calls with error_status=error, reported zero buckets, and no total.
    let (errors, missing_total): (i64, i64) = conn
        .query_row(
            "SELECT SUM(error_status = 'error'), SUM(source_total IS NULL) \
             FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(errors, 2, "两条 error 消息（fixture anon-4/anon-6）");
    assert_eq!(missing_total, 2, "missing_total_msgs=2");
    let zero_tokens: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE total_tokens = 0",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(zero_tokens, 2, "error 消息 tokens 全零是已知量");

    // Registered version 7.4.8 has one matched session comparison.
    assert_eq!(report.reconciliations.len(), 1);
    assert_eq!(report.reconciliations[0].detail_sum, 312_472);
    assert_eq!(report.reconciliations[0].verdict, "matched");
    let _ = dir;
}

#[test]
fn contract_781_toplevel_model_fields_parse_and_reconcile() {
    // Redacted native 7.8.1 session has top-level modelID/providerID k3-256k/kimi-code-owent.
    // Its 34 calls match the snapshot; registering the checked version removes latest_fallback.
    let dir = TempDir::new("kilo-781");
    let root = build_kilo_db_from_fixture(&dir, "session-7.8.1-k3.sanitized.json");
    let (_db, storage) = temp_storage("kilo-781");
    let reports = run_kilo(&storage, &root, NOW);

    let report = &reports[0];
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].records_seen, 35, "34 assistant + 1 user");
    assert_eq!(report.files[0].events, 34);

    let summary = summary(&storage, "2026-01-01", "2026-12-31");
    assert_eq!(summary.totals.call_count, 34);
    assert_eq!(summary.totals.input_total_known, Some(3_144_818));
    assert_eq!(summary.totals.output_total_known, Some(44_490));
    assert_eq!(summary.totals.cache_read_known, Some(3_019_520));
    assert_eq!(summary.totals.total_tokens_known, Some(3_189_308));

    let conn = storage.conn();
    let models: std::collections::BTreeMap<String, i64> = {
        let mut stmt = conn
            .prepare("SELECT model_raw, COUNT(*) FROM usage_events GROUP BY model_raw")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    assert_eq!(
        models,
        std::collections::BTreeMap::from([("k3-256k".to_string(), 34)])
    );
    // Store top-level providerID from the checked 7.8.1 records.
    let provider: String = conn
        .query_row("SELECT DISTINCT provider_id FROM usage_events", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(provider, "kimi-code-owent");

    // Registered 7.8.1 produces known_version and active health without active_compat.
    let file_status: String = conn
        .query_row("SELECT DISTINCT status FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(file_status, "active");

    // The session cumulative comparison is matched.
    assert_eq!(report.reconciliations.len(), 1);
    assert_eq!(report.reconciliations[0].detail_sum, 3_189_308);
    assert_eq!(report.reconciliations[0].verdict, "matched");
    // Simulate an old consumed cursor; new version support must reevaluate unchanged bytes.
    let before_revision = storage.data_revision().unwrap();
    storage.conn().execute_batch("UPDATE source_instances SET capabilities=json_set(capabilities,'$.supported_versions',json('[\"7.4.8\",\"7.4.9\"]'));
        UPDATE source_files SET status='active_compat';").unwrap();
    let replay = run_kilo(&storage, &root, NOW + 1);
    assert!(replay[0].files[0].records_seen > 0);
    let status: String = storage
        .conn()
        .query_row("SELECT status FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(status, "active");
    assert_eq!(
        common::summary(&storage, "2026-01-01", "2026-12-31")
            .totals
            .call_count,
        34
    );
    assert!(storage.data_revision().unwrap() >= before_revision);
    let _ = dir;
}

#[test]
fn empty_unverified_new_version_does_not_certify_the_source_or_change_known_usage() {
    let dir = TempDir::new("kilo-empty-unverified");
    let root = build_kilo_db_from_fixture(&dir, "session-7.8.1-k3.sanitized.json");
    let source = rusqlite::Connection::open(root.join(".local/share/kilo/kilo.db")).unwrap();
    source.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
    source.execute("INSERT INTO session(id,project_id,slug,directory,title,version,time_created,time_updated)
        VALUES('synthetic-empty','anon-proj','stub','/work','SYNTHETIC','7.8.3',?1,?1)",[NOW]).unwrap();
    let (_db, storage) = temp_storage("kilo-empty-unverified");
    let reports = run_kilo(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].events, 34);
    let (status, format): (String, String) = storage
        .conn()
        .query_row("SELECT status,format_status FROM source_files", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    let format: serde_json::Value = serde_json::from_str(&format).unwrap();
    assert_eq!(status, "active_compat");
    assert_eq!(format["found_version"], "7.8.3");
    assert_eq!(format["compat"], "unverified");
    assert_eq!(
        summary(&storage, "2026-01-01", "2026-12-31")
            .totals
            .total_tokens_known,
        Some(3_189_308)
    );
    let basis: String = storage
        .conn()
        .query_row("SELECT DISTINCT parse_basis FROM usage_events", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        basis, "known_version",
        "empty sessions do not downgrade independently verified records"
    );
}

#[test]
fn highest_verified_database_version_does_not_certify_older_unverified_messages() {
    let dir = TempDir::new("kilo-mixed-versions");
    let root = build_kilo_db_from_fixture(&dir, "session-7.8.1-k3.sanitized.json");
    let source = rusqlite::Connection::open(root.join(".local/share/kilo/kilo.db")).unwrap();
    source
        .execute_batch(
            "PRAGMA foreign_keys=OFF; UPDATE session SET version='7.4.7';
        INSERT INTO session(id,project_id,slug,directory,title,version,time_created,time_updated)
        VALUES('synthetic-empty','anon-proj','stub','/work','SYNTHETIC','7.8.1',0,0);",
        )
        .unwrap();
    let (_db, storage) = temp_storage("kilo-mixed-versions");
    run_kilo(&storage, &root, NOW);
    let data = serde_json::json!({"role":"assistant","finish":"stop","modelID":"k3-256k","providerID":"kimi-code-owent",
        "time":{"created":NOW,"completed":NOW},"tokens":{"input":1,"output":2,"reasoning":0,"cache":{"read":0,"write":0},"total":3}});
    source.execute("INSERT INTO message(id,session_id,time_created,time_updated,data) VALUES('synthetic-known','synthetic-empty',?1,?1,?2)",rusqlite::params![NOW,data.to_string()]).unwrap();
    source
        .execute(
            "UPDATE session SET tokens_input=1,tokens_output=2 WHERE id='synthetic-empty'",
            [],
        )
        .unwrap();
    for now in [NOW, NOW + 1] {
        run_kilo(&storage, &root, now + 1);
        let (status,basis):(String,String)=storage.conn().query_row(
            "SELECT f.status,e.parse_basis FROM source_files f JOIN usage_events e ON e.source_instance_id=f.instance_id WHERE e.schema_version='7.4.7' LIMIT 1",
            [],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
        assert_eq!(status, "active_compat");
        assert_eq!(basis, "latest_fallback");
        let sums = summary(&storage, "2026-01-01", "2027-12-31").totals;
        assert_eq!(sums.call_count, 35);
        assert_eq!(sums.total_tokens_known, Some(3_189_311));
    }
    let known:i64=storage.conn().query_row("SELECT COUNT(*) FROM usage_events WHERE schema_version='7.8.1' AND parse_basis='known_version'",[],|r|r.get(0)).unwrap();
    assert_eq!(
        known, 1,
        "verified messages keep their own evidence even in a mixed database"
    );
}

#[test]
fn capability_table_is_structured_and_complete() {
    use llm_usage_core::adapters::framework::SourceAdapter;
    let adapter = llm_usage_core::adapters::kilo::KiloAdapter::new();
    let cap = adapter.capability();
    let json = serde_json::to_value(&cap).unwrap();
    assert_eq!(json["adapter_id"], "kilo");
    assert_eq!(
        json["supported_versions"],
        serde_json::json!(["7.4.8", "7.4.9", "7.8.1"])
    );
    assert_eq!(json["discovery"]["env_override"], serde_json::Value::Null);
    for key in [
        "tokens",
        "cache_read",
        "cache_write",
        "per_request_calls",
        "model",
        "time",
        "cost",
        "latency",
    ] {
        assert!(
            json["fields"].get(key).is_some(),
            "capability fields missing {key}"
        );
    }
    assert_eq!(json["fields"]["tokens"]["availability"], "available");
    assert!(
        json["fields"]["cost"]["availability"]
            .get("unavailable")
            .is_some(),
        "cost 列存模型 JSON，不可用"
    );
    for section in [
        "discovery",
        "detection",
        "lifecycle",
        "incremental",
        "dedup",
        "integrity",
        "maintenance",
        "scheduling",
    ] {
        assert!(json.get(section).is_some(), "capability missing {section}");
    }
    assert!(!cap.limitations.is_empty());
}

#[test]
fn discover_finds_kilo_db_from_default_home_shape() {
    use llm_usage_core::adapters::framework::{DiscoverContext, SourceAdapter};
    let dir = TempDir::new("kilo-discover");
    build_kilo_db_from_fixture(&dir, "session-7.4.8-edges.sanitized.json");
    let adapter = llm_usage_core::adapters::kilo::KiloAdapter::new();

    // Default root derives <home>/.local/share/kilo from home_dir on Unix and Windows.
    let ctx = DiscoverContext {
        home_dir: Some(dir.path().to_path_buf()),
        env: Default::default(),
        manual_roots: vec![],
    };
    let roots = adapter.discover(&ctx);
    assert_eq!(roots.len(), 1);
    let instance_id = adapter.instance_id(&roots[0]);
    assert!(
        instance_id.starts_with("kilo@") && instance_id.contains(".local/share/kilo"),
        "实例身份来自发现的 kilo home：{instance_id}"
    );

    // Manual roots can be the Kilo directory, its parent, or the user home.
    for manual in [
        dir.path().join(".local").join("share").join("kilo"),
        dir.path().join(".local").join("share"),
        dir.path().to_path_buf(),
    ] {
        let ctx = DiscoverContext {
            home_dir: None,
            env: Default::default(),
            manual_roots: vec![manual.clone()],
        };
        let roots = adapter.discover(&ctx);
        assert_eq!(roots.len(), 1, "手工根 {manual:?} 应恰好发现一个 kilo.db");
        assert!(roots[0].files[0]
            .file_name()
            .is_some_and(|n| n == "kilo.db"));
        assert_eq!(
            roots[0].files[0].parent(),
            Some(
                dir.path()
                    .join(".local")
                    .join("share")
                    .join("kilo")
                    .as_path()
            )
        );
    }

    // Empty directories yield no root.
    let ctx = DiscoverContext {
        home_dir: Some(std::env::temp_dir()),
        env: Default::default(),
        manual_roots: vec![],
    };
    assert!(adapter.discover(&ctx).is_empty());
    let _ = dir;
}
