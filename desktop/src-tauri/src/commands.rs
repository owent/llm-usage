//! Tauri IPC 命令：查询/来源/刷新/设置/导出。
//! DTO 合同（architecture.md）：token 等大数值用十进制字符串传输，
//! 避免前端浮点舍入；错误返回结构化 code+message，不回传内部 SQL/游标。

use crate::app_state::{load_settings, save_settings, AppSettings, AppState};
use crate::scanner::{now_ms, run_refresh};
use llm_usage_core::calendar::{parse_date, WeekStart};
use llm_usage_core::exchange::{
    build_aggregate_export, ExchangeKind, ExportRequest, EXCHANGE_FORMAT_VERSION,
};
use llm_usage_core::jobs::TriggerKind;
use llm_usage_core::query::{
    heatmap_cells, hourly_breakdown, query_summary, Filters, Granularity, SummaryRequest,
};
use serde::Deserialize;
use std::io::Write;
use std::sync::Arc;
use tauri::Manager;

/// Empty ownership must select no rows; it must never mean all users.
fn user_instances(
    storage: &llm_usage_core::storage::Storage,
    user_id: &str,
) -> Result<Vec<String>, String> {
    let instances: Vec<String> = storage
        .conn()
        .prepare("SELECT instance_id FROM source_instances WHERE user_id = ?1")
        .and_then(|mut stmt| {
            let rows = stmt.query_map([user_id], |r| r.get(0))?;
            rows.collect::<Result<Vec<String>, _>>()
        })
        .map_err(|e| err("db", e.to_string()))?;
    Ok(instances)
}

/// 写操作日志到诊断表（白名单 code，无正文）。
pub(crate) fn log_operation(storage: &llm_usage_core::storage::Storage, code: &str, message: &str) {
    let _ = storage.conn().execute(
        "INSERT INTO diagnostics (code, message, created_ms) VALUES (?1, ?2, ?3)",
        rusqlite::params![code, message, crate::scanner::now_ms()],
    );
}

fn err(code: &str, message: impl Into<String>) -> String {
    serde_json::json!({ "code": code, "message": message.into() }).to_string()
}

fn week_start_of(n: u8) -> Result<WeekStart, String> {
    match n {
        0 => Ok(WeekStart::Monday),
        6 => Ok(WeekStart::Sunday),
        _ => Err(err(
            "invalid_week_start",
            format!("week_start {n} unsupported; core supports Monday(0)/Sunday(6)"),
        )),
    }
}

#[cfg(target_family = "windows")]
mod win_tasks {
    use super::err;

    const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    const TASK_NAME: &str = "LLMUsageDataRefresh";

    fn run_value() -> String {
        format!(
            "\"{}\"",
            std::env::current_exe().unwrap_or_default().display()
        )
    }

    pub fn auto_start_enabled() -> Result<bool, String> {
        let out = std::process::Command::new("reg")
            .args(["query", RUN_KEY, "/v", "LLMUsage"])
            .output()
            .map_err(|e| err("reg_query", e.to_string()))?;
        Ok(out.status.success() && String::from_utf8_lossy(&out.stdout).contains("REG_SZ"))
    }

    pub fn set_auto_start(enabled: bool) -> Result<(), String> {
        let result = if enabled {
            std::process::Command::new("reg")
                .args([
                    "add",
                    RUN_KEY,
                    "/v",
                    "LLMUsage",
                    "/t",
                    "REG_SZ",
                    "/d",
                    &run_value(),
                    "/f",
                ])
                .output()
        } else {
            std::process::Command::new("reg")
                .args(["delete", RUN_KEY, "/v", "LLMUsage", "/f"])
                .output()
        }
        .map_err(|e| err("reg_write", e.to_string()))?;
        // 删除不存在的值也返回错误码（视为已关闭）。
        if !result.status.success() && enabled {
            return Err(err(
                "reg_write",
                String::from_utf8_lossy(&result.stderr).to_string(),
            ));
        }
        Ok(())
    }

    pub fn refresh_task_enabled() -> Result<bool, String> {
        let out = std::process::Command::new("schtasks")
            .args(["/Query", "/TN", TASK_NAME])
            .output()
            .map_err(|e| err("schtasks", e.to_string()))?;
        Ok(out.status.success())
    }

    /// 安装每小时 headless 刷新任务（当前用户上下文，无需提权）。
    /// 间隔与界面"刷新间隔"独立：系统任务保证应用未运行时也补采集。
    pub fn install_refresh_task() -> Result<(), String> {
        let exe = run_value();
        let out = std::process::Command::new("schtasks")
            .args([
                "/Create",
                "/TN",
                TASK_NAME,
                "/TR",
                &format!("{exe} --headless"),
                "/SC",
                "HOURLY",
                "/F",
            ])
            .output()
            .map_err(|e| err("schtasks", e.to_string()))?;
        if !out.status.success() {
            return Err(err(
                "schtasks_install",
                String::from_utf8_lossy(&out.stderr).to_string(),
            ));
        }
        Ok(())
    }

    pub fn uninstall_refresh_task() -> Result<(), String> {
        let out = std::process::Command::new("schtasks")
            .args(["/Delete", "/TN", TASK_NAME, "/F"])
            .output()
            .map_err(|e| err("schtasks", e.to_string()))?;
        if !out.status.success() {
            return Err(err(
                "schtasks_uninstall",
                String::from_utf8_lossy(&out.stderr).to_string(),
            ));
        }
        Ok(())
    }
}

