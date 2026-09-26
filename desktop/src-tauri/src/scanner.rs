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
    let (host_id, manual_roots, retention_days, timezone) = {
        let settings = state.settings.lock().unwrap();
        let host = state.host_id.lock().unwrap().clone();
        (
            host,
            settings.manual_roots.clone(),
            settings.retention_days,
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
    // 写锁按适配器分段获取（而非整个刷新持有）：查询走 WAL 只读连接不受影响，
    // 设置写入等其他写操作可在适配器之间交错；任意时刻仍只有一个写者。
    for (adapter_index, adapter) in built_in_adapters().into_iter().enumerate() {
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
        // 有限保留：采集后按设置执行（封存过期日 → 删除过期明细；单事务）。
        if let Some(days) = retention_days {
            let policy = llm_usage_core::retention::RetentionPolicy {
                detail_days: days,
                diagnostics_days: 30.min(days),
                hard_max_days: None,
            };
            let outcome = {
                let storage = state.storage.lock().unwrap();
                llm_usage_core::retention::enforce_retention(&storage, &timezone, now_ms(), &policy)
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
    }
    true
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
