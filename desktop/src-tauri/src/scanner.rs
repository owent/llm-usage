//! 采集运行器：内置适配器注册表 + 全源刷新 + 间隔调度循环（M6）。
//! 合同：只读取用户启用的本地来源；定时任务只运行本应用采集逻辑；
//! 同一时刻仅一个刷新在执行（重复触发合并，V12/V23）。

use crate::app_state::{summarize_reports, AppState, RefreshInstanceSummary};
use llm_usage_core::adapters::claude::ClaudeAdapter;
use llm_usage_core::adapters::codex::CodexAdapter;
use llm_usage_core::adapters::framework::{
    run_adapter_scan, DiscoverContext, RunConfig, ScanLimits, SourceAdapter,
};
use llm_usage_core::adapters::gemini::GeminiAdapter;
use llm_usage_core::adapters::omp::OmpAdapter;
use llm_usage_core::adapters::pi::PiAdapter;
use llm_usage_core::adapters::qwen::QwenAdapter;
use llm_usage_core::jobs::TriggerKind;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// 内置适配器注册表：新增适配器在此登记（目录合同见 architecture.md#adapter-layout）。
pub fn built_in_adapters() -> Vec<Box<dyn SourceAdapter>> {
    vec![
        Box::new(CodexAdapter::new()),
        Box::new(ClaudeAdapter::new()),
        Box::new(PiAdapter::new()),
        Box::new(OmpAdapter::new()),
        Box::new(GeminiAdapter::new()),
        Box::new(QwenAdapter::new()),
        Box::new(llm_usage_core::adapters::kilo::KiloAdapter::new()),
        Box::new(llm_usage_core::adapters::zcode::ZcodeAdapter::new()),
        Box::new(llm_usage_core::adapters::kimi_code::KimiCodeAdapter::new()),
        Box::new(llm_usage_core::adapters::kimi_work::KimiWorkAdapter::new()),
        // 以下为本机未安装产品（2026-09-25 盘点 not_found）：文档级证据实现，
        // discover 在本机返回空；真实数据出现后自动发现（真实验收后置）。
        Box::new(llm_usage_core::adapters::cline::ClineAdapter::new()),
        Box::new(llm_usage_core::adapters::dsh::DshAdapter::new()),
        Box::new(llm_usage_core::adapters::hermes::HermesAdapter::new()),
        Box::new(llm_usage_core::adapters::openclaw::OpenClawAdapter::new()),
        Box::new(llm_usage_core::adapters::opencode::OpenCodeAdapter::new()),
        Box::new(llm_usage_core::adapters::mimo_code::MimoCodeAdapter::new()),
        Box::new(llm_usage_core::adapters::zoo::ZooAdapter::new()),
    ]
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn discover_context(manual_roots: Vec<String>) -> DiscoverContext {
    DiscoverContext {
        home_dir: std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .ok()
            .map(std::path::PathBuf::from),
        env: std::env::vars().collect(),
        manual_roots: manual_roots
            .into_iter()
            .map(std::path::PathBuf::from)
            .collect(),
    }
}

/// 执行一次全源刷新（已启用来源）。已在执行时直接返回 false（合并触发，不并发）。
pub fn run_refresh(state: &Arc<AppState>, trigger: TriggerKind) -> bool {
    {
        let refresh = state.refresh.lock().unwrap();
        if refresh.running {
            return false;
        }
    }
    let (host_id, manual_roots, retention, timezone) = {
        let settings = state.settings.lock().unwrap();
        let host = state.host_id.lock().unwrap().clone();
        (
            host,
            settings.manual_roots.clone(),
            settings.retention.clone(),
            settings.timezone.clone(),
        )
    };
    {
        let mut refresh = state.refresh.lock().unwrap();
        refresh.running = true;
        refresh.started_ms = now_ms();
        refresh.trigger = trigger.as_str().to_string();
        refresh.instances.clear();
    }
    let now = now_ms();
    // 日汇总分区用用户统计时区（V04/V12：日界随用户时区；此前误用 UTC 导致
    // UI 按本地时区查询永远为空——2026-09-26 修复，验证记录见 m6 修订）。
    let config = RunConfig {
        timezone: timezone.clone(),
        now_ms: now,
        limits: ScanLimits::default(),
        trigger,
        run_id_prefix: format!("scan-{now}"),
        origin_host_id: Some(host_id),
    };
    let ctx = discover_context(manual_roots);
    let mut summaries: Vec<RefreshInstanceSummary> = Vec::new();
    // 注册表变更后重扫：active_compat 文件的游标已推进，不会被 unchanged 短路
    // 重新检测——主动清除其游标与状态让下轮全量重扫（幂等，事件去重保证不双计）。
    {
        let storage = state.storage.lock().unwrap();
        let n = storage
            .conn()
            .execute(
                "DELETE FROM ingestion_checkpoints WHERE scope_key IN (
                   SELECT file_identity FROM source_files WHERE status = 'active_compat'
                 )",
                [],
            )
            .unwrap_or(0);
        if n > 0 {
            let _ = storage.conn().execute(
                "UPDATE source_files SET status = 'new' WHERE status = 'active_compat'",
                [],
            );
            eprintln!("reset {n} compat files for re-detection (registry may have changed)");
        }
    }
    // 写锁按适配器分段获取（而非整个刷新持有）：查询走 WAL 只读连接不受影响，
    // 设置写入等其他写操作可在适配器之间交错；任意时刻仍只有一个写者。
    let total_adapters = 17usize; // built_in_adapters().len()；硬编码避免双重枚举
    let scan_start = now_ms();
    for (adapter_index, adapter) in built_in_adapters().into_iter().enumerate() {
        // 进度：按适配器序号估算（完成后百分百精确；运行中含当前适配器的
        // 文件级进度由各适配器内部掌握，此处用粗粒度近似+ETA）。
        {
            let mut refresh = state.refresh.lock().unwrap();
            refresh.progress_percent =
                ((adapter_index as f64 / total_adapters as f64) * 100.0) as u8;
            let elapsed = now_ms() - scan_start;
            if adapter_index > 0 {
                let per_adapter = elapsed as f64 / adapter_index as f64;
                let remaining =
                    ((total_adapters - adapter_index) as f64 * per_adapter / 1000.0) as u64;
                refresh.eta_seconds = Some(remaining);
            }
        }
        // run_id 全局唯一：核心按「前缀-实例序号」生成，前缀须每次调用唯一
        //（ingest_runs.run_id 是主键；同前缀多适配器会撞键）。
        let config = RunConfig {
            run_id_prefix: format!("scan-{now}-a{adapter_index}"),
            ..config.clone()
        };
        let result = {
            let storage = state.storage.lock().unwrap();
            run_adapter_scan(&storage, adapter.as_ref(), &ctx, &config)
        };
        match result {
            Ok(reports) => summaries.extend(summarize_reports(&reports)),
            Err(e) => summaries.push(RefreshInstanceSummary {
                instance_id: format!("{}@*", adapter.adapter_id()),
                agent: adapter.agent().to_string(),
                status: "failed".to_string(),
                error: Some(e.to_string()),
                added: 0,
                updated: 0,
                files: 0,
                events: 0,
                diagnostics: 0,
            }),
        }
    }
    {
        // 分级归档保留：采集后按设置执行（明细→小时→物化周期→日→周/月；单事务）。
        {
            let policy = llm_usage_core::retention_tiered::TieredRetentionPolicy {
                events_days: retention.events_days,
                hourly_days: retention.hourly_days,
                daily_days: retention.daily_days,
                weekly_days: retention.weekly_days,
                monthly_days: retention.monthly_days,
                yearly_days: retention.yearly_days,
            };
            let outcome = {
                let storage = state.storage.lock().unwrap();
                llm_usage_core::retention_tiered::enforce_tiered_retention(
                    &storage,
                    &timezone,
                    now_ms(),
                    &policy,
                )
            };
            if let Err(e) = outcome {
                summaries.push(RefreshInstanceSummary {
                    instance_id: "retention".to_string(),
                    agent: "app".to_string(),
                    status: "failed".to_string(),
                    error: Some(format!("retention enforcement failed: {e}")),
                    added: 0,
                    updated: 0,
                    files: 0,
                    events: 0,
                    diagnostics: 0,
                });
            }
        }
    }
    {
        let mut refresh = state.refresh.lock().unwrap();
        refresh.running = false;
        refresh.last_finished_ms = now_ms();
        refresh.instances = summaries;
        refresh.progress_percent = 100;
        refresh.eta_seconds = None;
    }
    // 操作日志：采集完成摘要（白名单计数）。
    {
        let storage = state.storage.lock().unwrap();
        let total_events: u64 = refresh_summary_events(state);
        let _ = storage.conn().execute(
            "INSERT INTO diagnostics (code, message, created_ms) VALUES ('scan_completed', ?1, ?2)",
            rusqlite::params![
                format!(
                    "scan finished: {} instances, {} events collected ({})",
                    state.refresh.lock().unwrap().instances.len(),
                    total_events,
                    trigger.as_str()
                ),
                now_ms()
            ],
        );
    }
    true
}