/// 系统保存对话框：用户选定导出位置（返回 None = 取消）。
/// 后端只把文件写到该路径（architecture.md 导出合同）。
#[tauri::command]
pub fn pick_save_path(default_name: String) -> Option<String> {
    rfd::FileDialog::new()
        .set_file_name(&default_name)
        .save_file()
        .map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
pub fn system_task_status() -> Result<serde_json::Value, String> {
    #[cfg(target_family = "windows")]
    {
        Ok(serde_json::json!({
            "platform": "windows",
            "auto_start": win_tasks::auto_start_enabled()?,
            "refresh_task": win_tasks::refresh_task_enabled()?,
            "refresh_task_interval": "hourly",
        }))
    }
    #[cfg(not(target_family = "windows"))]
    {
        Ok(serde_json::json!({
            "platform": std::env::consts::OS,
            "auto_start": false,
            "refresh_task": false,
            "unsupported": true,
        }))
    }
}

#[tauri::command]
pub fn set_auto_start(enabled: bool) -> Result<(), String> {
    #[cfg(target_family = "windows")]
    {
        win_tasks::set_auto_start(enabled)
    }
    #[cfg(not(target_family = "windows"))]
    {
        let _ = enabled;
        Err(err(
            "unsupported_platform",
            "auto start is Windows-only for now",
        ))
    }
}

#[tauri::command]
pub fn set_refresh_task(install: bool) -> Result<(), String> {
    #[cfg(target_family = "windows")]
    {
        if install {
            win_tasks::install_refresh_task()
        } else {
            win_tasks::uninstall_refresh_task()
        }
    }
    #[cfg(not(target_family = "windows"))]
    {
        let _ = install;
        Err(err(
            "unsupported_platform",
            "background task is Windows-only for now",
        ))
    }
}

/// 查询请求 DTO。
#[derive(Debug, Clone, Deserialize)]
pub struct SummaryQuery {
    pub first_day: String,
    pub last_day: String,
    /// day | week | month。
    pub granularity: String,
    #[serde(default)]
    pub agents: Vec<String>,
    #[serde(default)]
    pub providers: Vec<String>,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub first_period: Option<String>,
    #[serde(default)]
    pub last_period: Option<String>,
}

fn build_request(
    settings: &AppSettings,
    q: &SummaryQuery,
    instances: Vec<String>,
) -> Result<SummaryRequest, String> {
    let granularity = match q.granularity.as_str() {
        "hour" => Granularity::Hour,
        "day" => Granularity::Day,
        "week" => Granularity::Week,
        "month" => Granularity::Month,
        other => {
            return Err(err(
                "invalid_granularity",
                format!("unknown granularity {other:?}"),
            ))
        }
    };
    Ok(SummaryRequest {
        timezone: settings.timezone.clone(),
        week_start: week_start_of(settings.effective_week_start())?,
        first_day: parse_date(&q.first_day)
            .map_err(|e| err("invalid_date", format!("first_day: {e}")))?,
        last_day: parse_date(&q.last_day)
            .map_err(|e| err("invalid_date", format!("last_day: {e}")))?,
        granularity,
        filters: Filters {
            agents: q.agents.clone(),
            providers: q.providers.clone(),
            models: q.models.clone(),
            quality_buckets: Vec::new(),
            instances: Some(instances),
        },
        today: {
            let cal = llm_usage_core::calendar::Calendar::new(&settings.timezone)
                .map_err(|e| err("invalid_timezone", e.to_string()))?;
            cal.today(now_ms())
                .map_err(|e| err("calendar", e.to_string()))?
        },
        retention_cutoff: None,
    })
}

/// token 数值 → 十进制字符串（unknown 保持 null）。
fn opt_num(v: Option<i64>) -> Option<String> {
    v.map(|n| n.to_string())
}

fn metric_sums_dto(s: &llm_usage_core::query::MetricSums) -> serde_json::Value {
    serde_json::json!({
        "input_total_known": opt_num(s.input_total_known),
        "uncached_known": opt_num(s.uncached_known),
        "cache_read_known": opt_num(s.cache_read_known),
        "cache_write_known": opt_num(s.cache_write_known),
        "output_total_known": opt_num(s.output_total_known),
        "total_tokens_known": opt_num(s.total_tokens_known),
        "input_known_count": s.input_known_count,
        "input_unknown_count": s.input_unknown_count,
        "output_known_count": s.output_known_count,
        "output_unknown_count": s.output_unknown_count,
        "total_known_count": s.total_known_count,
        "total_unknown_count": s.total_unknown_count,
        "event_count": s.event_count,
        "call_count": s.call_count,
        "attempt_count": s.attempt_count,
        "conflict_count": s.conflict_count,
        "cache_input_ratio": s.cache_input_ratio().map(|r| r.as_f64()),
        "avg_duration_ms": opt_num(s.avg_duration_ms),
        "total_duration_ms": opt_num(s.total_duration_ms),
        "duration_sample_count": s.duration_sample_count,
    })
}

#[tauri::command]
pub async fn summary(
    state: tauri::State<'_, Arc<AppState>>,
    q: SummaryQuery,
) -> Result<serde_json::Value, String> {
    let state = Arc::clone(&state);
    let settings = state.settings.lock().unwrap().clone();
    let current_user = state.current_user.lock().unwrap().clone();
    tauri::async_runtime::spawn_blocking(move || {
        summary_query(&state, &settings, &current_user, &q)
    })
    .await
    .map_err(|_| err("query", "query worker failed"))?
}

fn summary_query(
    state: &Arc<AppState>,
    settings: &AppSettings,
    current_user: &str,
    q: &SummaryQuery,
) -> Result<serde_json::Value, String> {
    let request = build_request(settings, q, Vec::new())?;
    // 读路径：常驻只读连接（WAL 与后台扫描并发；失败回退写连接）。
    let storage = crate::app_state::read_conn(state);
    let instances = user_instances(&storage, current_user)?;
    let request = SummaryRequest {
        filters: Filters {
            instances: Some(instances),
            ..request.filters
        },
        ..request
    };
    let selection = match (q.first_period.as_deref(), q.last_period.as_deref()) {
        (None, None) => None,
        (Some(first), Some(last)) => Some((first, last)),
        _ => return Err(err("query", "both period bounds are required")),
    };
    let s = llm_usage_core::query::query_summary_selected(&storage, &request, selection)
        .map_err(|e| err("query", e.to_string()))?;
    let today_hourly = if request.first_day == request.today && request.last_day == request.today {
        hourly_breakdown(
            &storage,
            &settings.timezone,
            request.today,
            &request.filters,
        )
        .map_err(|e| err("query", e.to_string()))?
    } else {
        Vec::new()
    };
    drop(storage);
    Ok(serde_json::json!({
        "data_revision": s.data_revision,
        "timezone": s.timezone,
        "periods": s.periods.iter().map(|p| serde_json::json!({
            "label": p.label,
            "start_day": p.start_day.to_string(),
            "end_day": p.end_day.to_string(),
            "in_progress": p.in_progress,
            "partial_history": p.partial_history,
            "sums": metric_sums_dto(&p.sums),
            "distinct_sessions": p.distinct_sessions,
            "active_days": p.active_days,
        })).collect::<Vec<_>>(),
        "totals": metric_sums_dto(&s.totals),
        "distinct_sessions": s.distinct_sessions,
        "active_days": s.active_days,
        "models": s.model_breakdown.iter().map(|m| serde_json::json!({
            "provider": m.provider_id,
            "model": m.model_raw,
            "sums": metric_sums_dto(&m.sums),
        })).collect::<Vec<_>>(),
        "agents": s.agent_breakdown.iter().map(|a| serde_json::json!({
            "agent": a.agent,
            "sums": metric_sums_dto(&a.sums),
        })).collect::<Vec<_>>(),
        "today_hourly": today_hourly.iter().map(|h| serde_json::json!({
            "hour": h.hour,
            "calls": h.call_count,
            "total_tokens": opt_num(h.total_tokens_known),
            "input_total": opt_num(h.input_total_known),
            "cache_read": opt_num(h.cache_read_known),
            "output_total": opt_num(h.output_total_known),
            "sessions": h.session_count,
            "avg_duration_ms": opt_num(h.avg_duration_ms),
        })).collect::<Vec<_>>(),
        "excluded_event_count": s.excluded_event_count,
    }))
}

#[tauri::command]
pub async fn heatmap(
    state: tauri::State<'_, Arc<AppState>>,
    q: SummaryQuery,
) -> Result<serde_json::Value, String> {
    let state = Arc::clone(&state);
    let settings = state.settings.lock().unwrap().clone();
    let current_user = state.current_user.lock().unwrap().clone();
    tauri::async_runtime::spawn_blocking(move || {
        heatmap_query(&state, &settings, &current_user, &q)
    })
    .await
    .map_err(|_| err("query", "query worker failed"))?
}

fn heatmap_query(
    state: &Arc<AppState>,
    settings: &AppSettings,
    current_user: &str,
    q: &SummaryQuery,
) -> Result<serde_json::Value, String> {
    let mut request = build_request(settings, q, Vec::new())?;
    let storage = crate::app_state::read_conn(state);
    request.filters.instances = Some(user_instances(&storage, current_user)?);
    let cells = heatmap_cells(
        &storage,
        &settings.timezone,
        request.first_day,
        request.last_day,
        &request.filters,
    )
    .map_err(|e| err("query", e.to_string()))?;
    drop(storage);
    Ok(serde_json::json!({
        "cells": cells.iter().map(|c| serde_json::json!({
            "day": c.day,
            "weekday": c.weekday,
            "calls": c.call_count,
            "total_tokens": opt_num(c.total_tokens_known),
            "available": c.available,
            "partial": c.partial,
        })).collect::<Vec<_>>(),
    }))
}

/// 来源清单：实例注册 + 最近运行 + 兼容标记计数 + 失踪文件数
/// （磁盘已不存在的注册文件：Agent 自行清理/压实产生，其历史只存于本应用）。
#[tauri::command]
pub fn list_sources(state: tauri::State<'_, Arc<AppState>>) -> Result<serde_json::Value, String> {
    let storage = crate::app_state::read_conn(&state);
    let mut stmt = storage
        .conn()
        .prepare(
            "SELECT s.instance_id, s.agent, s.format, s.health, s.enabled, s.origin_host_id,
                    s.user_id,
                    (SELECT e.rule_kind FROM extraction_schedules e
                     WHERE e.schedule_id = 'source:' || s.instance_id AND e.enabled = 1) AS sched_kind,
                    (SELECT e.interval_seconds FROM extraction_schedules e
                     WHERE e.schedule_id = 'source:' || s.instance_id AND e.enabled = 1) AS sched_interval,
                    (SELECT e.time_of_day FROM extraction_schedules e
                     WHERE e.schedule_id = 'source:' || s.instance_id AND e.enabled = 1) AS sched_time,
                    (SELECT e.weekday FROM extraction_schedules e
                     WHERE e.schedule_id = 'source:' || s.instance_id AND e.enabled = 1) AS sched_weekday,
                    (SELECT e.next_due_at_ms FROM extraction_schedules e
                     WHERE e.schedule_id = 'source:' || s.instance_id AND e.enabled = 1) AS sched_next_due,
                    (SELECT MAX(finished_ms) FROM ingest_runs r
                     WHERE r.instance_id = s.instance_id AND r.status = 'succeeded') AS last_ok_ms,
                    (SELECT COUNT(*) FROM source_files f WHERE f.instance_id = s.instance_id
                     AND f.status = 'active_compat') AS compat_files,
                    (SELECT COUNT(*) FROM source_files f WHERE f.instance_id = s.instance_id
                     AND f.status = 'degraded') AS degraded_files,
                    (SELECT COUNT(*) FROM source_files f WHERE f.instance_id = s.instance_id
                     AND f.status = 'unsupported') AS unsupported_files,
                    (SELECT COUNT(*) FROM source_files f WHERE f.instance_id = s.instance_id
                     AND f.status = 'incompatible') AS incompatible_files,
                    s.location_hint
             FROM source_instances s WHERE s.health != 'not_applicable' ORDER BY s.agent, s.instance_id",
        )
        .map_err(|e| err("db", e.to_string()))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(serde_json::json!({
                "instance_id": r.get::<_, String>(0)?,
                "agent": r.get::<_, String>(1)?,
                "format": r.get::<_, Option<String>>(2)?,
                "health": r.get::<_, String>(3)?,
                "enabled": r.get::<_, i64>(4)? != 0,
                "origin_host_id": r.get::<_, String>(5)?,
                "user_id": r.get::<_, String>(6)?,
                "last_success_ms": r.get::<_, Option<i64>>(12)?,
                "compat_files": r.get::<_, i64>(13)?,
                "degraded_files": r.get::<_, i64>(14)?,
                "unsupported_files": r.get::<_, i64>(15)?,
                "incompatible_files": r.get::<_, i64>(16)?,
                "available": r.get::<_, Option<String>>(17)?.and_then(|path| std::path::Path::new(&path).try_exists().ok()),
                "schedule": match r.get::<_, Option<String>>(7)? {
                    Some(kind) => serde_json::json!({
                        "kind": kind,
                        "intervalSeconds": r.get::<_, Option<i64>>(8)?,
                        "timeOfDay": r.get::<_, Option<String>>(9)?,
                        "weekday": r.get::<_, Option<i64>>(10)?,
                        "nextDueMs": r.get::<_, Option<i64>>(11)?,
                    }),
                    None => serde_json::Value::Null,
                },
            }))
        })
        .map_err(|e| err("db", e.to_string()))?;
    let mut sources: Vec<serde_json::Value> = rows
        .collect::<Result<_, _>>()
        .map_err(|e| err("db", e.to_string()))?;
    drop(stmt);
    // 失踪文件计数（SQLite 无文件系统访问，磁盘检查逐实例做；只统计不改状态）。
    for s in sources.iter_mut() {
        let instance = s["instance_id"].as_str().unwrap_or("").to_string();
        let mut stmt = storage
            .conn()
            .prepare("SELECT file_id FROM source_files WHERE instance_id = ?1")
            .map_err(|e| err("db", e.to_string()))?;
        let files = stmt
            .query_map(rusqlite::params![instance], |r| r.get::<_, String>(0))
            .and_then(|rows| rows.collect::<Result<Vec<_>, _>>())
            .map_err(|e| err("db", e.to_string()))?;
        let missing = files
            .iter()
            .filter(|f| !std::path::Path::new(f.as_str()).exists())
            .count();
        s["missing_files"] = serde_json::json!(missing);
    }
    Ok(serde_json::json!({ "sources": sources }))
}

