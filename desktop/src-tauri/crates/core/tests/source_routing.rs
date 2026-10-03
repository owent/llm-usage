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
