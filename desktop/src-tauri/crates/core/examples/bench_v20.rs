//! V20 性能初值基准：合成 100 万事件（366 日/多模型/多源/少量超大会话），
//! 测量插入吞吐、查询分位数、库与 WAL 大小、进程内存（工作集按 OS 报告口径
//! 由外部读取；本进程以 RSS 近似：Windows 上用 taskmgr 不可行，改由
//! psapi 工作集——示例输出 self_reported_rss_via_allocator 不做，记录
//! 数据库侧指标 + 查询分位数，进程内存由验收脚本另测）。
//! 用法：cargo run --release -p llm-usage-core --example bench_v20 -- <work_dir>

use llm_usage_core::domain::*;
use llm_usage_core::ingest::IngestBatch;
use llm_usage_core::storage::Storage;
use std::path::PathBuf;
use std::time::Instant;

fn event(instance: &str, i: u64, day: i64, model: &str) -> EventInput {
    let base = 1_700_000_000_000i64 + day * 86_400_000;
    EventInput {
        source_instance_id: instance.to_string(),
        source_record_key: format!("bench-{i}"),
        record_kind: RecordKind::ModelCall,
        schema_version: "1".to_string(),
        parser_version: "bench".to_string(),
        parse_basis: None,
        origin_call_id: None,
        attempt_id: None,
        session_id: Some(format!("sess-{i} / 64")),
        parent_session_id: None,
        host_application: None,
        agent: format!("agent-{}", i % 6),
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
    let db = PathBuf::from(&work).join("bench-v20.sqlite");
    let _ = std::fs::remove_file(&db);
    let _ = std::fs::remove_file(db.with_file_name("bench-v20.sqlite-wal"));
    let storage = Storage::open(&db).expect("open");
    // 规模可由第二参数覆盖（默认 100 万；验收含 1,000 万档）。
    let total: u64 = std::env::args()
        .nth(2)
        .and_then(|v| v.parse().ok())
        .unwrap_or(1_000_000);

    const DAYS: i64 = 366;
    let models = ["m-a", "m-b", "m-c", "m-d"];
    let t0 = Instant::now();
    let mut inserted = 0i64;
    for chunk_start in (0..total).step_by(50_000) {
        let mut batch = IngestBatch {
            batch_id: format!("bench-{chunk_start}"),
            instance_id: format!("inst-{}", chunk_start % 4),
            timezone: "UTC".to_string(),
            now_ms: 1_800_000_000_000,
            events: Vec::with_capacity(50_000),
            checkpoints: Vec::new(),
            diagnostics: Vec::new(),
            run_id: None,
            retention_cutoff_ms: None,
        };
        for i in chunk_start..(chunk_start + 50_000).min(total) {
            let day = ((i * DAYS as u64) / total.max(1)) as i64;
            batch
                .events
                .push(event(&batch.instance_id, i, day, models[(i % 4) as usize]));
        }
        let outcome = llm_usage_core::ingest::commit_batch(&storage, &batch, None).unwrap();
        inserted += outcome.added;
    }
    let insert_secs = t0.elapsed().as_secs_f64();
    println!(
        "inserted: {inserted} in {insert_secs:.1}s ({:.0}/s)",
        inserted as f64 / insert_secs
    );
    // 库与 WAL 大小。
    let db_bytes = std::fs::metadata(&db).map(|m| m.len()).unwrap_or(0);
    let wal_bytes = std::fs::metadata(db.with_file_name("bench-v20.sqlite-wal"))
        .map(|m| m.len())
        .unwrap_or(0);
    println!(
        "db: {:.1} MiB, wal: {:.1} MiB",
        db_bytes as f64 / 1048576.0,
        wal_bytes as f64 / 1048576.0
    );
    // 查询分位数：日汇总查询 100 次取分位。
    let mut samples = Vec::new();
    for _ in 0..100 {
        let t = Instant::now();
        let _n: i64 = storage
            .conn()
            .query_row("SELECT COUNT(*) FROM daily_usage", [], |r| r.get(0))
            .unwrap();
        let _agg: i64 = storage.conn().query_row(
            "SELECT SUM(total_tokens) FROM daily_usage WHERE local_day BETWEEN '2023-01-01' AND '2023-12-31'",
            [],
            |r| r.get(0),
        ).unwrap_or(0);
        samples.push(t.elapsed().as_micros() as f64 / 1000.0);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = |p: f64| samples[((samples.len() as f64 - 1.0) * p) as usize];
    println!(
        "daily-range query ms: p50={:.2} p95={:.2} p99={:.2} max={:.2}",
        q(0.50),
        q(0.95),
        q(0.99),
        samples[samples.len() - 1]
    );
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