/// 启用/停用来源（停用后自动提取不再读取该实例）。
#[tauri::command]
pub fn set_source_enabled(
    state: tauri::State<'_, Arc<AppState>>,
    instance_id: String,
    enabled: bool,
) -> Result<(), String> {
    let storage = state.storage.lock().unwrap();
    storage
        .conn()
        .execute(
            "UPDATE source_instances SET enabled = ?2, updated_at_ms = ?3 WHERE instance_id = ?1",
            rusqlite::params![instance_id, enabled as i64, now_ms()],
        )
        .map_err(|e| err("db", e.to_string()))?;
    Ok(())
}

/// 设置逐源提取计划（rule=None 删除，恢复继承全局；M6 逐源定时接线）。
/// 固定间隔 15s–24h / 每日 HH:MM / 每周 ISO weekday+HH:MM；时区继承统计时区。
#[tauri::command]
pub fn set_source_schedule(
    state: tauri::State<'_, Arc<AppState>>,
    instance_id: String,
    rule: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let tz = state.settings.lock().unwrap().timezone.clone();
    let storage_lock = state.storage.lock().unwrap();
    let now = now_ms();
    match rule {
        None => {
            llm_usage_core::schedules::delete_source_schedule(&storage_lock, &instance_id)
                .map_err(|e| err("db", e.to_string()))?;
        }
        Some(value) => {
            let parsed = llm_usage_core::schedules::SourceScheduleRule {
                instance_id: instance_id.clone(),
                rule_kind: value
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                interval_seconds: value.get("intervalSeconds").and_then(|v| v.as_i64()),
                time_of_day: value
                    .get("timeOfDay")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                weekday: value.get("weekday").and_then(|v| v.as_i64()),
                tz: String::new(),
                enabled: true,
            };
            llm_usage_core::schedules::upsert_source_schedule(&storage_lock, &parsed, now, &tz)
                .map_err(|e| err("db", e.to_string()))?;
        }
    }
    // 回读新状态（nextDue 供 UI 显示）。
    let current = llm_usage_core::schedules::source_schedule(&storage_lock, &instance_id)
        .map_err(|e| err("db", e.to_string()))?;
    let next_due: Option<i64> = storage_lock
        .conn()
        .query_row(
            "SELECT next_due_at_ms FROM extraction_schedules WHERE schedule_id = ?1",
            [format!("source:{instance_id}")],
            |r| r.get(0),
        )
        .unwrap_or(None);
    Ok(serde_json::json!({
        "instanceId": instance_id,
        "schedule": current.as_ref().map(|r| serde_json::json!({
            "kind": r.rule_kind,
            "intervalSeconds": r.interval_seconds,
            "timeOfDay": r.time_of_day,
            "weekday": r.weekday,
            "enabled": r.enabled,
        })),
        "nextDueMs": next_due,
    }))
}

/// 触发一次手动刷新（已在执行时合并返回）。
/// 后台线程执行（同步命令会阻塞主线程冻结 UI；整轮扫描可达分钟级），
/// 进度经 state.refresh 由顶栏轮询展示。
#[tauri::command]
pub fn refresh_sources(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let already = state.refresh.lock().unwrap().running;
    if !already {
        let state = state.inner().clone();
        std::thread::spawn(move || {
            run_refresh(&state, TriggerKind::Manual);
        });
    }
    let refresh = state.refresh.lock().unwrap();
    Ok(serde_json::json!({
        // 后台启动是乐观值：极小并发窗口内重复触发由 run_refresh 合并。
        "started": !already,
        "running": refresh.running || !already,
        "last_finished_ms": refresh.last_finished_ms,
    }))
}

#[tauri::command]
pub fn refresh_status(state: tauri::State<'_, Arc<AppState>>) -> Result<serde_json::Value, String> {
    let refresh = state.refresh.lock().unwrap();
    serde_json::to_value(&*refresh).map_err(|e| err("serialize", e.to_string()))
}

#[tauri::command]
pub fn get_settings(state: tauri::State<'_, Arc<AppState>>) -> Result<AppSettings, String> {
    Ok(state.settings.lock().unwrap().clone())
}

