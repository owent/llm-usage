//! Read original Agent files; modify only a pre-created database under repo build/.
//! Print aggregate counts and pricing reasons, never paths or session/record IDs.
use llm_usage_core::adapters::copilot_chat::CopilotChatAdapter;
use llm_usage_core::adapters::framework::{
    run_adapter_scan, DiscoverContext, RunConfig, ScanLimits,
};
use llm_usage_core::adapters::otel::OtelAdapter;
use llm_usage_core::jobs::TriggerKind;
use llm_usage_core::pricing::EstimateOptions;
use llm_usage_core::storage::pricing::{CostFilters, CostSummaryRequest};
use llm_usage_core::storage::Storage;
use std::path::PathBuf;

fn snapshot(storage: &Storage, label: &str) {
    let (calls,observations,input,output,total,excluded):(i64,i64,Option<i64>,Option<i64>,Option<i64>,i64)=storage.conn().query_row(
        "SELECT COALESCE(SUM(record_kind='model_call' AND attribution_status='verified'),0),
            COALESCE(SUM(record_kind='usage_observation' AND attribution_status='verified'),0),
            SUM(CASE WHEN attribution_status='verified' THEN input_total END),
            SUM(CASE WHEN attribution_status='verified' THEN output_total END),
            SUM(CASE WHEN attribution_status='verified' THEN total_tokens END),
            COALESCE(SUM(attribution_status='excluded'),0) FROM usage_events WHERE agent='vscode-copilot-chat'",
        [],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).unwrap();
    println!("{label}: calls={calls} observations={observations} input={input:?} output={output:?} total={total:?} excluded={excluded}");
}
fn main() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../..")
        .canonicalize()
        .unwrap();
    let db = repo.join("build/dashboard-repair/real-snapshot.sqlite");
    assert!(
        db.is_file(),
        "create readonly SQLite backup in build/dashboard-repair first"
    );
    assert!(db
        .canonicalize()
        .unwrap()
        .starts_with(repo.join("build").canonicalize().unwrap()));
    let storage = Storage::open(&db).unwrap();
    storage
        .ensure_seed_price_snapshot(jiff::Timestamp::now().as_millisecond())
        .unwrap();
    let app = PathBuf::from(std::env::var_os("APPDATA").expect("Windows real verification"))
        .join("llm-usage-desktop");
    let roots = std::fs::read_dir(app.join("telemetry"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("copilot-vscode")
        })
        .map(|e| e.path())
        .filter(|p| p.join("events.jsonl").is_file())
        .collect::<Vec<_>>();
    let now = jiff::Timestamp::now().as_millisecond();
    let mut config = RunConfig {
        timezone: "Asia/Shanghai".into(),
        now_ms: now,
        limits: ScanLimits::default(),
        trigger: TriggerKind::Manual,
        run_id_prefix: "dashboard-real".into(),
        origin_host_id: storage.local_host_id().unwrap(),
    };
    assert!(
        config.origin_host_id.is_some(),
        "preserve local source namespace"
    );
    snapshot(&storage, "before");
    let ctx = DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: roots,
    };
    for pass in 0..2 {
        config.run_id_prefix = format!("dashboard-real-{now}-otel-{pass}");
        let reports = run_adapter_scan(&storage, &OtelAdapter::new(), &ctx, &config).unwrap();
        let (mut added, mut errors) = (0, 0);
        for report in reports {
            assert!(report.error.is_none());
            if let Some(o) = report.outcome {
                added += o.added;
                errors += o.errors + o.conflicts;
            }
        }
        assert_eq!(errors, 0);
        println!("otel-pass-{pass}: added={added} errors={errors}");
        if pass == 1 {
            assert_eq!(added, 0);
        }
    }
    snapshot(&storage, "after-otel");
    let home = PathBuf::from(std::env::var_os("USERPROFILE").unwrap());
    let ctx = DiscoverContext {
        home_dir: Some(home),
        env: std::env::vars().filter(|(k, _)| k == "APPDATA").collect(),
        manual_roots: vec![],
    };
    let revision = storage.data_revision().unwrap();
    config.run_id_prefix = format!("dashboard-real-{now}-native");
    let reports = run_adapter_scan(&storage, &CopilotChatAdapter::new(), &ctx, &config).unwrap();
    assert!(reports.iter().all(|r| r.error.is_none()));
    println!(
        "native-rescan: revision_before={revision} revision_after={}",
        storage.data_revision().unwrap()
    );
    snapshot(&storage, "after-native-rescan");
    storage
        .ensure_cost_matching_policy("Asia/Shanghai", now, &EstimateOptions::default())
        .unwrap();
    let mut stmt=storage.conn().prepare("SELECT currency,SUM(total_amount_minor),SUM(priced_event_count),SUM(unpriced_event_count),SUM(fallback_event_count)
        FROM daily_cost_usage WHERE kind='estimate_at_time' GROUP BY currency").unwrap();
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })
        .unwrap();
    for row in rows {
        println!("prices: {:?}", row.unwrap());
    }
    let summary = storage
        .cost_summary(&CostSummaryRequest {
            timezone: "Asia/Shanghai".into(),
            first_day: "2026-09-01".into(),
            last_day: "2026-10-02".into(),
            filters: CostFilters::default(),
            now_ms: now,
            options: EstimateOptions::default(),
        })
        .unwrap();
    println!(
        "current_reference_unpriced: {:?}",
        summary.current_sim.unpriced_reasons
    );
    println!("VERDICT: PASS (isolated snapshot, local Agent files only)");
}