fn refresh_summary_events(state: &Arc<AppState>) -> u64 {
    state
        .refresh
        .lock()
        .unwrap()
        .instances
        .iter()
        .map(|i| i.events)
        .sum()
}

/// 间隔调度循环：按设置的全局间隔触发刷新；间隔 0 = 暂停自动提取。
/// 错过时点（休眠）醒来后立即补一次扫描（V23 补扫合并语义：只补一次）。
pub fn spawn_scheduler(state: Arc<AppState>, stop: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        // 启动后先做一次回填扫描（Startup 触发）。
        run_refresh(&state, TriggerKind::Startup);
        let mut next_due = next_due_ms(&state);
        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let now = now_ms();
            if now >= next_due {
                run_refresh(&state, TriggerKind::Interval);
                next_due = next_due_ms(&state);
            }
            let wait = next_due.saturating_sub(now_ms()).clamp(500, 5_000);
            std::thread::sleep(std::time::Duration::from_millis(wait as u64));
        }
    });
}

fn next_due_ms(state: &Arc<AppState>) -> i64 {
    let interval = state.settings.lock().unwrap().refresh_interval_secs;
    if interval == 0 {
        // 暂停自动提取：仅轮询设置变化，不采集。
        return now_ms() + 5_000;
    }
    now_ms() + (interval as i64) * 1000
}