#[tauri::command]
pub fn set_settings(
    state: tauri::State<'_, Arc<AppState>>,
    mut settings: AppSettings,
) -> Result<(), String> {
    // Receiver lifecycle is owned by telemetry setup. A settings form opened earlier
    // must not silently turn it back off when saving unrelated preferences.
    if let Some(w) = settings.week_start {
        week_start_of(w)?;
    }
    // 分级保留层级合法性（复用 core 校验）。
    llm_usage_core::retention_tiered::TieredRetentionPolicy {
        events_days: settings.retention.events_days,
        hourly_days: settings.retention.hourly_days,
        daily_days: settings.retention.daily_days,
        weekly_days: settings.retention.weekly_days,
        monthly_days: settings.retention.monthly_days,
        yearly_days: settings.retention.yearly_days,
    }
    .validate()
    .map_err(|e| err("invalid_retention", e.to_string()))?;
    if settings.timezone != "UTC" {
        llm_usage_core::calendar::Calendar::new(&settings.timezone).map_err(|_| {
            err(
                "invalid_timezone",
                format!("unknown timezone {:?}", settings.timezone),
            )
        })?;
    }
    // F2 费用设置校验：供应商默认字段非空去空格；TTL 仅 5/60 分钟档。
    for d in &settings.pricing.provider_defaults {
        for (name, value) in [
            ("provider_id", &d.provider_id),
            ("region", &d.region),
            ("channel", &d.channel),
        ] {
            if value.trim().is_empty() {
                return Err(err(
                    "invalid_pricing",
                    format!("provider default {name} is empty"),
                ));
            }
        }
        if let Some(ttl) = d.cache_ttl_minutes {
            if ttl != llm_usage_core::pricing::CACHE_TTL_5M_MINUTES
                && ttl != llm_usage_core::pricing::CACHE_TTL_1H_MINUTES
            {
                return Err(err(
                    "invalid_pricing",
                    format!("cache TTL must be 5 or 60 minutes, got {ttl}"),
                ));
            }
        }
    }
    // F2 在线刷新：缓存 TTL 天数范围（1–365）。
    if settings.pricing.online_cache_ttl_days < crate::price_refresh::MIN_TTL_DAYS
        || settings.pricing.online_cache_ttl_days > crate::price_refresh::MAX_TTL_DAYS
    {
        return Err(err(
            "invalid_pricing",
            format!(
                "online cache TTL must be {}–{} days, got {}",
                crate::price_refresh::MIN_TTL_DAYS,
                crate::price_refresh::MAX_TTL_DAYS,
                settings.pricing.online_cache_ttl_days
            ),
        ));
    }
    {
        let storage = state.storage.lock().unwrap();
        let mut live = state.settings.lock().unwrap();
        settings.otel_receiver_enabled = live.otel_receiver_enabled;
        settings.otel_receiver_port = live.otel_receiver_port;
        // 时区变更 ⇒ 在新时区重算日分区（事件仍在 ⇒ 推导；封存日跳过）。
        let old_tz = live.timezone.clone();
        if old_tz != settings.timezone {
            crate::app_state::repair_tz_partitions(&storage, &settings.timezone, true)?;
        }
        save_settings(&storage, &settings)?;
        *live = settings;
        log_operation(&storage, "settings_changed", "user settings updated");
    }
    Ok(())
}

#[tauri::command]
pub fn app_info(state: tauri::State<'_, Arc<AppState>>) -> Result<serde_json::Value, String> {
    let storage = state.storage.lock().unwrap();
    let revision = storage
        .data_revision()
        .map_err(|e| err("db", e.to_string()))?;
    let schema = storage
        .schema_version()
        .map_err(|e| err("db", e.to_string()))?;
    drop(storage);
    Ok(serde_json::json!({
        "schema_version": schema,
        "data_revision": revision,
        "db_path": state.db_path.to_string_lossy(),
        "host_id": state.host_id.lock().unwrap().clone(),
        "exchange_format_version": EXCHANGE_FORMAT_VERSION,
    }))
}

/// 额度总览（通用 quota_history 最新快照；agent=None 取全部）。
/// 请求/额度计数，非 token，独立展示。
#[tauri::command]
pub fn quota_summary(
    state: tauri::State<'_, Arc<AppState>>,
    agent: Option<String>,
) -> Result<serde_json::Value, String> {
    let storage = state.storage.lock().unwrap();
    let latest = llm_usage_core::quota_history::latest(&storage, agent.as_deref())
        .map_err(|e| err("db", e.to_string()))?;
    let quotas: Vec<serde_json::Value> = latest
        .iter()
        .map(|q| {
            serde_json::json!({
                "agent": q.agent,
                "quota_id": q.quota_id,
                "kind": q.kind,
                "unit": q.unit,
                "limit_value": q.limit_value,
                "used": q.used,
                "remaining": q.remaining,
                "percent_remaining": q.percent_remaining,
                "locality_verified": q.locality_verified,
                "observed_at_ms": q.observed_at_ms,
            })
        })
        .collect();
    Ok(serde_json::json!({ "quotas": quotas }))
}

/// 某 (agent, quota_id) 的每日额度趋势（used=已用请求/额度数）。
#[tauri::command]
pub fn quota_series(
    state: tauri::State<'_, Arc<AppState>>,
    agent: String,
    quota_id: String,
) -> Result<serde_json::Value, String> {
    let timezone = state.settings.lock().unwrap().timezone.clone();
    let storage = state.storage.lock().unwrap();
    let series =
        llm_usage_core::quota_history::daily_series(&storage, &agent, &quota_id, &timezone)
            .map_err(|e| err("db", e.to_string()))?;
    let points: Vec<serde_json::Value> = series
        .iter()
        .map(|p| {
            serde_json::json!({
                "local_day": p.local_day,
                "used": p.used,
                "remaining": p.remaining,
                "limit_value": p.limit_value,
            })
        })
        .collect();
    Ok(serde_json::json!({ "agent": agent, "quota_id": quota_id, "points": points }))
}

/// 导出：summary-csv（展示用）或 exchange（无损交换 JSON，M1a 合同）。
/// target_dir 为空时写应用数据目录 exports/ 下；返回写入的完整路径。
/// CSV 防公式注入：以 = + - @ 开头的单元格加 `'` 前缀。
#[tauri::command]
pub fn export_data(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    kind: String,
    target_dir: Option<String>,
    q: SummaryQuery,
    user_filter: Option<String>,
    host_filter: Option<String>,
) -> Result<serde_json::Value, String> {
    let settings = state.settings.lock().unwrap().clone();
    let request = build_request(&settings, &q, Vec::new())?;
    let dir = match target_dir {
        Some(d) => std::path::PathBuf::from(d),
        None => app
            .path()
            .app_data_dir()
            .map_err(|e| err("path", e.to_string()))?
            .join("exports"),
    };
    std::fs::create_dir_all(&dir).map_err(|e| err("io", e.to_string()))?;
    match kind.as_str() {
        "summary-csv" => {
            let storage = crate::app_state::read_conn(&state);
            let s = query_summary(&storage, &request).map_err(|e| err("query", e.to_string()))?;
            drop(storage);
            let path = dir.join(format!("usage-{}-{}.csv", q.first_day, q.last_day));
            let mut w = std::io::BufWriter::new(
                std::fs::File::create(&path).map_err(|e| err("io", e.to_string()))?,
            );
            let guard = |v: &str| -> String {
                if v.starts_with('=')
                    || v.starts_with('+')
                    || v.starts_with('-')
                    || v.starts_with('@')
                {
                    format!("'{v}")
                } else {
                    v.to_string()
                }
            };
            writeln!(
                w,
                "period,start_day,end_day,in_progress,partial_history,calls,input_total_known,cache_read_known,output_total_known,total_tokens_known,unknown_token_fields"
            )
            .map_err(|e| err("io", e.to_string()))?;
            for p in &s.periods {
                writeln!(
                    w,
                    "{},{},{},{},{},{},{},{},{},{},{}",
                    guard(&p.label),
                    p.start_day,
                    p.end_day,
                    p.in_progress,
                    p.partial_history,
                    p.sums.call_count,
                    p.sums
                        .input_total_known
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                    p.sums
                        .cache_read_known
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                    p.sums
                        .output_total_known
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                    p.sums
                        .total_tokens_known
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                    p.sums.input_unknown_count + p.sums.output_unknown_count,
                )
                .map_err(|e| err("io", e.to_string()))?;
            }
            w.flush().map_err(|e| err("io", e.to_string()))?;
            {
                let writer = state.storage.lock().unwrap();
                log_operation(
                    &writer,
                    "export_completed",
                    &format!("exported {kind} to {path:?}"),
                );
            }
            Ok(serde_json::json!({ "path": path.to_string_lossy(), "kind": kind }))
        }
        "exchange" => {
            let storage = crate::app_state::read_conn(&state);
            // 按用户/主机过滤导出范围（默认当前用户+当前主机）。
            let instances: Vec<String> = {
                let mut sql = String::from("SELECT instance_id FROM source_instances WHERE 1=1");
                let mut vals: Vec<rusqlite::types::Value> = Vec::new();
                if let Some(user) = &user_filter {
                    vals.push(user.clone().into());
                    sql.push_str(&format!(" AND user_id = ?{}", vals.len()));
                }
                if let Some(host) = &host_filter {
                    vals.push(host.clone().into());
                    sql.push_str(&format!(" AND origin_host_id = ?{}", vals.len()));
                }
                storage
                    .conn()
                    .prepare(&sql)
                    .and_then(|mut stmt| {
                        let rows =
                            stmt.query_map(rusqlite::params_from_iter(vals), |r| r.get(0))?;
                        rows.collect::<Result<Vec<String>, _>>()
                    })
                    .map_err(|e| err("db", e.to_string()))?
            };
            let export = build_aggregate_export(
                &storage,
                &ExportRequest {
                    timezone: settings.timezone.clone(),
                    from_ms: 0,
                    to_ms: now_ms() + 86_400_000,
                    instances: Some(instances),
                    redact_hostnames: true,
                    kind: ExchangeKind::FullSnapshot,
                    batch_id: format!("export-{}", now_ms()),
                },
                now_ms(),
            )
            .map_err(|e| err("export", e.to_string()))?;
            drop(storage);
            let path = dir.join(format!("exchange-{}.json", now_ms()));
            std::fs::write(
                &path,
                serde_json::to_vec_pretty(&export).map_err(|e| err("serialize", e.to_string()))?,
            )
            .map_err(|e| err("io", e.to_string()))?;
            Ok(serde_json::json!({ "path": path.to_string_lossy(), "kind": kind }))
        }
        other => Err(err(
            "invalid_kind",
            format!("unknown export kind {other:?}"),
        )),
    }
}

