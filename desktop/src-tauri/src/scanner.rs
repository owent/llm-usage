//! 采集运行器：内置适配器注册表 + 全源刷新 + 间隔调度循环（M6）。
//! 合同：只读取用户启用的本地来源；定时任务只运行本应用采集逻辑；
//! 同一时刻仅一个刷新在执行（重复触发合并，V12/V23）。

use crate::app_state::{summarize_reports, AppState, RefreshInstanceSummary};
use llm_usage_core::adapters::framework::{
    DiscoverContext, RunConfig, ScanLimits, SourceAdapter, SourceRunReport,
};
use llm_usage_core::jobs::{RunStart, RunStatus, TriggerKind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// 内置适配器注册表（与探针工具共用 core 实现）。
pub fn built_in_adapters() -> Vec<Box<dyn SourceAdapter>> {
    llm_usage_core::adapters::built_in_adapters()
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn completed_successfully(report: &SourceRunReport) -> bool {
    matches!(report.start, Some(RunStart::Started(_))) && report.finish == RunStatus::Succeeded
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
    run_refresh_filtered(state, trigger, None, false)
}

/// 清空任务提交后执行的全源重采（含自定义计划来源）；
/// 清空标志保持有效直到重采结束。
pub fn run_refresh_after_clear(state: &Arc<AppState>) -> bool {
    run_refresh_filtered(state, TriggerKind::Manual, None, true)
}

/// 逐源定时接线的过滤版：
/// - include（到期集合）非空：只运行这些实例（FixedTime/Interval 触发）；
/// - include=None 的全局刷新：排除有自定义启用计划的实例（覆盖语义），
///   运行结束后推进到期计划。
fn run_refresh_filtered(
    state: &Arc<AppState>,
    trigger: TriggerKind,
    include: Option<std::collections::BTreeSet<String>>,
    after_clear: bool,
) -> bool {
    {
        let mut refresh = state.refresh.lock().unwrap();
        if refresh.running
            || (state
                .clear_job_running
                .load(std::sync::atomic::Ordering::SeqCst)
                && !after_clear)
        {
            return false;
        }
        refresh.running = true;
        refresh.progress_percent = 0;
        refresh.eta_seconds = None;
        refresh.completed_adapters.clear();
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
    // F2：刷新前修订号——本轮采集重写的 daily_usage 行 revision 均大于它
    // （retention 之后还会再 bump，不能用"当前 revision"等于过滤）。
    let revision_before: i64 = {
        let storage = state.storage.lock().unwrap();
        storage.data_revision().unwrap_or(0)
    };
    // 全局刷新排除有自定义启用计划的实例（逐源节奏覆盖全局）。排除集
    // 加载失败时宁可本轮不扫（记失败摘要），也不能把自定义计划的来源
    // 卷进全局节奏——节奏合同优先于本轮覆盖。
    let global_exclude = if include.is_none() && !after_clear {
        let storage = state.storage.lock().unwrap();
        match llm_usage_core::schedules::custom_scheduled_instances(&storage) {
            Ok(set) => Some(Some(set)),
            Err(e) => {
                summaries.push(RefreshInstanceSummary {
                    instance_id: "scheduler".to_string(),
                    agent: "app".to_string(),
                    status: "failed".to_string(),
                    error: Some(format!(
                        "failed to load per-source schedules; global refresh skipped to respect per-source cadence: {e}"
                    )),
                    added: 0,
                    updated: 0,
                    files: 0,
                    events: 0,
                    diagnostics: 0,
                });
                None
            }
        }
    } else {
        Some(None)
    };
    // 逐源定时成败归属：实例 → 本轮实际运行结果（到期计划按真实结果推进）。
    let mut instance_outcomes: std::collections::BTreeMap<String, bool> =
        std::collections::BTreeMap::new();
    // 写锁按适配器分段获取（而非整个刷新持有）：查询走 WAL 只读连接不受影响，
    // 设置写入等其他写操作可在适配器之间交错；任意时刻仍只有一个写者。
    let adapters = built_in_adapters();
    let total_adapters = adapters.len();
    let scan_start = now_ms();
    for (adapter_index, adapter) in adapters.into_iter().enumerate() {
        let Some(exclude) = global_exclude.clone() else {
            break;
        };
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
        let filter = llm_usage_core::adapters::framework::InstanceFilter {
            include: include.clone(),
            exclude,
        };
        let result = {
            let storage = state.storage.lock().unwrap();
            llm_usage_core::adapters::framework::run_adapter_scan_filtered(
                &storage,
                adapter.as_ref(),
                &ctx,
                &config,
                &filter,
            )
        };
        state
            .refresh
            .lock()
            .unwrap()
            .completed_adapters
            .push(adapter.agent().to_string());
        match result {
            Ok(reports) => {
                // 逐实例记录真实成败（RunStatus::Succeeded 才算成功）；
                // 适配器级失败时本次到期实例保持无记录 ⇒ 下面按失败推进。
                for report in &reports {
                    instance_outcomes
                        .insert(report.instance_id.clone(), completed_successfully(report));
                }
                summaries.extend(summarize_reports(&reports));
            }
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
    // Copilot premium 请求额度（本机 copilot-user-cache.json；账户级请求配额，
    // 非 token，独立展示）。采集失败只记摘要，不影响本轮其他来源。
    if include.is_none() && global_exclude.is_some() {
        let storage = state.storage.lock().unwrap();
        match llm_usage_core::copilot_quota::collect(
            &storage,
            &ctx.env,
            ctx.home_dir.as_deref(),
            &timezone,
            now_ms(),
        ) {
            Ok(n) if n > 0 => summaries.push(RefreshInstanceSummary {
                instance_id: "copilot-quota".to_string(),
                agent: "copilot".to_string(),
                status: "succeeded".to_string(),
                error: None,
                added: n as i64,
                updated: 0,
                files: 1,
                events: n,
                diagnostics: 0,
            }),
            Ok(_) => {}
            Err(e) => summaries.push(RefreshInstanceSummary {
                instance_id: "copilot-quota".to_string(),
                agent: "copilot".to_string(),
                status: "failed".to_string(),
                error: Some(format!("copilot quota collection failed: {e}")),
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
        // F2 费用回填：采集改写了当日事件（数据修订）⇒ 按当前价格设置重算
        // 受影响未封存日的日成本行。估算未启用时跳过（显式重算仍可用）。
        let (pricing_enabled, tz, options) = {
            let settings = state.settings.lock().unwrap();
            (
                settings.pricing.enabled,
                settings.timezone.clone(),
                settings.pricing.estimate_options(),
            )
        };
        if pricing_enabled {
            let storage = state.storage.lock().unwrap();
            let now = now_ms();
            let days = storage
                .cost_backfill_days_since(&tz, revision_before)
                .unwrap_or_default();
            for day in days {
                if let Err(e) = storage.recompute_cost_day(&tz, &day, now, &options) {
                    let _ = storage.conn().execute(
                        "INSERT INTO diagnostics (code, message, created_ms)
                         VALUES ('cost_recompute_failed', ?1, ?2)",
                        rusqlite::params![format!("cost recompute failed for {day}: {e}"), now],
                    );
                }
            }
        }
    }
    {
        // 逐源定时：本轮已实际运行的到期计划按真实成败推进 next_due
        // （失败记 running_error 留痕；未运行/无报告的实例记失败不冒认成功；
        // 未到期的不动，错过时点醒来后仍只补一次）。
        if let Some(due) = &include {
            let storage = state.storage.lock().unwrap();
            let tz = {
                let settings = state.settings.lock().unwrap();
                settings.timezone.clone()
            };
            for instance_id in due {
                let success = instance_outcomes.get(instance_id).copied().unwrap_or(false);
                if let Err(e) = llm_usage_core::schedules::mark_source_run(
                    &storage,
                    instance_id,
                    now_ms(),
                    success,
                    &tz,
                    None,
                ) {
                    let _ = storage.conn().execute(
                        "INSERT INTO diagnostics (code, message, created_ms) VALUES ('schedule_mark_failed', ?1, ?2)",
                        rusqlite::params![
                            format!("mark_source_run failed for {instance_id}: {e}"),
                            now_ms()
                        ],
                    );
                }
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
        let mut schedule = IntervalSchedule::default();
        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let interval = state.settings.lock().unwrap().refresh_interval_secs;
            let finished = state.refresh.lock().unwrap().last_finished_ms;
            let requested = {
                let storage = state.storage.lock().unwrap();
                crate::process_guard::take_refresh_request(&storage).unwrap_or(false)
            };
            if requested || schedule.tick(now_ms(), interval, finished) {
                run_refresh(&state, TriggerKind::Interval);
            } else {
                // 逐源定时：无全局刷新在跑时，触发到期实例（FixedTime 语义；
                // 与手动/全局合并由 refresh 单飞保证，同源不并发）。
                // 到期探测失败不静默跳过：记诊断留痕，下轮重试。
                let due = {
                    let storage = state.storage.lock().unwrap();
                    match llm_usage_core::schedules::due_instances(&storage, now_ms()) {
                        Ok(due) => due,
                        Err(e) => {
                            let _ = storage.conn().execute(
                                "INSERT INTO diagnostics (code, message, created_ms) VALUES ('schedule_probe_failed', ?1, ?2)",
                                rusqlite::params![
                                    format!("due_instances probe failed: {e}"),
                                    now_ms()
                                ],
                            );
                            std::collections::BTreeSet::new()
                        }
                    }
                };
                if !due.is_empty() && !state.refresh.lock().unwrap().running {
                    run_refresh_filtered(&state, TriggerKind::FixedTime, Some(due), false);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    });
}

/// Pure scheduling state: zero disables, settings changes reset the deadline,
/// and waking after multiple missed intervals schedules only one scan.
#[derive(Default)]
struct IntervalSchedule {
    interval_secs: u64,
    last_finished: i64,
    due: Option<i64>,
}

impl IntervalSchedule {
    fn tick(&mut self, now: i64, interval_secs: u64, last_finished: i64) -> bool {
        let delay = i64::try_from(interval_secs)
            .unwrap_or(i64::MAX)
            .saturating_mul(1000);
        if interval_secs != self.interval_secs || last_finished != self.last_finished {
            self.interval_secs = interval_secs;
            self.last_finished = last_finished;
            self.due = (interval_secs > 0).then(|| now.saturating_add(delay));
        }
        if self.due.is_some_and(|due| now >= due) {
            self.due = Some(now.saturating_add(delay));
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merged_run_does_not_complete_a_due_source_schedule() {
        let mut report = SourceRunReport {
            instance_id: "source".into(),
            run_id: Some("existing".into()),
            start: Some(RunStart::Merged("existing".into())),
            outcome: None,
            files: Vec::new(),
            reconciliations: Vec::new(),
            finish: RunStatus::Running,
            error: None,
        };
        assert!(!completed_successfully(&report));
        report.finish = RunStatus::Succeeded;
        assert!(!completed_successfully(&report));
        report.start = Some(RunStart::Started("existing".into()));
        assert!(completed_successfully(&report));
    }

    #[test]
    fn disabled_interval_never_scans_and_changes_take_effect() {
        let mut timer = IntervalSchedule::default();
        for now in [0, 5000, 10000, 1000000] {
            assert!(!timer.tick(now, 0, 0));
        }
        assert!(!timer.tick(1000000, 60, 0));
        assert!(!timer.tick(1059999, 60, 0));
        assert!(timer.tick(1060000, 60, 0));
        assert!(!timer.tick(1060001, 0, 0));
        assert!(!timer.tick(i64::MAX, 0, 0));
    }

    #[test]
    fn sleep_missed_intervals_merge_and_manual_completion_resets_due() {
        let mut timer = IntervalSchedule::default();
        assert!(!timer.tick(0, 10, 0));
        assert!(timer.tick(100000, 10, 0));
        assert!(!timer.tick(100001, 10, 0));
        assert!(!timer.tick(105000, 10, 105000));
        assert!(!timer.tick(110000, 10, 105000));
        assert!(timer.tick(115000, 10, 105000));
    }
}
