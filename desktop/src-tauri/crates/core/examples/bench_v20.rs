//! V20: real ingestion/query pipeline, 366 days, 50 models, 20 Agents,
//! repeated sessions and two large session keys per source. Output contains synthetic counts,
//! timings and database sizes; process memory is sampled externally.
//! Usage: bench_v20 <new_work_dir> [event_count]; --query-only reads an existing DB.
//! Add --prepare-indexes to install current optional indexes in a synthetic DB first.

use llm_usage_core::calendar::{Calendar, WeekStart};
use llm_usage_core::domain::*;
use llm_usage_core::ingest::IngestBatch;
use llm_usage_core::query::{query_summary, Filters, Granularity, SummaryRequest};
use llm_usage_core::storage::Storage;
use std::path::PathBuf;
use std::time::Instant;

fn event(instance: &str, i: u64, day: i64, model: &str, first_ms: i64) -> EventInput {
    let base = first_ms + day * 86_400_000;
    EventInput {
        source_instance_id: instance.to_string(),
        source_record_key: format!("bench-{i}"),
        record_kind: RecordKind::ModelCall,
        schema_version: "1".to_string(),
        parser_version: "bench".to_string(),
        parse_basis: None,
        origin_call_id: None,
        attempt_id: None,
        session_id: Some(if i % 5 == 0 {
            format!("large-{}", i % 2)
        } else {
            format!("sess-{}", i / 64)
        }),
        parent_session_id: None,
        host_application: None,
        agent: format!("agent-{}", (i / 50) % 20),
        call_category: CallCategory::Primary,
        occurred_at_ms: base + ((i % 86_400) * 1000) as i64,
        observed_at_ms: None,
        source_time: None,
        time_basis: TimeBasis::SourceCompletion,
        interval_start_ms: None,
        interval_end_ms: None,
        provider_id: Some("prov".to_string()),
        model_raw: Some(model.to_string()),
        model_canonical: None,
        model_attribution: ModelAttribution::RequestField,
        usage: TokenUsage {
            input_uncached: Some((i % 900) as i64),
            input_cache_read: Some((i % 5000) as i64),
            input_cache_write: None,
            input_total: Some(((i % 900) + (i % 5000)) as i64),
            output_total: Some((i % 400) as i64),
            output_reasoning: None,
            total_tokens: Some(((i % 900) + (i % 5000) + (i % 400)) as i64),
            source_total: None,
        },
        quality: TokenQuality {
            input_uncached: FieldQuality::Reported,
            input_cache_read: FieldQuality::Reported,
            input_cache_write: FieldQuality::Unknown,
            input_total: FieldQuality::Derived,
            output_total: FieldQuality::Reported,
            output_reasoning: FieldQuality::Unknown,
            total_tokens: FieldQuality::Derived,
            source_total: FieldQuality::Unknown,
        },
        lifecycle: Lifecycle::Final,
        source_revision: None,
        error_status: None,
        duration_ms: Some((i % 20_000) as i64),
        ttft_ms: None,
        attribution_status: AttributionStatus::Verified,
        exclusion_reason: None,
        cost: None,
    }
}