/// 应用数据库里刷新一次设置（导入/回填工具用；当前主要是测试钩子）。
#[allow(dead_code)]
#[tauri::command]
pub fn reload_settings(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    let storage = state.storage.lock().unwrap();
    let s = load_settings(&storage);
    drop(storage);
    *state.settings.lock().unwrap() = s;
    Ok(())
}

// ---- 多用户（v6）与导入/归档统计命令 ----

#[tauri::command]
pub fn list_users(state: tauri::State<'_, Arc<AppState>>) -> Result<serde_json::Value, String> {
    let storage = crate::app_state::read_conn(&state);
    let mut stmt = storage
        .conn()
        .prepare("SELECT user_id, name, created_at_ms FROM users ORDER BY user_id")
        .map_err(|e| err("db", e.to_string()))?;
    let users: Vec<serde_json::Value> = stmt
        .query_map([], |r| {
            Ok(serde_json::json!({
                "user_id": r.get::<_, String>(0)?,
                "name": r.get::<_, String>(1)?,
                "created_at_ms": r.get::<_, i64>(2)?,
            }))
        })
        .map_err(|e| err("db", e.to_string()))?
        .collect::<Result<_, _>>()
        .map_err(|e| err("db", e.to_string()))?;
    let current = state.current_user.lock().unwrap().clone();
    Ok(serde_json::json!({ "users": users, "current": current }))
}

/// 创建用户并（可选）立即切换。
#[tauri::command]
pub fn create_user(
    state: tauri::State<'_, Arc<AppState>>,
    name: String,
    switch: bool,
) -> Result<serde_json::Value, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(err("invalid_user", "name is empty"));
    }
    let user_id;
    {
        let storage = state.storage.lock().unwrap();
        user_id = insert_user(&storage, name, crate::scanner::now_ms())?;
        if switch {
            set_current_user_locked(&storage, &user_id)?;
        }
    }
    if switch {
        *state.current_user.lock().unwrap() = user_id.clone();
    }
    Ok(serde_json::json!({ "user_id": user_id }))
}

/// 切换当前统计用户（持久化 settings.current_user）。
#[tauri::command]
pub fn set_current_user(
    state: tauri::State<'_, Arc<AppState>>,
    user_id: String,
) -> Result<(), String> {
    {
        let storage = state.storage.lock().unwrap();
        let exists: bool = storage
            .conn()
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM users WHERE user_id = ?1)",
                [&user_id],
                |r| r.get(0),
            )
            .map_err(|e| err("db", e.to_string()))?;
        if !exists {
            return Err(err("unknown_user", format!("user {user_id:?} not found")));
        }
        set_current_user_locked(&storage, &user_id)?;
    }
    *state.current_user.lock().unwrap() = user_id;
    Ok(())
}

fn set_current_user_locked(
    storage: &llm_usage_core::storage::Storage,
    user_id: &str,
) -> Result<(), String> {
    storage
        .conn()
        .execute(
            "INSERT INTO settings (key, value, schema_version, updated_at_ms)
             VALUES ('current_user', ?1, 1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at_ms = excluded.updated_at_ms",
            rusqlite::params![user_id, crate::scanner::now_ms()],
        )
        .map_err(|e| err("db", e.to_string()))?;
    Ok(())
}

/// 把来源实例改归指定用户（多用户分开统计的分配入口）。
#[tauri::command]
pub fn assign_source_user(
    state: tauri::State<'_, Arc<AppState>>,
    instance_id: String,
    user_id: String,
) -> Result<(), String> {
    let storage = state.storage.lock().unwrap();
    storage
        .conn()
        .execute(
            "UPDATE source_instances SET user_id = ?2, updated_at_ms = ?3 WHERE instance_id = ?1",
            rusqlite::params![instance_id, user_id, crate::scanner::now_ms()],
        )
        .map_err(|e| err("db", e.to_string()))?;
    Ok(())
}

/// 导入聚合交换包（导出 → 导入闭环；M1a 合并规则）。
#[tauri::command]
pub fn import_exchange(
    state: tauri::State<'_, Arc<AppState>>,
    path: String,
) -> Result<serde_json::Value, String> {
    let bytes = std::fs::read(&path).map_err(|e| err("io", format!("{path:?}: {e}")))?;
    let export: llm_usage_core::exchange::ExchangeExport =
        serde_json::from_slice(&bytes).map_err(|e| err("parse", e.to_string()))?;
    let storage = state.storage.lock().unwrap();
    let outcome = llm_usage_core::exchange_import::import_aggregate(
        &storage,
        &export,
        crate::scanner::now_ms(),
    )
    .map_err(|e| err("import", e.to_string()))?;
    serde_json::to_value(&outcome).map_err(|e| err("serialize", e.to_string()))
}

/// 各归档层条目数 + 库文件占用（含 WAL）。
#[tauri::command]
pub fn storage_stats(state: tauri::State<'_, Arc<AppState>>) -> Result<serde_json::Value, String> {
    let storage = crate::app_state::read_conn(&state);
    let count = |table: &str| -> i64 {
        storage
            .conn()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap_or(0)
    };
    let db_bytes: i64 = storage
        .conn()
        .query_row(
            "SELECT page_count * page_size FROM pragma_page_count, pragma_page_size",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let wal = std::fs::metadata(format!("{}-wal", state.db_path.display()))
        .map(|m| m.len())
        .unwrap_or(0);
    Ok(serde_json::json!({
        "events": count("usage_events"),
        "hourly": count("hourly_usage"),
        "daily": count("daily_usage"),
        "period": count("period_usage"),
        "diagnostics": count("diagnostics"),
        "db_bytes": db_bytes,
        "wal_bytes": wal,
    }))
}

/// 手动清理：按"days_before 天之前"执行一次分层保留（各层 cutoff = min(设置, days_before)）。
#[tauri::command]
pub fn manual_cleanup(
    state: tauri::State<'_, Arc<AppState>>,
    days_before: u32,
) -> Result<serde_json::Value, String> {
    if days_before < 1 {
        return Err(err("invalid_days", "days_before must be >= 1"));
    }
    // 各层统一按 days_before 截断（用户"清理多久之前"语义；进行中周期保护仍生效）。
    let timezone = state.settings.lock().unwrap().timezone.clone();
    let storage = state.storage.lock().unwrap();
    let outcome = llm_usage_core::retention_tiered::enforce_tiered_retention(
        &storage,
        &timezone,
        crate::scanner::now_ms(),
        &llm_usage_core::retention_tiered::TieredRetentionPolicy {
            events_days: days_before,
            hourly_days: days_before,
            daily_days: days_before,
            weekly_days: days_before,
            monthly_days: days_before,
            yearly_days: Some(days_before),
        },
    )
    .map_err(|e| err("cleanup", e.to_string()))?;
    log_operation(
        &storage,
        "manual_cleanup",
        &format!(
            "cleaned data older than {days_before} days: {} events, {} daily rows pruned",
            outcome.deleted_events, outcome.deleted_daily_rows
        ),
    );
    Ok(serde_json::json!({
        "deleted_events": outcome.deleted_events,
        "deleted_hourly_rows": outcome.deleted_hourly_rows,
        "deleted_daily_rows": outcome.deleted_daily_rows,
        "deleted_period_rows": outcome.deleted_period_rows,
        "materialized_period_rows": outcome.materialized_period_rows,
    }))
}

/// 原生打开对话框（导入文件选择）。
#[tauri::command]
pub fn pick_open_path(extension: String) -> Option<String> {
    rfd::FileDialog::new()
        .add_filter("LLMUsage export", &[&extension])
        .pick_file()
        .map(|p| p.to_string_lossy().to_string())
}

/// 清空预检（确认层展示）：事件总量 + 已注册但磁盘已不存在的源文件数
/// （这些历史清空后无法从源重采，见 2026-09-27 zcode 数据丢失分析）。
#[tauri::command]
pub fn clear_all_preview(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let storage = crate::app_state::read_conn(&state);
    let event_count: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap_or(0);
    let mut missing = 0i64;
    let mut total = 0i64;
    let mut stmt = storage
        .conn()
        .prepare("SELECT file_id FROM source_files")
        .map_err(|e| err("db", e.to_string()))?;
    let files = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .and_then(|rows| rows.collect::<Result<Vec<_>, _>>())
        .map_err(|e| err("db", e.to_string()))?;
    for f in files {
        total += 1;
        if !std::path::Path::new(&f).exists() {
            missing += 1;
        }
    }
    Ok(
        serde_json::json!({ "event_count": event_count, "missing_files": missing, "total_files": total }),
    )
}

/// 清空前自动备份整库（共享助手：VACUUM INTO 一致快照 + 空间检查 + 保留 3 份）。
/// Agent 会清理/压实自己的源文件，清空后部分历史无法重采（2026-09-27
/// zcode 数据丢失教训）；备份给恢复留一条路。无数据时不备份。
const CLEAR_ALL_TABLES: &[&str] = &[
    "usage_events",
    "hourly_usage",
    "daily_usage",
    "daily_cost_usage",
    "period_usage",
    "diagnostics",
    "ingestion_checkpoints",
    "event_aliases",
    "source_aggregates",
    "ingest_runs",
    "quota_snapshots",
    "quota_history",
];

fn backup_before_clear(state: &Arc<AppState>) -> Result<Option<String>, String> {
    let storage = state.storage.lock().unwrap();
    let mut has_data = false;
    for table in CLEAR_ALL_TABLES {
        let present: bool = storage
            .conn()
            .query_row(&format!("SELECT EXISTS(SELECT 1 FROM {table})"), [], |r| {
                r.get(0)
            })
            .map_err(|e| err("db", format!("inspect {table} before backup: {e}")))?;
        if present {
            has_data = true;
            break;
        }
    }
    if !has_data {
        return Ok(None);
    }
    crate::db_backup::consistent_backup(
        storage.conn(),
        &state.db_path,
        "llm-usage-backup",
        crate::scanner::now_ms(),
    )
    .map(|p| p.map(|path| path.to_string_lossy().to_string()))
}

/// 清理全部数据（所有归档层+诊断+游标），并触发全量重新采集。
/// 主机身份、用户、设置保留；source_files 状态重置为 new 使探测重新执行。
/// 清空前自动备份（见 backup_before_clear）。
///
/// 后台执行（UI 不阻塞，同步命令会冻结主线程事件循环）：
/// 阶段进度经 `clear-all-progress` 事件推送——
/// waiting（等当前采集结束）→ backup → clearing → cleared（各表计数）
/// → rescan → done；任一步失败发 failed（含 error）并终止。
/// 重复触发直接返回 started=false（任务单例）。
#[tauri::command]
pub fn clear_all_data(
    state: tauri::State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    use std::sync::atomic::Ordering;
    if state.clear_job_running.swap(true, Ordering::SeqCst) {
        return Ok(serde_json::json!({ "started": false }));
    }
    let state = state.inner().clone();
    std::thread::spawn(move || {
        // panic 也要复位标志并发失败事件，否则按钮会永久停留在忙碌态。
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_clear_all_job(&state, &app)
        }))
        .unwrap_or_else(|_| Err("clear job panicked".to_string()));
        state.clear_job_running.store(false, Ordering::SeqCst);
        if let Err(e) = result {
            let _ = tauri_event(&app, serde_json::json!({ "phase": "failed", "error": e }));
        }
    });
    Ok(serde_json::json!({ "started": true }))
}

