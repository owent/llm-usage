mod common;
use common::{temp_storage, ts};
use llm_usage_core::adapters::{
    built_in_adapters,
    framework::{
        normalize_path, run_adapter_scan, DiscoverContext, RunConfig, ScanLimits, SourceAdapter,
    },
    omp::OmpAdapter,
    otel::OtelAdapter,
    pi::PiAdapter,
    routing::{context_for_adapter, retire_misrouted_sources},
};
use llm_usage_core::{jobs::TriggerKind, storage::Storage};
use rusqlite::params;
use serde_json::json;
use std::path::Path;

fn config(id: &str) -> RunConfig {
    RunConfig {
        timezone: "UTC".into(),
        now_ms: ts("2026-10-03T12:00:00Z"),
        limits: ScanLimits::default(),
        trigger: TriggerKind::Manual,
        run_id_prefix: id.into(),
        origin_host_id: None,
    }
}

#[test]
fn verified_zed_file_routes_registry_and_retires_empty_legacy_parent_instances() {
    use llm_usage_core::adapters::routing::retire_misrouted_zed_sources;
    let (dir, s) = temp_storage("zed-registry-routing");
    let file = dir.path().join("threads.db");
    let conn = rusqlite::Connection::open(&file).unwrap();
    conn.execute("CREATE TABLE threads(id TEXT PRIMARY KEY,summary TEXT,updated_at TEXT,data_type TEXT,data BLOB,created_at TEXT)", []).unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/zed/native-1.22.0.json")).unwrap();
    for row in fixture["threads"].as_array().unwrap() {
        conn.execute(
            "INSERT INTO threads VALUES(?1,'synthetic',?2,'json',?3,'2026-10-07T03:38:00Z')",
            params![
                row["id"].as_str().unwrap(),
                row["updated_at"].as_str().unwrap(),
                serde_json::to_vec(row).unwrap()
            ],
        )
        .unwrap();
    }
    drop(conn);
    let ctx = DiscoverContext {
        manual_roots: vec![file.clone()],
        ..Default::default()
    };
    let mut legacy = 0;
    for adapter in built_in_adapters() {
        if adapter.adapter_id() == "zed" {
            continue;
        }
        for root in adapter.discover(&ctx) {
            register(
                &s,
                &adapter.instance_id(&root),
                adapter.adapter_id(),
                &root.root,
            );
            legacy += 1;
        }
    }
    assert!(legacy >= 2, "exercise real DSH and MiMo parent discovery");
    register(
        &s,
        "unrelated-error",
        "mimo-code",
        &dir.path().join("other"),
    );
    s.conn().execute("INSERT INTO diagnostics(instance_id,code,message,created_ms) VALUES('unrelated-error','bad_file','retained evidence',0)",[]).unwrap();
    for pass in 0..2 {
        retire_misrouted_zed_sources(&s, &ctx).unwrap();
        for adapter in built_in_adapters() {
            let routed = context_for_adapter(&ctx, adapter.adapter_id(), &[]);
            let reports = run_adapter_scan(
                &s,
                adapter.as_ref(),
                &routed,
                &config(&format!("zed-route-{pass}")),
            )
            .unwrap();
            if adapter.adapter_id() != "zed" {
                assert!(reports.is_empty(), "{}", adapter.adapter_id());
            }
        }
        assert_eq!(count(&s, "source_files"), 1);
        assert_eq!(count(&s, "usage_events"), 0);
        assert_eq!(count(&s, "source_aggregates"), 2);
        assert_eq!(
            s.conn()
                .query_row(
                    "SELECT COUNT(*) FROM source_instances WHERE health='not_applicable'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            legacy
        );
        assert_eq!(
            s.conn()
                .query_row(
                    "SELECT health FROM source_instances WHERE instance_id='unrelated-error'",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            "error"
        );
        assert_eq!(count(&s, "diagnostics"), 1);
    }
    let bad = dir.path().join("bad/threads.db");
    std::fs::create_dir_all(bad.parent().unwrap()).unwrap();
    std::fs::write(&bad, b"not sqlite").unwrap();
    let ctx = DiscoverContext {
        manual_roots: vec![bad.clone()],
        ..Default::default()
    };
    assert_eq!(
        context_for_adapter(&ctx, "mimo-code", &[]).manual_roots,
        vec![bad]
    );
}
fn telemetry(root: &Path) {
    std::fs::create_dir_all(root).unwrap();
    let span = json!({"name":"chat claude-opus-4.8","kind":2,"spanId":"span","traceId":"trace",
        "startTime":[1790985600,0],"resource":{"attributes":{"service.name":"copilot-chat"}},
        "attributes":{"copilot_chat.chat_session_id":"session","gen_ai.request.model":"claude-opus-4.8",
        "gen_ai.usage.input_tokens":100,"gen_ai.usage.output_tokens":20}});
    std::fs::write(
        root.join("events.jsonl"),
        format!("{}\n{span}\n", json!({"resource":{},"scopeMetrics":[]})),
    )
    .unwrap();
}
fn count(s: &Storage, table: &str) -> i64 {
    s.conn()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}
fn register(s: &Storage, id: &str, format: &str, root: &Path) {
    s.conn().execute("INSERT INTO source_instances(instance_id,agent,format,location_hint,locality_basis,attribution_status,health,created_at_ms,updated_at_ms)
        VALUES(?1,?2,?2,?3,'local_filesystem','verified','error',0,0)",params![id,format,normalize_path(root)]).unwrap();
}

#[test]
fn managed_telemetry_runs_through_entire_registry_without_phantom_agents() {
    let (dir, s) = temp_storage("routed-registry");
    let root = dir.path().join("telemetry");
    telemetry(&root);
    let base = DiscoverContext::default();
    for pass in 0..2 {
        for adapter in built_in_adapters() {
            let ctx = context_for_adapter(&base, adapter.adapter_id(), std::slice::from_ref(&root));
            let reports = run_adapter_scan(
                &s,
                adapter.as_ref(),
                &ctx,
                &config(&format!("{pass}-{}", adapter.adapter_id())),
            )
            .unwrap();
            if adapter.adapter_id() != "otel" {
                assert!(reports.is_empty(), "{}", adapter.adapter_id());
            }
            for report in reports {
                assert!(report.error.is_none(), "{:?}", report.error);
            }
        }
        assert_eq!(count(&s, "source_instances"), 1);
        assert_eq!(count(&s, "source_files"), 1);
        assert_eq!(count(&s, "usage_events"), 1);
        let totals: (i64, i64) = s
            .conn()
            .query_row(
                "SELECT input_total, total_tokens FROM usage_events",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(totals, (100, 120));
    }
}

#[test]
fn legacy_wrong_registration_recovers_without_clearing_history_or_diagnostics() {
    let (dir, s) = temp_storage("routing-recovery");
    let root = dir.path().join("telemetry");
    telemetry(&root);
    register(&s, "wrong-pi", "pi", &root);
    register(&s, "wrong-omp", "omp", &root);
    s.conn().execute("INSERT INTO source_files(file_id,instance_id,file_identity,status,content_hash,first_seen_ms,last_seen_ms)
        VALUES(?1,'wrong-pi','legacy-unrecognized','unsupported','0:0:0:-1',0,0)",[normalize_path(&root.join("events.jsonl"))]).unwrap();
    s.conn().execute("INSERT INTO diagnostics(instance_id,code,message,created_ms) VALUES('wrong-pi','unknown_format','historical evidence',0)",[]).unwrap();
    s.conn()
        .execute(
            "UPDATE source_instances SET enabled=0 WHERE instance_id='wrong-omp'",
            [],
        )
        .unwrap();
    let ctx = DiscoverContext {
        manual_roots: vec![root.clone()],
        ..Default::default()
    };
    for pass in 0..2 {
        retire_misrouted_sources(&s, std::slice::from_ref(&root)).unwrap();
        let reports = run_adapter_scan(
            &s,
            &OtelAdapter::new(),
            &ctx,
            &config(&format!("repair-{pass}")),
        )
        .unwrap();
        assert!(reports.iter().all(|r| r.error.is_none()));
        assert_eq!(count(&s, "usage_events"), 1);
        assert_eq!(
            s.conn()
                .query_row(
                    "SELECT COUNT(*) FROM source_instances WHERE health='not_applicable'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            2
        );
        assert_eq!(
            s.conn()
                .query_row(
                    "SELECT COUNT(*) FROM diagnostics WHERE code='unknown_format'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
    }
    assert_eq!(
        s.conn()
            .query_row(
                "SELECT enabled FROM source_instances WHERE instance_id='wrong-omp'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    let owner: String = s
        .conn()
        .query_row(
            "SELECT i.format FROM source_files f JOIN source_instances i USING(instance_id)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(owner, "otel");
}

#[test]
fn shared_manual_root_respects_an_existing_owner() {
    let (dir, s) = temp_storage("manual-format");
    let root = dir.path().join("manual");
    telemetry(&root);
    let ctx = DiscoverContext {
        manual_roots: vec![root.clone()],
        ..Default::default()
    };
    run_adapter_scan(&s, &OtelAdapter::new(), &ctx, &config("otel-owner")).unwrap();
    for adapter in [&PiAdapter::new() as &dyn SourceAdapter, &OmpAdapter::new()] {
        assert!(
            run_adapter_scan(&s, adapter, &ctx, &config(adapter.adapter_id()))
                .unwrap()
                .is_empty()
        );
    }
    assert_eq!(count(&s, "source_instances"), 1);
    // pi and omp share a session shape; the first verified owner keeps the file.
    let session = json!({"type":"session","version":3,"id":"s","timestamp":"2026-10-03T00:00:00Z"});
    std::fs::write(root.join("session.jsonl"), format!("{session}\n")).unwrap();
    run_adapter_scan(&s, &PiAdapter::new(), &ctx, &config("pi-real")).unwrap();
    assert!(
        run_adapter_scan(&s, &OmpAdapter::new(), &ctx, &config("omp-duplicate"))
            .unwrap()
            .is_empty()
    );
    assert_eq!(count(&s, "source_instances"), 2);
}

#[test]
fn retained_periods_prevent_retirement_or_file_transfer() {
    let (dir, s) = temp_storage("routing-retained");
    let root = dir.path().join("telemetry");
    telemetry(&root);
    register(&s, "history", "pi", &root);
    s.conn().execute("INSERT INTO period_usage(tz_version,granularity,period_key,period_start_day,period_end_day,instance_id,agent,call_category,quality_bucket,event_count,call_count,conflict_count,active_days,materialized_at_ms,data_revision)
        VALUES('UTC','month','2026-09','2026-09-01','2026-09-30','history','pi','primary','reported',1,1,0,1,0,1)",[]).unwrap();
    s.conn().execute("INSERT INTO source_files(file_id,instance_id,file_identity,status,content_hash,first_seen_ms,last_seen_ms)
        VALUES(?1,'history','old','unsupported','0:0:0:-1',0,0)",[normalize_path(&root.join("events.jsonl"))]).unwrap();
    retire_misrouted_sources(&s, std::slice::from_ref(&root)).unwrap();
    let ctx = DiscoverContext {
        manual_roots: vec![root],
        ..Default::default()
    };
    let reports = run_adapter_scan(&s, &OtelAdapter::new(), &ctx, &config("preserve")).unwrap();
    assert!(reports.iter().all(|r| r.error.is_none()));
    assert_eq!(count(&s, "usage_events"), 0);
    let owner: String = s
        .conn()
        .query_row("SELECT instance_id FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(owner, "history");
    let health: String = s
        .conn()
        .query_row(
            "SELECT health FROM source_instances WHERE instance_id='history'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(health, "error");
    assert_eq!(count(&s, "period_usage"), 1);
}

#[test]
fn legacy_registry_discovery_recovers_transformed_roots_without_retiring_real_sources() {
    use llm_usage_core::adapters::junie::JunieAdapter;
    let (dir, s) = temp_storage("routing-transformed");
    let managed = dir.path().join("telemetry").join("copilot-vscode");
    telemetry(&managed);
    let legacy = DiscoverContext {
        manual_roots: vec![managed.clone()],
        ..Default::default()
    };
    for adapter in built_in_adapters() {
        if adapter.adapter_id() == "otel" {
            continue;
        }
        for root in adapter.discover(&legacy) {
            register(
                &s,
                &adapter.instance_id(&root),
                adapter.adapter_id(),
                &root.root,
            );
        }
    }
    let junie = JunieAdapter::new().discover(&legacy);
    assert_eq!(junie[0].root, managed.parent().unwrap());
    register(
        &s,
        "real-junie",
        "junie",
        &dir.path().join(".junie/sessions"),
    );
    let original = count(&s, "source_instances");
    assert!(original > 2);
    for _ in 0..2 {
        retire_misrouted_sources(&s, std::slice::from_ref(&managed)).unwrap();
        assert_eq!(
            count(&s, "source_instances"),
            original,
            "preserve registrations and settings for audit"
        );
        let visible: Vec<String> = s
            .conn()
            .prepare("SELECT instance_id FROM source_instances WHERE health!='not_applicable'")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(
            visible,
            ["real-junie"],
            "recover every old discovered path, leave a genuine source error visible"
        );
    }
}

fn qwen_native(root: &Path) -> std::path::PathBuf {
    let chats = root.join("projects/local/chats");
    std::fs::create_dir_all(&chats).unwrap();
    let path = chats.join("native.jsonl");
    std::fs::write(
        &path,
        include_str!("fixtures/otel/real-qwen-0.25.0-sdk/native.jsonl"),
    )
    .unwrap();
    path
}

#[test]
fn genuine_qwen_manual_root_recovers_a_consumed_wrong_claude_owner_through_registry() {
    use llm_usage_core::adapters::{claude::ClaudeAdapter, routing::retire_misrouted_qwen_sources};
    let (dir, s) = temp_storage("qwen-manual-owner");
    let root = dir.path().join(".qwen");
    let file = qwen_native(&root);
    let ctx = DiscoverContext {
        manual_roots: vec![root.clone()],
        ..Default::default()
    };
    // Reproduce the actual old type-only detector and consumed, unchanged cursor.
    run_adapter_scan(&s, &ClaudeAdapter::new(), &ctx, &config("old-claude")).unwrap();
    assert_eq!(count(&s, "usage_events"), 0);
    assert_eq!(count(&s, "ingestion_checkpoints"), 1);
    let wrong: String = s
        .conn()
        .query_row(
            "SELECT instance_id FROM source_files WHERE file_id=?1",
            [normalize_path(&file)],
            |r| r.get(0),
        )
        .unwrap();
    s.conn().execute("INSERT INTO diagnostics(instance_id,code,message,created_ms) VALUES(?1,'old-routing','historical evidence',0)",[&wrong]).unwrap();
    s.conn()
        .execute(
            "UPDATE source_instances SET enabled=0 WHERE instance_id=?1",
            [&wrong],
        )
        .unwrap();
    let before = std::fs::read(&file).unwrap();
    for pass in 0..2 {
        retire_misrouted_qwen_sources(&s, &ctx).unwrap();
        for adapter in built_in_adapters() {
            let routed = context_for_adapter(&ctx, adapter.adapter_id(), &[]);
            let reports = run_adapter_scan(
                &s,
                adapter.as_ref(),
                &routed,
                &config(&format!("qwen-{pass}-{}", adapter.adapter_id())),
            )
            .unwrap();
            if adapter.adapter_id() != "qwen" {
                assert!(reports.is_empty(), "{}", adapter.adapter_id());
            }
            assert!(reports.iter().all(|r| r.error.is_none()));
        }
        let values: (i64, i64) = s
            .conn()
            .query_row(
                "SELECT COUNT(*),SUM(total_tokens) FROM usage_events",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(values, (1, 10228));
        assert_eq!(count(&s, "source_files"), 1);
        assert_eq!(count(&s, "source_instances"), 2);
        assert_eq!(count(&s, "ingestion_checkpoints"), 1);
        let old: (bool, String) = s
            .conn()
            .query_row(
                "SELECT enabled,health FROM source_instances WHERE instance_id=?1",
                [&wrong],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(old, (false, "not_applicable".into()));
        assert_eq!(
            s.conn()
                .query_row(
                    "SELECT COUNT(*) FROM diagnostics WHERE code='old-routing'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        assert_eq!(std::fs::read(&file).unwrap(), before);
    }
}

#[test]
fn qwen_routing_preserves_retained_history_and_reports_real_bad_lines() {
    use llm_usage_core::adapters::{
        claude::ClaudeAdapter, qwen::QwenAdapter, routing::retire_misrouted_qwen_sources,
    };
    for retained in [false, true] {
        let (dir, s) = temp_storage("qwen-routing-boundary");
        let root = dir.path().join(".qwen");
        let file = qwen_native(&root);
        let ctx = DiscoverContext {
            manual_roots: vec![root.clone()],
            ..Default::default()
        };
        if retained {
            run_adapter_scan(&s, &ClaudeAdapter::new(), &ctx, &config("retained-claude")).unwrap();
            let id: String = s
                .conn()
                .query_row("SELECT instance_id FROM source_files", [], |r| r.get(0))
                .unwrap();
            s.conn().execute("INSERT INTO period_usage(tz_version,granularity,period_key,period_start_day,period_end_day,instance_id,agent,call_category,quality_bucket,event_count,call_count,conflict_count,active_days,materialized_at_ms,data_revision) VALUES('UTC','month','2026-09','2026-09-01','2026-09-30',?1,'claude-code','primary','reported',1,1,0,1,0,1)",[&id]).unwrap();
            retire_misrouted_qwen_sources(&s, &ctx).unwrap();
            run_adapter_scan(&s, &QwenAdapter::new(), &ctx, &config("retained-qwen")).unwrap();
            assert_eq!(count(&s, "usage_events"), 0);
            assert_eq!(count(&s, "period_usage"), 1);
            assert_eq!(
                s.conn()
                    .query_row("SELECT instance_id FROM source_files", [], |r| r
                        .get::<_, String>(0))
                    .unwrap(),
                id
            );
        } else {
            use std::io::Write;
            std::fs::OpenOptions::new()
                .append(true)
                .open(&file)
                .unwrap()
                .write_all(b"{broken}\n")
                .unwrap();
            let routed = context_for_adapter(&ctx, "qwen", &[]);
            let reports =
                run_adapter_scan(&s, &QwenAdapter::new(), &routed, &config("broken-qwen")).unwrap();
            assert_eq!(reports.len(), 1);
            assert!(count(&s, "diagnostics") > 0);
            assert!(
                s.conn()
                    .query_row(
                        "SELECT health FROM source_instances WHERE agent='qwen-code'",
                        [],
                        |r| r.get::<_, String>(0)
                    )
                    .unwrap()
                    != "ok"
            );
            assert_eq!(
                context_for_adapter(&ctx, "claude", &[]).manual_roots.len(),
                0
            );
        }
    }
}

#[test]
fn a_dot_qwen_directory_name_and_shared_type_do_not_prove_qwen_format() {
    let (dir, _s) = temp_storage("qwen-name-only");
    let root = dir.path().join(".qwen");
    let chats = root.join("projects/local/chats");
    std::fs::create_dir_all(&chats).unwrap();
    let record = json!({"type":"user","uuid":"u","sessionId":"s","timestamp":"2026-10-05T00:00:00Z","message":{"role":"user","content":[]}});
    std::fs::write(chats.join("claude.jsonl"), format!("{record}\n")).unwrap();
    let ctx = DiscoverContext {
        manual_roots: vec![root],
        ..Default::default()
    };
    assert_eq!(
        context_for_adapter(&ctx, "claude", &[]).manual_roots,
        ctx.manual_roots
    );
}

#[test]
fn a_non_sqlite_sdk_manual_file_is_diagnosed_then_claimed_by_otel() {
    use llm_usage_core::adapters::goose::GooseAdapter;
    let (dir, s) = temp_storage("sdk-manual-file");
    let file = dir.path().join("telemetry.json");
    std::fs::write(
        &file,
        include_str!("fixtures/otel/real-qwen-0.25.0-sdk/telemetry.json"),
    )
    .unwrap();
    let ctx = DiscoverContext {
        manual_roots: vec![file],
        ..Default::default()
    };
    let report = run_adapter_scan(&s, &GooseAdapter::new(), &ctx, &config("not-goose")).unwrap();
    assert!(report.iter().all(|r| r.error.is_none()));
    assert!(count(&s, "diagnostics") > 0);
    run_adapter_scan(&s, &OtelAdapter::new(), &ctx, &config("sdk-owner")).unwrap();
    assert_eq!(count(&s, "usage_events"), 2);
    assert_eq!(count(&s, "source_files"), 1);
    let owner: String = s
        .conn()
        .query_row(
            "SELECT i.format FROM source_files f JOIN source_instances i USING(instance_id)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(owner, "otel");
    assert_eq!(count(&s, "ingestion_checkpoints"), 1);
    assert!(
        run_adapter_scan(&s, &GooseAdapter::new(), &ctx, &config("owned-sdk"))
            .unwrap()
            .is_empty()
    );
}