fn main() {
    let work = std::env::args()
        .nth(1)
        .expect("usage: bench_v20 <work_dir>");
    std::fs::create_dir_all(&work).unwrap();
    let db = PathBuf::from(&work).join("llm-usage.sqlite");
    let query_only = std::env::args().any(|arg| arg == "--query-only");
    if query_only {
        let guard = Storage::open_readonly(&db).expect("existing benchmark");
        let foreign: i64 = guard.conn().query_row("SELECT COUNT(*) FROM source_instances WHERE format IS NOT 'synthetic' OR parser_version IS NOT 'bench'",[],|r| r.get(0)).unwrap();
        assert_eq!(
            foreign, 0,
            "benchmark operations require synthetic/bench sources"
        );
        assert!(
            guard
                .conn()
                .query_row("SELECT COUNT(*) FROM source_instances", [], |r| r
                    .get::<_, i64>(0))
                .unwrap()
                > 0,
            "nonempty synthetic sources required"
        );
    }
    if query_only && std::env::args().any(|arg| arg == "--prepare-indexes") {
        let at = Instant::now();
        drop(Storage::open(&db).expect("prepare current indexes"));
        println!(
            "prepare current indexes ms: {:.2}",
            at.elapsed().as_secs_f64() * 1000.0
        );
    }
    assert!(
        query_only || !db.exists(),
        "use a new benchmark directory; existing databases are never overwritten"
    );
    let storage = if query_only {
        Storage::open_readonly(&db)
    } else {
        Storage::open(&db)
    }
    .expect("open");
    // 规模可由第二参数覆盖（默认 100 万；验收含 1,000 万档）。
    let total: u64 = std::env::args()
        .nth(2)
        .and_then(|v| v.parse().ok())
        .unwrap_or(1_000_000);

    const DAYS: i64 = 366;
    let models: Vec<String> = (0..50).map(|i| format!("model-{i}")).collect();
    let now_ms = jiff::Timestamp::now().as_millisecond();
    let calendar = Calendar::new("UTC").unwrap();
    let (first_day, today) = if query_only {
        let (first, last): (String, String) = storage
            .conn()
            .query_row(
                "SELECT MIN(local_day),MAX(local_day) FROM daily_usage WHERE tz_version='UTC'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        (
            llm_usage_core::calendar::parse_date(&first).unwrap(),
            llm_usage_core::calendar::parse_date(&last).unwrap(),
        )
    } else {
        let today = calendar.today(now_ms).unwrap();
        (
            today.checked_sub(jiff::Span::new().days(365)).unwrap(),
            today,
        )
    };
    let (first_ms, _) = calendar.day_range_ms(first_day).unwrap();
    if !query_only {
        // Assign the same ownership and timezone metadata a native application uses.
        for i in 0..20 {
            storage.conn().execute("INSERT INTO source_instances(instance_id,agent,locality_basis,attribution_status,enabled,format,parser_version,capabilities,health,user_id,created_at_ms,updated_at_ms) VALUES(?1,?2,'local_filesystem','verified',0,'synthetic','bench','{}','ok','default',?3,?3)", rusqlite::params![format!("inst-{i}"),format!("agent-{i}"),now_ms]).unwrap();
        }
        let settings = serde_json::json!({"timezone":"UTC","week_start":1,"language":"en","theme":"dark","refresh_interval_secs":0,"retention":{"events_days":366,"hourly_days":366,"daily_days":366,"weekly_days":1095,"monthly_days":3650,"yearly_days":null}});
        for (key, value) in [
            ("app_settings", settings.to_string()),
            ("hourly_fields_version:UTC", "1".into()),
        ] {
            storage.conn().execute("INSERT INTO settings(key,value,schema_version,updated_at_ms) VALUES(?1,?2,1,?3)",rusqlite::params![key,value,now_ms]).unwrap();
        }
        let t0 = Instant::now();
        let mut inserted = 0i64;
        for chunk_start in (0..total).step_by(50_000) {
            let mut batch = IngestBatch {
                batch_id: format!("bench-{chunk_start}"),
                instance_id: format!("inst-{}", (chunk_start / 50_000) % 20),
                timezone: "UTC".to_string(),
                now_ms,
                events: Vec::with_capacity(50_000),
                checkpoints: Vec::new(),
                diagnostics: Vec::new(),
                run_id: None,
                retention_cutoff_ms: None,
            };
            for i in chunk_start..(chunk_start + 50_000).min(total) {
                let day = ((i * DAYS as u64) / total.max(1)) as i64;
                batch.events.push(event(
                    &batch.instance_id,
                    i,
                    day,
                    &models[(i % 50) as usize],
                    first_ms,
                ));
            }
            let outcome = llm_usage_core::ingest::commit_batch(&storage, &batch, None).unwrap();
            inserted += outcome.added;
            println!("progress: {inserted}/{total}");
        }
        let insert_secs = t0.elapsed().as_secs_f64();
        println!(
            "inserted: {inserted} in {insert_secs:.1}s ({:.0}/s)",
            inserted as f64 / insert_secs
        );
    }
    // 库与 WAL 大小。
    let db_bytes = std::fs::metadata(&db).map(|m| m.len()).unwrap_or(0);
    let wal_bytes = std::fs::metadata(db.with_file_name("llm-usage.sqlite-wal"))
        .map(|m| m.len())
        .unwrap_or(0);
    println!(
        "db: {:.1} MiB, wal: {:.1} MiB",
        db_bytes as f64 / 1048576.0,
        wal_bytes as f64 / 1048576.0
    );
    // Exercise the actual dashboard query, including session and duration statistics.
    let request = SummaryRequest {
        timezone: "UTC".into(),
        week_start: WeekStart::Monday,
        first_day,
        last_day: today,
        granularity: Granularity::Day,
        filters: Filters::default(),
        today,
        retention_cutoff: None,
    };
    let mut samples = Vec::new();
    for _ in 0..20 {
        storage.clear_summary_cache();
        let t = Instant::now();
        let summary = query_summary(&storage, &request).unwrap();
        assert_eq!(summary.totals.call_count, total as i64);
        assert_eq!(summary.model_breakdown.len(), 50);
        assert_eq!(summary.agent_breakdown.len(), 20);
        samples.push(t.elapsed().as_micros() as f64 / 1000.0);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = |p: f64| samples[((samples.len() as f64 - 1.0) * p) as usize];
    println!(
        "uncached dashboard 366d/50 models/20 agents query ms: p50={:.2} p95={:.2} p99={:.2} max={:.2}",
        q(0.50),
        q(0.95),
        q(0.99),
        samples[samples.len() - 1]
    );
    let mut warm = Vec::new();
    for _ in 0..20 {
        let at = Instant::now();
        assert_eq!(
            query_summary(&storage, &request).unwrap().totals.call_count,
            total as i64
        );
        warm.push(at.elapsed().as_secs_f64() * 1000.0);
    }
    warm.sort_by(f64::total_cmp);
    println!(
        "cached dashboard query ms: p50={:.3} p95={:.3} max={:.3}",
        warm[9], warm[18], warm[19]
    );
    if std::env::args().any(|arg| arg == "--filtered") {
        for (name, filters, expected) in [
            (
                "model",
                Filters {
                    models: vec!["model-0".into()],
                    ..Filters::default()
                },
                (0..total).filter(|i| i % 50 == 0).count(),
            ),
            (
                "agent",
                Filters {
                    agents: vec!["agent-0".into()],
                    ..Filters::default()
                },
                (0..total).filter(|i| (i / 50) % 20 == 0).count(),
            ),
            (
                "provider",
                Filters {
                    providers: vec!["prov".into()],
                    ..Filters::default()
                },
                total as usize,
            ),
            (
                "model+agent",
                Filters {
                    models: vec!["model-0".into()],
                    agents: vec!["agent-0".into()],
                    ..Filters::default()
                },
                (0..total)
                    .filter(|i| i % 50 == 0 && (i / 50) % 20 == 0)
                    .count(),
            ),
        ] {
            let filtered = SummaryRequest {
                filters,
                ..request.clone()
            };
            let mut timings = Vec::new();
            for _ in 0..20 {
                storage.clear_summary_cache();
                let at = Instant::now();
                assert_eq!(
                    query_summary(&storage, &filtered)
                        .unwrap()
                        .totals
                        .call_count,
                    expected as i64
                );
                timings.push(at.elapsed().as_secs_f64() * 1000.0);
            }
            timings.sort_by(f64::total_cmp);
            println!(
                "filtered {name} query ms: p50={:.2} p95={:.2} max={:.2}",
                timings[9], timings[18], timings[19]
            );
        }
    }
    // 明细分页查询分位。
    let mut page_samples = Vec::new();
    for offset in (0..100).map(|i| i * 1000) {
        let t = Instant::now();
        let _ = storage
            .conn()
            .prepare(
                "SELECT event_id FROM usage_events ORDER BY occurred_at_ms LIMIT 200 OFFSET ?1",
            )
            .unwrap()
            .query_map([offset], |_| Ok(()))
            .unwrap()
            .count();
        page_samples.push(t.elapsed().as_micros() as f64 / 1000.0);
    }
    page_samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let qp = |p: f64| page_samples[((page_samples.len() as f64 - 1.0) * p) as usize];
    println!(
        "detail page (200 rows) ms: p50={:.2} p95={:.2} p99={:.2}",
        qp(0.50),
        qp(0.95),
        qp(0.99)
    );
    println!("V20 BENCH DONE");
}