fn tauri_event(app: &tauri::AppHandle, payload: serde_json::Value) -> Result<(), String> {
    use tauri::Emitter as _;
    app.emit("clear-all-progress", payload)
        .map_err(|e| err("emit", e.to_string()))
}

/// 清理+重采后台任务主体；错误向上传递由调用方发 failed 事件。
fn run_clear_all_job(state: &Arc<AppState>, app: &tauri::AppHandle) -> Result<(), String> {
    // 清库与扫描并发会让进行中的采集把已清表回写（数据复活），
    // 先等当前采集结束再清。
    if state.refresh.lock().unwrap().running {
        let _ = tauri_event(app, serde_json::json!({ "phase": "waiting" }));
        while state.refresh.lock().unwrap().running {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }
    let _ = tauri_event(app, serde_json::json!({ "phase": "backup" }));
    let backup_path = backup_before_clear(state)?;
    let _ = tauri_event(app, serde_json::json!({ "phase": "clearing" }));
    let (cleared, revision) = clear_all_tables(state)?;
    let _ = tauri_event(
        app,
        serde_json::json!({
            "phase": "cleared",
            "cleared": cleared,
            "data_revision": revision,
            "backup": backup_path,
        }),
    );
    // 游标已重置，本轮刷新即全量重新采集；进度由 state.refresh 顶栏轮询展示。
    let _ = tauri_event(app, serde_json::json!({ "phase": "rescan" }));
    let rescan_started = crate::scanner::run_refresh_after_clear(state);
    let _ = tauri_event(
        app,
        serde_json::json!({ "phase": "done", "rescan_started": rescan_started }),
    );
    Ok(())
}

/// 清库事务：各表 DELETE + source_files 状态重置 + 保留水位设置清除 + 修订号推进。
fn clear_all_tables(
    state: &Arc<AppState>,
) -> Result<(serde_json::Map<String, serde_json::Value>, i64), String> {
    let storage = state.storage.lock().unwrap();
    let tx = storage
        .conn()
        .unchecked_transaction()
        .map_err(|e| err("db", e.to_string()))?;
    let mut cleared = serde_json::Map::new();
    for table in CLEAR_ALL_TABLES {
        let n = tx
            .execute(&format!("DELETE FROM {table}"), [])
            .map_err(|e| err("db", format!("{table}: {e}")))?;
        cleared.insert(table.to_string(), serde_json::json!(n));
    }
    // 游标清除后 source_files 的代数/身份保留（避免同文件重复探测），
    // 但状态重置为 new 让下一轮扫描重新判定。
    tx.execute("UPDATE source_files SET status = 'new'", [])
        .map_err(|e| err("db", e.to_string()))?;
    tx.execute(
        "DELETE FROM settings WHERE key='detail_retention_floor_ms'
        OR key LIKE 'daily_retention_floor:%' OR key LIKE 'retention_applied:%'
        OR key LIKE 'zcode_archive_day:%' OR key LIKE 'zcode_archive_authority:%'
        OR key='copilot_otel_scopes_v1' OR key LIKE 'cost_matching_policy:%'",
        [],
    )
    .map_err(|e| err("db", e.to_string()))?;
    let revision =
        llm_usage_core::storage::Storage::bump_data_revision_tx(&tx, crate::scanner::now_ms())
            .map_err(|e| err("db", e.to_string()))?;
    tx.commit().map_err(|e| err("db", e.to_string()))?;
    log_operation(
        &storage,
        "clear_all_data",
        "all statistics cleared; full rescan will trigger",
    );
    Ok((cleared, revision))
}

#[tauri::command]
pub fn event_details(
    state: tauri::State<'_, Arc<AppState>>,
    q: SummaryQuery,
    page: i64,
    page_size: i64,
) -> Result<serde_json::Value, String> {
    let settings = state.settings.lock().unwrap().clone();
    let current_user = state.current_user.lock().unwrap().clone();
    let storage = crate::app_state::read_conn(&state);
    let instances = user_instances(&storage, &current_user)?;
    let request = build_request(&settings, &q, instances)?;
    // 日期范围由请求给出（避免全量扫描；近 24h/当天时精确到小时）。
    let calendar = llm_usage_core::calendar::Calendar::new(&settings.timezone)
        .map_err(|e| err("calendar", e.to_string()))?;
    let (from_ms, to_ms) = calendar
        .day_range_ms(request.last_day)
        .map_err(|e| err("calendar", e.to_string()))?;
    let from_ms = if request.first_day == request.last_day {
        from_ms
    } else {
        calendar
            .day_range_ms(request.first_day)
            .map_err(|e| err("calendar", e.to_string()))?
            .0
    };
    let detail = llm_usage_core::query::event_details(
        &storage,
        &llm_usage_core::query::EventDetailRequest {
            timezone: settings.timezone.clone(),
            from_ms,
            to_ms,
            offset: page.max(0) * page_size,
            limit: page_size.clamp(1, 500),
            filters: request.filters,
        },
    )
    .map_err(|e| err("query", e.to_string()))?;
    Ok(serde_json::json!({
        "rows": detail.rows.iter().map(|r| serde_json::json!({
            "event_id": r.event_id,
            "agent": r.agent,
            "model": r.model_raw,
            "category": r.call_category,
            "occurred_at_ms": r.occurred_at_ms,
            "session": r.session_id,
            "input": opt_num(r.input_total),
            "cache_read": opt_num(r.cache_read),
            "output": opt_num(r.output_total),
            "total": opt_num(r.total_tokens),
            "duration_ms": opt_num(r.duration_ms),
            "lifecycle": r.lifecycle,
        })).collect::<Vec<_>>(),
        "total": detail.total_count,
        "page": page,
        "page_size": page_size,
    }))
}

/// 导出过滤选项：可用的用户与主机列表（含当前值）。
#[tauri::command]
pub fn export_filter_options(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let storage = crate::app_state::read_conn(&state);
    let users: Vec<(String, String)> = storage
        .conn()
        .prepare("SELECT user_id, name FROM users ORDER BY user_id")
        .and_then(|mut s| {
            s.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .and_then(|rows| rows.collect::<Result<_, _>>())
        })
        .map_err(|e| err("db", e.to_string()))?;
    let hosts: Vec<(String, Option<String>)> = storage
        .conn()
        .prepare(
            "SELECT DISTINCT s.origin_host_id,
                    (SELECT hostname FROM origin_host_names n WHERE n.host_id = s.origin_host_id
                     ORDER BY last_seen_ms DESC LIMIT 1)
             FROM source_instances s ORDER BY s.origin_host_id",
        )
        .and_then(|mut s| {
            s.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .and_then(|rows| rows.collect::<Result<_, _>>())
        })
        .map_err(|e| err("db", e.to_string()))?;
    let current_user = state.current_user.lock().unwrap().clone();
    let current_host = state.host_id.lock().unwrap().clone();
    Ok(serde_json::json!({
        "users": users.iter().map(|(id, name)| serde_json::json!({
            "user_id": id, "name": name,
            "is_current": id == &current_user,
        })).collect::<Vec<_>>(),
        "hosts": hosts.iter().map(|(id, name)| serde_json::json!({
            "host_id": id, "name": name,
            "is_current": id == &current_host,
        })).collect::<Vec<_>>(),
        "current_user": current_user,
        "current_host": current_host,
    }))
}

/// 按维度分组的时间序列（图表数据源；直接从聚合表读，低计算量）。
#[tauri::command]
pub fn chart_series(
    state: tauri::State<'_, Arc<AppState>>,
    q: SummaryQuery,
    dimension: String,
) -> Result<serde_json::Value, String> {
    let settings = state.settings.lock().unwrap().clone();
    let current_user = state.current_user.lock().unwrap().clone();
    let storage = crate::app_state::read_conn(&state);
    let instances = user_instances(&storage, &current_user)?;
    let request = build_request(&settings, &q, instances)?;
    let dim = match dimension.as_str() {
        "total" => llm_usage_core::query::ChartDimension::Total,
        "model" => llm_usage_core::query::ChartDimension::ByModel,
        "agent" => llm_usage_core::query::ChartDimension::ByAgent,
        "agent_model" => llm_usage_core::query::ChartDimension::ByAgentModel,
        other => {
            return Err(err(
                "invalid_dimension",
                format!("unknown dimension {other:?}"),
            ))
        }
    };
    let rows = llm_usage_core::query::chart_series(&storage, &request, &dim)
        .map_err(|e| err("query", e.to_string()))?;
    Ok(serde_json::json!({
        "rows": rows.iter().map(|r| serde_json::json!({
            "label": r.label,
            "series": r.series_name,
            "calls": r.call_count,
            "input": opt_num(r.input_total),
            "cache_read": opt_num(r.cache_read),
            "cache_write": opt_num(r.cache_write),
            "uncached": opt_num(r.uncached),
            "cache_ratio": r.cache_ratio,
            "output": opt_num(r.output_total),
            "total": opt_num(r.total_tokens),
        })).collect::<Vec<_>>(),
    }))
}

/// 最近诊断日志（设置页日志 Tab）。
#[tauri::command]
pub fn diagnostic_logs(
    state: tauri::State<'_, Arc<AppState>>,
    limit: i64,
    code_filter: Option<String>,
) -> Result<serde_json::Value, String> {
    let storage = crate::app_state::read_conn(&state);
    let rows = llm_usage_core::query::diagnostic_logs(
        &storage,
        limit.clamp(1, 500),
        code_filter.as_deref(),
    )
    .map_err(|e| err("query", e.to_string()))?;
    Ok(serde_json::json!({
        "rows": rows.iter().map(|r| serde_json::json!({
            "time": r.created_ms,
            "code": r.code,
            "field": r.field,
            "instance": r.instance_id,
            "message": r.message,
        })).collect::<Vec<_>>(),
    }))
}

fn insert_user(
    storage: &llm_usage_core::storage::Storage,
    name: &str,
    now: i64,
) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(err("invalid_user", "name is empty"));
    }
    storage.conn().query_row(
        "INSERT INTO users(user_id,name,created_at_ms) VALUES ('u-' || lower(hex(randomblob(16))),?1,?2) RETURNING user_id",
        rusqlite::params![name,now], |r| r.get(0),
    ).map_err(|e| err("db", e.to_string()))
}

