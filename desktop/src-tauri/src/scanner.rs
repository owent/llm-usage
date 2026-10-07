//! 采集运行器：内置适配器注册表 + 全源刷新 + 间隔调度循环（M6）。
//! 约定：只读取用户启用的本地来源；定时任务只运行本应用采集逻辑；
//! 同一时刻仅一个刷新在执行（重复触发合并，V12/V23）。

use crate::app_state::{summarize_reports, AppState, RefreshInstanceSummary};
use llm_usage_core::adapters::framework::{
    DiscoverContext, RunConfig, ScanLimits, SourceAdapter, SourceRunReport,
    DEFAULT_SOURCE_TIME_BUDGET,
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

fn schedule_outcome(report: &SourceRunReport) -> Option<bool> {
    if !matches!(report.start, Some(RunStart::Started(_))) {
        return None;
    }
    match report.finish {
        RunStatus::Succeeded => Some(true),
        RunStatus::Failed => Some(false),
        _ => None,
    }
}

fn controlled_post_operation<T>(
    storage: &llm_usage_core::storage::Storage,
    operation: impl FnOnce() -> Result<T, llm_usage_core::error::CoreError>,
) -> Result<T, llm_usage_core::error::CoreError> {
    use llm_usage_core::adapters::run_policy;
    run_policy::check()?;
    let _sql = run_policy::SqliteScope::new(storage.conn())?;
    let result = operation();
    run_policy::check()?;
    result
}

fn automatic_allowed(state: &AppState) -> bool {
    if state.automatic_pause_requests.load(Ordering::SeqCst) > 0 {
        return false;
    }
    let settings = state.settings.lock().unwrap();
    crate::power::automatic_allowed(
        settings.refresh_interval_secs,
        settings.pause_on_battery_saver,
        crate::power::battery_saver(),
    )
}

fn discover_context(manual_roots: Vec<String>, manual_only: bool) -> DiscoverContext {
    if manual_only {
        return DiscoverContext {
            home_dir: None,
            env: Default::default(),
            manual_roots: manual_roots
                .into_iter()
                .map(std::path::PathBuf::from)
                .collect(),
        };
    }
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
/// - include=None 的全局自动刷新：排除自定义计划尚未到期的实例，
///   运行结束后推进到期计划。
fn run_refresh_filtered(
    state: &Arc<AppState>,
    trigger: TriggerKind,
    include: Option<std::collections::BTreeSet<String>>,
    after_clear: bool,
) -> bool {
    run_refresh_in_context(state, trigger, include, after_clear, None)
}

fn run_refresh_in_context(
    state: &Arc<AppState>,
    trigger: TriggerKind,
    include: Option<std::collections::BTreeSet<String>>,
    after_clear: bool,
    context: Option<DiscoverContext>,
) -> bool {
    if trigger != TriggerKind::Manual && !automatic_allowed(state) {
        return false;
    }
    {
        let mut refresh = state.refresh.lock().unwrap();
        if refresh.running
            || (state
                .clear_job_running
                .load(std::sync::atomic::Ordering::SeqCst)
                && !after_clear)
        {
            if trigger == TriggerKind::Manual && !after_clear {
                refresh.pending_manual = true;
            }
            return false;
        }
        refresh.running = true;
        state.disabled_during_scan.lock().unwrap().clear();
        refresh.progress_percent = 0;
        refresh.eta_seconds = None;
        refresh.completed_adapters.clear();
    }
    let (host_id, manual_roots, manual_only, retention, timezone) = {
        let settings = state.settings.lock().unwrap();
        let host = state.host_id.lock().unwrap().clone();
        (
            host,
            settings.manual_roots.clone(),
            settings.manual_roots_only,
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
    let ctx = context.unwrap_or_else(|| discover_context(manual_roots, manual_only));
    // Only the verified Copilot/Qwen file targets produced by this app are promoted.
    // Other supplemental exports retain their isolated validation boundary.
    let copilot_roots = if manual_only {
        Vec::new()
    } else {
        state
            .db_path
            .parent()
            .map(crate::telemetry_setup::verified_usage_roots)
            .unwrap_or_default()
    };
    let mut summaries: Vec<RefreshInstanceSummary> = Vec::new();
    // F2：刷新前修订号——本轮采集重写的 daily_usage 行 revision 均大于它
    // （retention 之后还会再 bump，不能用"当前 revision"等于过滤）。
    let revision_before: i64 = {
        let storage = state.storage.lock().unwrap();
        if let Err(e) =
            llm_usage_core::adapters::routing::retire_misrouted_sources(&storage, &copilot_roots)
        {
            eprintln!("source routing repair failed: {e}");
        }
        if let Err(e) =
            llm_usage_core::adapters::routing::retire_misrouted_qwen_sources(&storage, &ctx)
        {
            eprintln!("Qwen source routing repair failed: {e}");
        }
        if let Err(e) =
            llm_usage_core::adapters::routing::retire_misrouted_zed_sources(&storage, &ctx)
        {
            eprintln!("Zed source routing repair failed: {e}");
        }
        storage.data_revision().unwrap_or(0)
    };
    // 全局刷新排除有自定义启用计划的实例（逐源节奏覆盖全局）。排除集
    // 加载失败时宁可本轮不扫（记失败摘要），也不能把自定义计划的来源
    // 卷进全局节奏——节奏约定优先于本轮覆盖。
    let scheduled_due = if include.is_none() && trigger == TriggerKind::Interval {
        let storage = state.storage.lock().unwrap();
        state
            .source_intervals
            .lock()
            .unwrap()
            .due(&storage, now)
            .unwrap_or_default()
    } else {
        include.clone().unwrap_or_default()
    };
    let global_exclude = if include.is_none() && !after_clear && trigger == TriggerKind::Interval {
        let storage = state.storage.lock().unwrap();
        match llm_usage_core::schedules::custom_scheduled_instances(&storage) {
            Ok(set) => Some(Some(set.difference(&scheduled_due).cloned().collect())),
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
    // 两个解析槽在文件读取期间释放写锁；元数据、归档与提交仍用同一写者。
    let adapters = built_in_adapters();
    let total_adapters = adapters.len();
    let scan_start = now_ms();
    let scan_clock = std::time::Instant::now();
    let mut interrupted = false;
    for (pair_index, pair) in adapters.chunks(2).enumerate() {
        let adapter_index = pair_index * 2;
        if trigger != TriggerKind::Manual
            && (!automatic_allowed(state)
                || scan_clock.elapsed() >= std::time::Duration::from_secs(300))
        {
            interrupted = true;
            summaries.push(RefreshInstanceSummary {
                instance_id: "scheduler".into(),
                agent: "app".into(),
                status: "interrupted".into(),
                error: Some(
                    "automatic_scan_paused_or_time_budget_exhausted; unvisited sources remain due"
                        .into(),
                ),
                added: 0,
                updated: 0,
                files: 0,
                events: 0,
                diagnostics: 0,
            });
            break;
        }
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
        let requests: Vec<_> = pair
            .iter()
            .enumerate()
            .map(|(offset, adapter)| {
                let mut config = RunConfig {
                    run_id_prefix: format!("scan-{now}-a{}", adapter_index + offset),
                    ..config.clone()
                };
                if trigger != TriggerKind::Manual {
                    let remaining =
                        std::time::Duration::from_secs(300).saturating_sub(scan_clock.elapsed());
                    config.limits.jsonl.time_budget =
                        Some(DEFAULT_SOURCE_TIME_BUDGET.min(remaining));
                }
                llm_usage_core::adapters::framework::ParallelScanRequest {
                    adapter: adapter.as_ref(),
                    context: llm_usage_core::adapters::routing::context_for_adapter(
                        &ctx,
                        adapter.adapter_id(),
                        &copilot_roots,
                    ),
                    config,
                    filter: llm_usage_core::adapters::framework::InstanceFilter {
                        include: include.clone(),
                        exclude: exclude.clone(),
                    },
                }
            })
            .collect();
        let allowed_state = Arc::clone(state);
        let source_state = Arc::clone(state);
        let results = llm_usage_core::adapters::framework::run_adapter_scans_parallel(
            &state.storage,
            &requests,
            (trigger != TriggerKind::Manual)
                .then(|| scan_clock + std::time::Duration::from_secs(300)),
            Arc::new(move || trigger == TriggerKind::Manual || automatic_allowed(&allowed_state)),
            Some(Arc::new(move |instance| {
                !source_state
                    .disabled_during_scan
                    .lock()
                    .unwrap()
                    .contains(instance)
            })),
        );
        for (adapter, result) in pair.iter().zip(results) {
            state
                .refresh
                .lock()
                .unwrap()
                .completed_adapters
                .push(adapter.agent().to_string());
            match result {
                Ok(reports) => {
                    interrupted |= reports
                        .iter()
                        .any(|report| report.finish == RunStatus::Interrupted);
                    // 逐实例记录真实成败（RunStatus::Succeeded 才算成功）；
                    // 适配器级失败时本次到期实例保持无记录 ⇒ 下面按失败推进。
                    for report in &reports {
                        if let Some(outcome) = schedule_outcome(report) {
                            instance_outcomes.insert(report.instance_id.clone(), outcome);
                        }
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
    }
    if trigger != TriggerKind::Manual
        && (!automatic_allowed(state)
            || scan_clock.elapsed() >= std::time::Duration::from_secs(300))
    {
        interrupted = true;
    }
    let allowed_state = Arc::clone(state);
    let _post_scope = llm_usage_core::adapters::run_policy::enter(
        (trigger != TriggerKind::Manual).then(|| scan_clock + std::time::Duration::from_secs(300)),
        Some(Arc::new(move || {
            trigger == TriggerKind::Manual || automatic_allowed(&allowed_state)
        })),
    );
    // Copilot premium 请求额度（本机 copilot-user-cache.json；账户级请求配额，
    // 非 token，独立展示）。采集失败只记摘要，不影响本轮其他来源。
    if !interrupted
        && (trigger == TriggerKind::Manual || automatic_allowed(state))
        && !manual_only
        && include.is_none()
        && global_exclude.is_some()
    {
        let storage = state.storage.lock().unwrap();
        match controlled_post_operation(&storage, || {
            llm_usage_core::copilot_quota::collect(
                &storage,
                &ctx.env,
                ctx.home_dir.as_deref(),
                &timezone,
                now_ms(),
            )
        }) {
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
    if !interrupted && llm_usage_core::adapters::run_policy::check().is_ok() {
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
                (|| {
                    llm_usage_core::adapters::run_policy::check()?;
                    let _sql =
                        llm_usage_core::adapters::run_policy::SqliteScope::new(storage.conn())?;
                    llm_usage_core::retention_tiered::enforce_tiered_retention(
                        &storage,
                        &timezone,
                        now_ms(),
                        &policy,
                    )
                })()
                .map_err(|error| {
                    llm_usage_core::adapters::run_policy::check()
                        .err()
                        .unwrap_or(error)
                })
            };
            if let Err(e) = outcome {
                let stopped = matches!(e, llm_usage_core::error::CoreError::Interrupted(_));
                interrupted |= stopped;
                summaries.push(RefreshInstanceSummary {
                    instance_id: "retention".to_string(),
                    agent: "app".to_string(),
                    status: if stopped { "interrupted" } else { "failed" }.into(),
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
    if !interrupted && llm_usage_core::adapters::run_policy::check().is_ok() {
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
            let result = controlled_post_operation(&storage, || {
                let now = now_ms();
                if let Err(e) = storage.ensure_cost_matching_policy(&tz, now, &options) {
                    llm_usage_core::adapters::run_policy::check()?;
                    let _ = storage.conn().execute("INSERT INTO diagnostics(code,message,created_ms) VALUES('cost_policy_repair_failed',?1,?2)",
                        rusqlite::params![e.to_string(),now]);
                }
                let days = storage.cost_backfill_days_since(&tz, revision_before)?;
                for day in days {
                    llm_usage_core::adapters::run_policy::check()?;
                    if let Err(e) = storage.recompute_cost_day(&tz, &day, now, &options) {
                        llm_usage_core::adapters::run_policy::check()?;
                        let _ = storage.conn().execute(
                            "INSERT INTO diagnostics (code, message, created_ms) VALUES ('cost_recompute_failed', ?1, ?2)",
                            rusqlite::params![format!("cost recompute failed for {day}: {e}"), now],
                        );
                    }
                }
                Ok(())
            });
            if let Err(error) = result {
                let stopped = matches!(error, llm_usage_core::error::CoreError::Interrupted(_));
                interrupted |= stopped;
                summaries.push(RefreshInstanceSummary {
                    instance_id: "cost".into(),
                    agent: "app".into(),
                    status: if stopped { "interrupted" } else { "failed" }.into(),
                    error: Some(error.to_string()),
                    added: 0,
                    updated: 0,
                    files: 0,
                    events: 0,
                    diagnostics: 0,
                });
            }
        }
    }
    if !interrupted && llm_usage_core::adapters::run_policy::check().is_ok() {
        // F2 在线刷新（models.dev）：启用且缓存过期时后台刷新一次；
        // 失败仅记操作日志/诊断，不阻塞采集，不改写既有估算。
        crate::price_refresh::maybe_auto_refresh(state, false);
    }
    {
        // 逐源定时：本轮已实际运行的到期计划按真实成败推进 next_due
        // （失败记 running_error 留痕；未运行/无报告的实例记失败不冒认成功；
        // 未到期的不动，错过时点醒来后仍只补一次）。
        if !scheduled_due.is_empty() {
            let storage = state.storage.lock().unwrap();
            let tz = {
                let settings = state.settings.lock().unwrap();
                settings.timezone.clone()
            };
            for instance_id in &scheduled_due {
                let Some(success) = instance_outcomes.get(instance_id).copied() else {
                    continue;
                };
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
    interrupted |= llm_usage_core::adapters::run_policy::check().is_err();
    let completed = now_ms();
    let full_run = !interrupted && include.is_none() && global_exclude.is_some();
    if full_run {
        let interval = state.settings.lock().unwrap().refresh_interval_secs;
        let storage = state.storage.lock().unwrap();
        if let Err(error) = crate::system_tasks::mark_global_run(&storage, completed, interval) {
            summaries.push(RefreshInstanceSummary {
                instance_id: "scheduler".into(),
                agent: "app".into(),
                status: "failed".into(),
                error: Some(format!("could not persist global scan deadline: {error}")),
                added: 0,
                updated: 0,
                files: 0,
                events: 0,
                diagnostics: 0,
            });
        }
    }
    {
        let mut refresh = state.refresh.lock().unwrap();
        refresh.running = false;
        refresh.last_finished_ms = completed;
        if full_run {
            refresh.last_global_finished_ms = completed;
        }
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

/// System triggers honor persisted consent and the same per-source deadlines.
/// A leftover OS task is harmless after disabling the feature, even if deletion fails.
pub fn run_background_refresh(state: &Arc<AppState>) -> Result<bool, String> {
    run_background_refresh_in_context(state, None)
}

fn run_background_refresh_in_context(
    state: &Arc<AppState>,
    context: Option<DiscoverContext>,
) -> Result<bool, String> {
    let interval = state.settings.lock().unwrap().refresh_interval_secs;
    let now = now_ms();
    let (global_due, sources) = {
        let storage = state.storage.lock().unwrap();
        if !crate::system_tasks::desired(&storage)? || interval == 0 || !automatic_allowed(state) {
            return Ok(false);
        }
        (
            crate::system_tasks::global_due(&storage, now, interval)?,
            state
                .source_intervals
                .lock()
                .unwrap()
                .due(&storage, now)
                .map_err(|e| e.to_string())?,
        )
    };
    Ok(if global_due {
        run_refresh_in_context(state, TriggerKind::Interval, None, false, context)
    } else if !sources.is_empty() {
        run_refresh_in_context(state, TriggerKind::FixedTime, Some(sources), false, context)
    } else {
        false
    })
}

/// 间隔调度循环：按设置的全局间隔触发刷新；间隔 0 = 暂停自动提取。
/// 错过时点（休眠）醒来后立即补一次扫描（V23 补扫合并语义：只补一次）。
pub fn spawn_scheduler(state: Arc<AppState>, stop: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let mut was_allowed = automatic_allowed(&state);
        // 启动后先做一次回填扫描（Startup 触发）。
        if was_allowed {
            run_refresh(&state, TriggerKind::Startup);
        }
        let mut schedule = IntervalSchedule::default();
        let mut file_watch = crate::file_watch::FileWatch::default();
        let clock = std::time::Instant::now();
        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            state.scheduler_wakeups.fetch_add(1, Ordering::Relaxed);
            let interval = state.settings.lock().unwrap().refresh_interval_secs;
            let allowed = automatic_allowed(&state);
            let watch_settings = state.settings.lock().unwrap().clone();
            let watch_due = {
                let storage = state.storage.lock().unwrap();
                file_watch
                    .tick(
                        &storage,
                        allowed && watch_settings.file_watch_enabled,
                        watch_settings
                            .manual_roots_only
                            .then_some(watch_settings.manual_roots.as_slice()),
                        i64::try_from(clock.elapsed().as_millis()).unwrap_or(i64::MAX),
                    )
                    .unwrap_or_default()
            };
            let finished = state.refresh.lock().unwrap().last_global_finished_ms;
            let manual = {
                let mut refresh = state.refresh.lock().unwrap();
                if !refresh.running && !state.clear_job_running.load(Ordering::SeqCst) {
                    std::mem::take(&mut refresh.pending_manual)
                } else {
                    false
                }
            };
            let requested = {
                let storage = state.storage.lock().unwrap();
                crate::process_guard::take_refresh_request(&storage).unwrap_or(false)
            };
            let background_requested = {
                let storage = state.storage.lock().unwrap();
                crate::process_guard::take_background_refresh_request(&storage).unwrap_or(false)
            };
            if manual || requested {
                run_refresh(&state, TriggerKind::Manual);
            } else if allowed && !was_allowed {
                run_refresh(&state, TriggerKind::Startup);
            } else if allowed && !watch_due.is_empty() {
                run_refresh_filtered(&state, TriggerKind::FileWatch, Some(watch_due), false);
            } else if background_requested {
                let _ = run_background_refresh(&state);
            } else if allowed
                && schedule.tick(
                    i64::try_from(clock.elapsed().as_millis()).unwrap_or(i64::MAX),
                    interval,
                    finished,
                )
            {
                run_refresh(&state, TriggerKind::Interval);
            } else if allowed {
                // 逐源定时：无全局刷新在跑时，触发到期实例（FixedTime 语义；
                // 与手动/全局合并由 refresh 单飞保证，同源不并发）。
                // 到期探测失败不静默跳过：记诊断留痕，下轮重试。
                let due = {
                    let storage = state.storage.lock().unwrap();
                    match state
                        .source_intervals
                        .lock()
                        .unwrap()
                        .due(&storage, now_ms())
                    {
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
            was_allowed = allowed;
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
    fn pending_pause_stops_automatic_reads_while_settings_wait_for_the_writer() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/plan-execution/pause-intent")
            .join(format!("{}-{}", std::process::id(), now_ms()));
        let state = AppState::init(root.join("test.sqlite"), "test", false).unwrap();
        state.settings.lock().unwrap().refresh_interval_secs = 3600;
        state.settings.lock().unwrap().pause_on_battery_saver = false;
        let writer = state.storage.lock().unwrap();
        assert!(automatic_allowed(&state));
        let first = crate::power::PauseIntent::new(&state.automatic_pause_requests);
        let second = crate::power::PauseIntent::new(&state.automatic_pause_requests);
        assert!(!automatic_allowed(&state));
        drop(first);
        assert!(
            !automatic_allowed(&state),
            "another pending pause remains active"
        );
        drop(second);
        assert!(
            automatic_allowed(&state),
            "failed/cancelled save restores persisted settings"
        );
        drop(writer);
    }

    #[test]
    fn configured_roots_only_uses_real_registry_without_implicit_environment_sources() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/plan-finalization/manual-only")
            .join(format!("{}-{}", std::process::id(), now_ms()));
        let source = root.join("source/.codex");
        std::fs::create_dir_all(source.join("sessions")).unwrap();
        let at = jiff::Timestamp::now().to_string();
        let lines = format!(
            "{}\n{}\n",
            serde_json::json!({"timestamp":at,"type":"session_meta","payload":{"id":"isolated","session_id":"isolated","timestamp":at,"originator":"codex_vscode","cli_version":"0.999.0-synthetic","model_provider":"openai","source":"vscode"}}),
            serde_json::json!({"timestamp":at,"type":"token_usage_record","payload":{"thread_id":"isolated","turn_id":"turn","session_id":"isolated","response_id":"response","usage":{"input_tokens":10,"cached_input_tokens":0,"cache_write_input_tokens":0,"output_tokens":5,"reasoning_output_tokens":0,"total_tokens":15}}})
        );
        std::fs::write(source.join("sessions/rollout-only.jsonl"), lines).unwrap();
        let state = Arc::new(AppState::init(root.join("llm-usage.sqlite"), "test", false).unwrap());
        {
            let mut settings = state.settings.lock().unwrap();
            settings.manual_roots_only = true;
            settings.manual_roots = vec![source.to_string_lossy().into_owned()];
            settings.refresh_interval_secs = 0;
        }
        let context = discover_context(vec![source.to_string_lossy().into_owned()], true);
        assert!(context.env.is_empty());
        assert!(context.home_dir.is_none());
        assert!(run_refresh(&state, TriggerKind::Manual));
        let storage = state.storage.lock().unwrap();
        let agents: Vec<String> = storage
            .conn()
            .prepare("SELECT DISTINCT agent FROM source_instances")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(agents, vec!["codex"]);
        assert_eq!(
            storage
                .conn()
                .query_row("SELECT SUM(total_tokens) FROM usage_events", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            15
        );
        assert_eq!(
            storage
                .conn()
                .query_row("SELECT COUNT(*) FROM quota_history", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn manual_custom_schedule_and_disabled_background_use_real_registry_and_storage() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/plan-completion/scanner")
            .join(format!("{}-{}", std::process::id(), now_ms()));
        let home = root.join("home");
        let sessions = home.join(".codex/sessions");
        std::fs::create_dir_all(&sessions).unwrap();
        let state =
            Arc::new(AppState::init(root.join("llm-usage.sqlite"), "test-host", false).unwrap());
        let context = DiscoverContext {
            home_dir: None,
            env: std::collections::BTreeMap::from([(
                "CODEX_HOME".into(),
                home.join(".codex").display().to_string(),
            )]),
            manual_roots: Vec::new(),
        };
        let record = |id: &str| {
            let at = jiff::Timestamp::now().to_string();
            format!(
                "{}\n{}\n",
                serde_json::json!({"timestamp":at,"type":"session_meta","payload":{"id":id,"session_id":id,"timestamp":at,"originator":"codex_vscode","cli_version":"0.999.0-synthetic","model_provider":"openai","source":"vscode"}}),
                serde_json::json!({"timestamp":at,"type":"token_usage_record","payload":{"thread_id":id,"turn_id":id,"session_id":id,"response_id":id,"usage":{"input_tokens":10,"cached_input_tokens":0,"cache_write_input_tokens":0,"output_tokens":5,"reasoning_output_tokens":0,"total_tokens":15}}})
            )
        };
        std::fs::write(sessions.join("rollout-first.jsonl"), record("first")).unwrap();
        assert!(run_refresh_in_context(
            &state,
            TriggerKind::Manual,
            None,
            false,
            Some(context.clone())
        ));
        let instance: String = {
            let storage = state.storage.lock().unwrap();
            assert_eq!(
                storage
                    .conn()
                    .query_row("SELECT COUNT(*) FROM usage_events", [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                1
            );
            storage
                .conn()
                .query_row(
                    "SELECT instance_id FROM source_instances WHERE agent='codex'",
                    [],
                    |row| row.get(0),
                )
                .unwrap()
        };
        {
            let storage = state.storage.lock().unwrap();
            llm_usage_core::schedules::upsert_source_schedule(
                &storage,
                &llm_usage_core::schedules::SourceScheduleRule {
                    instance_id: instance.clone(),
                    rule_kind: "interval".into(),
                    interval_seconds: Some(86400),
                    time_of_day: None,
                    weekday: None,
                    tz: "UTC".into(),
                    enabled: true,
                },
                now_ms(),
                "UTC",
            )
            .unwrap();
            storage.conn().execute("INSERT INTO settings(key,value,schema_version,updated_at_ms) VALUES ('background_task','{\"enabled\":true}',1,0)", []).unwrap();
        }
        std::fs::write(sessions.join("rollout-second.jsonl"), record("second")).unwrap();
        state.settings.lock().unwrap().refresh_interval_secs = 0;
        assert!(!run_background_refresh_in_context(&state, Some(context.clone())).unwrap());
        for trigger in [
            TriggerKind::Startup,
            TriggerKind::Interval,
            TriggerKind::FixedTime,
        ] {
            assert!(
                !run_refresh_in_context(&state, trigger, None, false, Some(context.clone())),
                "paused automatic entry points must not scan"
            );
        }
        state.settings.lock().unwrap().refresh_interval_secs = 60;
        assert!(run_refresh_in_context(
            &state,
            TriggerKind::Interval,
            None,
            false,
            Some(context.clone())
        ));
        assert_eq!(
            state
                .storage
                .lock()
                .unwrap()
                .conn()
                .query_row("SELECT COUNT(*) FROM usage_events", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1,
            "automatic interval excludes the not-yet-due custom source"
        );
        state.settings.lock().unwrap().refresh_interval_secs = 0;
        assert!(run_refresh_in_context(
            &state,
            TriggerKind::Manual,
            None,
            false,
            Some(context.clone())
        ));
        assert_eq!(
            state
                .storage
                .lock()
                .unwrap()
                .conn()
                .query_row("SELECT COUNT(*) FROM usage_events", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            2,
            "manual collection includes custom schedules even when paused"
        );
        state.settings.lock().unwrap().refresh_interval_secs = 60;
        assert!(run_refresh_in_context(
            &state,
            TriggerKind::Manual,
            None,
            false,
            Some(context.clone())
        ));
        let global_finished = state.refresh.lock().unwrap().last_global_finished_ms;
        let deadline: String = {
            let storage = state.storage.lock().unwrap();
            assert!(!crate::system_tasks::global_due(&storage, now_ms(), 60).unwrap());
            storage
                .conn()
                .query_row(
                    "SELECT value FROM settings WHERE key='background_global_deadline'",
                    [],
                    |row| row.get(0),
                )
                .unwrap()
        };
        assert!(
            !run_background_refresh_in_context(&state, Some(context.clone())).unwrap(),
            "GUI full scans also postpone system global ticks"
        );
        std::fs::write(sessions.join("rollout-third.jsonl"), record("third")).unwrap();
        state
            .storage
            .lock()
            .unwrap()
            .conn()
            .execute("UPDATE extraction_schedules SET next_due_at_ms=0", [])
            .unwrap();
        assert!(run_refresh_in_context(
            &state,
            TriggerKind::FixedTime,
            Some(std::collections::BTreeSet::from([instance.clone()])),
            false,
            Some(context.clone())
        ));
        assert_eq!(
            state.refresh.lock().unwrap().last_global_finished_ms,
            global_finished,
            "frequent source-only completions must not reset the global timer"
        );
        let storage = state.storage.lock().unwrap();
        assert_eq!(
            storage
                .conn()
                .query_row("SELECT COUNT(*) FROM usage_events", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            3
        );
        assert_eq!(
            storage
                .conn()
                .query_row(
                    "SELECT value FROM settings WHERE key='background_global_deadline'",
                    [],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            deadline
        );
        drop(storage);
        {
            // No discovered carrier is an unvisited source, not a completed failure.
            for name in [
                "rollout-first.jsonl",
                "rollout-second.jsonl",
                "rollout-third.jsonl",
            ] {
                std::fs::remove_file(sessions.join(name)).unwrap();
            }
            state
                .storage
                .lock()
                .unwrap()
                .conn()
                .execute("UPDATE extraction_schedules SET next_due_at_ms=0", [])
                .unwrap();
            assert!(run_refresh_in_context(
                &state,
                TriggerKind::FixedTime,
                Some(std::collections::BTreeSet::from([instance.clone()])),
                false,
                Some(context)
            ));
            assert_eq!(
                state
                    .storage
                    .lock()
                    .unwrap()
                    .conn()
                    .query_row(
                        "SELECT next_due_at_ms FROM extraction_schedules WHERE instance_id=?1",
                        [&instance],
                        |r| r.get::<_, i64>(0)
                    )
                    .unwrap(),
                0
            );
        }
        {
            let mut refresh = state.refresh.lock().unwrap();
            refresh.running = true;
        }
        assert!(!run_refresh(&state, TriggerKind::Manual));
        assert!(state.refresh.lock().unwrap().pending_manual);
    }

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
        assert_eq!(schedule_outcome(&report), None);
        report.finish = RunStatus::Succeeded;
        assert_eq!(schedule_outcome(&report), None);
        report.start = Some(RunStart::Started("existing".into()));
        assert_eq!(schedule_outcome(&report), Some(true));
        report.finish = RunStatus::Interrupted;
        assert_eq!(schedule_outcome(&report), None);
        report.finish = RunStatus::Failed;
        assert_eq!(schedule_outcome(&report), Some(false));
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