/// Current API reference and optional selected hours; frozen daily history is preserved.
#[tauri::command]
pub async fn cost_summary(
    state: tauri::State<'_, Arc<AppState>>,
    q: SummaryQuery,
) -> Result<serde_json::Value, String> {
    let state = Arc::clone(&state);
    let settings = state.settings.lock().unwrap().clone();
    let current_user = state.current_user.lock().unwrap().clone();
    tauri::async_runtime::spawn_blocking(move || {
        cost_summary_query(&state, &settings, &current_user, &q)
    })
    .await
    .map_err(|_| err("cost_summary", "query worker failed"))?
}

fn cost_summary_query(
    state: &Arc<AppState>,
    settings: &AppSettings,
    current_user: &str,
    q: &SummaryQuery,
) -> Result<serde_json::Value, String> {
    let storage = crate::app_state::read_conn(state);
    let instances = user_instances(&storage, current_user)?;
    let request = llm_usage_core::storage::pricing::CostSummaryRequest {
        timezone: settings.timezone.clone(),
        first_day: parse_date(&q.first_day)
            .map_err(|e| err("invalid_date", format!("first_day: {e}")))?
            .to_string(),
        last_day: parse_date(&q.last_day)
            .map_err(|e| err("invalid_date", format!("last_day: {e}")))?
            .to_string(),
        filters: llm_usage_core::storage::pricing::CostFilters {
            agents: q.agents.clone(),
            providers: q.providers.clone(),
            models: q.models.clone(),
            instances: Some(instances),
        },
        now_ms: now_ms(),
        options: settings.pricing.estimate_options(),
    };
    let hours = match (
        q.first_period.as_deref(),
        q.last_period.as_deref(),
        q.granularity.as_str(),
    ) {
        (Some(first), Some(last), "hour") => Some((first, last)),
        (None, None, _) | (Some(_), Some(_), _) => None,
        _ => return Err(err("cost_summary", "incomplete period selection")),
    };
    let summary = storage
        .cost_summary_selected(&request, hours)
        .map_err(|e| err("cost_summary", e.to_string()))?;
    serde_json::to_value(&summary).map_err(|e| err("serialize", e.to_string()))
}

/// F2 显式重算费用（后台线程执行；完成写操作日志，UI 稍后刷新查看）。
#[tauri::command]
pub fn recompute_costs(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let (timezone, options) = {
        let settings = state.settings.lock().unwrap();
        (
            settings.timezone.clone(),
            settings.pricing.estimate_options(),
        )
    };
    let state = state.inner().clone();
    std::thread::spawn(move || {
        let now = crate::scanner::now_ms();
        let result = {
            let storage = state.storage.lock().unwrap();
            storage
                .recompute_unsealed_cost_days(&timezone, now, &options)
                .map(|outcomes| {
                    outcomes
                        .iter()
                        .filter(|o| o.rebuilt)
                        .map(|o| o.day.clone())
                        .collect::<Vec<_>>()
                })
        };
        let message = match result {
            Ok(days) => format!("cost recompute finished: {} days rebuilt", days.len()),
            Err(e) => format!("cost recompute failed: {e}"),
        };
        let storage = state.storage.lock().unwrap();
        log_operation(&storage, "cost_recompute", &message);
    });
    Ok(serde_json::json!({ "started": true }))
}

/// F2 价格快照列表（来源/新鲜度/行数）。
#[tauri::command]
pub fn list_price_snapshots(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<llm_usage_core::storage::pricing::PriceSnapshotInfo>, String> {
    let storage = crate::app_state::read_conn(&state);
    storage
        .list_price_snapshots()
        .map_err(|e| err("list_price_snapshots", e.to_string()))
}

/// F2 手工导入本地价格快照 JSON（schema 校验 + 区间重叠检查 + 幂等）。
#[tauri::command]
pub fn import_price_snapshot(
    state: tauri::State<'_, Arc<AppState>>,
    path: String,
) -> Result<serde_json::Value, String> {
    let text = std::fs::read_to_string(&path).map_err(|e| err("read", e.to_string()))?;
    let snapshot = llm_usage_core::pricing::parse_snapshot_json(&text)
        .map_err(|e| err("invalid_snapshot", e.to_string()))?;
    let storage = state.storage.lock().unwrap();
    let outcome = storage
        .import_price_snapshot(&snapshot, now_ms())
        .map_err(|e| err("import", e.to_string()))?;
    log_operation(
        &storage,
        "price_snapshot_import",
        &format!(
            "snapshot {} imported: {} rows (already present: {})",
            outcome.snapshot_id, outcome.inserted_rows, outcome.already_present
        ),
    );
    Ok(serde_json::json!({
        "snapshot_id": outcome.snapshot_id,
        "inserted_rows": outcome.inserted_rows,
        "already_present": outcome.already_present,
    }))
}

/// F2 在线刷新（models.dev）：手动触发（force 绕过 TTL）。需费用估算与
/// 在线刷新均已启用；后台线程执行，结果经 price_refresh_status 轮询。
#[tauri::command]
pub fn refresh_prices_online(
    state: tauri::State<'_, Arc<AppState>>,
    force: Option<bool>,
) -> Result<serde_json::Value, String> {
    {
        let settings = state.settings.lock().unwrap();
        if !settings.pricing.enabled || !settings.pricing.online_refresh_enabled {
            return Err(err(
                "disabled",
                "online price refresh is disabled (settings → costs)",
            ));
        }
    }
    let started = crate::price_refresh::maybe_auto_refresh(&state, force.unwrap_or(true));
    Ok(serde_json::json!({ "started": started }))
}

/// F2 在线刷新状态：启用位、TTL、缓存新鲜度、运行标记与最近一次结果。
#[tauri::command]
pub fn price_refresh_status(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let (enabled, ttl_days) = {
        let settings = state.settings.lock().unwrap();
        (
            settings.pricing.enabled && settings.pricing.online_refresh_enabled,
            settings.pricing.online_cache_ttl_days,
        )
    };
    let now = now_ms();
    let cache = crate::price_refresh::cache_info_for_status(&state.db_path, now);
    let refresh = state.price_refresh.lock().unwrap();
    Ok(serde_json::json!({
        "enabled": enabled,
        "ttl_days": ttl_days,
        "running": refresh.running,
        "cache": cache,
        "last_outcome": refresh.last_outcome,
    }))
}

#[cfg(test)]
mod user_tests {
    #[test]
    fn chinese_names_have_independent_ids_and_duplicate_names_are_rejected() {
        let storage = llm_usage_core::storage::Storage::open_in_memory().unwrap();
        let a = super::insert_user(&storage, "张三", 1).unwrap();
        let b = super::insert_user(&storage, "李四", 1).unwrap();
        assert_ne!(a, b);
        assert!(super::insert_user(&storage, "张三", 1).is_err());
        assert!(super::insert_user(&storage, "  ", 1).is_err());
    }
}

#[cfg(test)]
mod clear_all_tests {
    use super::*;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;

    /// 后台清理任务的清库事务：各表清空、source_files 状态重置为 new、
    /// 修订号推进；操作日志补写一条 diagnostics。
    #[test]
    fn clear_all_tables_clears_resets_status_and_bumps_revision() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/clear-all-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join(format!(
            "clear-{}-{}.sqlite",
            std::process::id(),
            crate::scanner::now_ms()
        ));
        let _ = std::fs::remove_file(&db);
        let state = Arc::new(
            crate::app_state::AppState::init(db.clone(), "test-host", false).expect("init state"),
        );
        assert!(!state.clear_job_running.load(Ordering::SeqCst));
        state.clear_job_running.store(true, Ordering::SeqCst);
        assert!(!run_refresh(&state, TriggerKind::Manual));
        state.clear_job_running.store(false, Ordering::SeqCst);
        {
            let storage = state.storage.lock().unwrap();
            storage.conn().execute_batch("INSERT INTO settings(key,value,schema_version,updated_at_ms) VALUES
                ('copilot_otel_scopes_v1','[]',1,0),('cost_matching_policy:UTC','official-reference-2',1,0);").unwrap();
            storage
                .conn()
                .execute(
                    "INSERT INTO source_instances (instance_id, agent, locality_basis, attribution_status, enabled, format, parser_version, capabilities, health, origin_host_id, created_at_ms, updated_at_ms)
                     VALUES ('inst', 'test', 'local_filesystem', 'verified', 1, 'test', 'p', '{}', 'ok', 'legacy_unknown', 1, 1)",
                    [],
                )
                .unwrap();
            storage
                .conn()
                .execute(
                    "INSERT INTO source_files (file_id, instance_id, file_identity, generation, byte_size, mtime_ms, status, first_seen_ms, last_seen_ms)
                     VALUES ('f', 'inst', 'file-1', 3, 10, 1, 'active', 1, 1)",
                    [],
                )
                .unwrap();
            storage
                .conn()
                .execute(
                    "INSERT INTO diagnostics (code, message, created_ms) VALUES ('x', 'm', 1)",
                    [],
                )
                .unwrap();
            storage
                .conn()
                .execute(
                    "INSERT INTO daily_cost_usage (
                       tz_version, local_day, instance_id, agent, provider_id, model_raw,
                       currency, kind, priced_event_count, unpriced_event_count,
                       partial_event_count, ttl_defaulted_events, total_amount_minor,
                       priced_tokens, known_tokens, price_basis, data_revision
                     ) VALUES ('UTC', '2026-09-26', 'inst', 'test', 'p', 'm',
                       'USD', 'estimate_at_time', 1, 0, 0, 0, 123, 1, 1, '[]', 0)",
                    [],
                )
                .unwrap();
        }
        // 明细已过期但日费用尚存时，清空前仍须保存一致备份。
        let backup = backup_before_clear(&state)
            .unwrap()
            .expect("backup created");
        {
            let saved = rusqlite::Connection::open(&backup).unwrap();
            let rows: i64 = saved
                .query_row("SELECT COUNT(*) FROM daily_cost_usage", [], |r| r.get(0))
                .unwrap();
            assert_eq!(rows, 1);
        }
        let (cleared, revision) = clear_all_tables(&state).expect("clear tables");
        assert_eq!(cleared.get("diagnostics").and_then(|v| v.as_i64()), Some(1));
        assert_eq!(
            cleared.get("daily_cost_usage").and_then(|v| v.as_i64()),
            Some(1)
        );
        assert!(revision >= 0);
        let storage = state.storage.lock().unwrap();
        let status: String = storage
            .conn()
            .query_row(
                "SELECT status FROM source_files WHERE file_id='f'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "new");
        // 操作日志补写一条；清空前的诊断已删除。
        let n: i64 = storage
            .conn()
            .query_row("SELECT COUNT(*) FROM diagnostics", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
        let cost_rows: i64 = storage
            .conn()
            .query_row("SELECT COUNT(*) FROM daily_cost_usage", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cost_rows, 0);
        let markers:i64=storage.conn().query_row("SELECT COUNT(*) FROM settings WHERE key='copilot_otel_scopes_v1' OR key LIKE 'cost_matching_policy:%'",[],|r|r.get(0)).unwrap();
        assert_eq!(
            markers, 0,
            "full data reset clears derived authority and matching markers"
        );
        drop(storage);
        drop(state);
        let _ = std::fs::remove_file(backup);
        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_file(db.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db.with_extension("sqlite-shm"));
    }
}
