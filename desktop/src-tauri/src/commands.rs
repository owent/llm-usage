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
fn log_operation(storage: &llm_usage_core::storage::Storage, code: &str, message: &str) {
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
pub fn summary(
    state: tauri::State<'_, Arc<AppState>>,
    q: SummaryQuery,
) -> Result<serde_json::Value, String> {
    let settings = state.settings.lock().unwrap().clone();
    let current_user = state.current_user.lock().unwrap().clone();
    let request = build_request(&settings, &q, Vec::new())?;
    // 读路径：常驻只读连接（WAL 与后台扫描并发；失败回退写连接）。
    let storage = crate::app_state::read_conn(&state);
    let instances = user_instances(&storage, &current_user)?;
    let request = SummaryRequest {
        filters: Filters {
            instances: Some(instances),
            ..request.filters
        },
        ..request
    };
    let s = query_summary(&storage, &request).map_err(|e| err("query", e.to_string()))?;
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
pub fn heatmap(
    state: tauri::State<'_, Arc<AppState>>,
    q: SummaryQuery,
) -> Result<serde_json::Value, String> {
    let settings = state.settings.lock().unwrap().clone();
    let current_user = state.current_user.lock().unwrap().clone();
    let mut request = build_request(&settings, &q, Vec::new())?;
    let storage = crate::app_state::read_conn(&state);
    request.filters.instances = Some(user_instances(&storage, &current_user)?);
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
                    (SELECT MAX(finished_ms) FROM ingest_runs r
                     WHERE r.instance_id = s.instance_id AND r.status = 'succeeded') AS last_ok_ms,
                    (SELECT COUNT(*) FROM source_files f WHERE f.instance_id = s.instance_id
                     AND f.status = 'active_compat') AS compat_files,
                    (SELECT COUNT(*) FROM source_files f WHERE f.instance_id = s.instance_id
                     AND f.status = 'degraded') AS degraded_files,
                    (SELECT COUNT(*) FROM source_files f WHERE f.instance_id = s.instance_id
                     AND f.status = 'unsupported') AS unsupported_files,
                    (SELECT COUNT(*) FROM source_files f WHERE f.instance_id = s.instance_id
                     AND f.status = 'incompatible') AS incompatible_files
             FROM source_instances s ORDER BY s.agent, s.instance_id",
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
                "last_success_ms": r.get::<_, Option<i64>>(7)?,
                "compat_files": r.get::<_, i64>(8)?,
                "degraded_files": r.get::<_, i64>(9)?,
                "unsupported_files": r.get::<_, i64>(10)?,
                "incompatible_files": r.get::<_, i64>(11)?,
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

/// 触发一次手动刷新（已在执行时合并返回）。
#[tauri::command]
pub fn refresh_sources(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let started = run_refresh(&state, TriggerKind::Manual);
    let refresh = state.refresh.lock().unwrap();
    Ok(serde_json::json!({
        "started": started,
        "running": refresh.running,
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
    settings: AppSettings,
) -> Result<(), String> {
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
    {
        let storage = state.storage.lock().unwrap();
        // 时区变更 ⇒ 在新时区重算日分区（事件仍在 ⇒ 推导；封存日跳过）。
        let old_tz = state.settings.lock().unwrap().timezone.clone();
        if old_tz != settings.timezone {
            crate::app_state::repair_tz_partitions(&storage, &settings.timezone, true)?;
        }
        save_settings(&storage, &settings)?;
    }
    {
        let storage = state.storage.lock().unwrap();
        log_operation(&storage, "settings_changed", "user settings updated");
    }
    *state.settings.lock().unwrap() = settings;
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

/// 秒级 epoch → (Y,M,D,H,Min,S)（备份文件名用；civil_from_days 算法）。
fn time_datetime(secs: i64) -> (i32, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, mi, s) = (
        (rem / 3600) as u32,
        ((rem % 3600) / 60) as u32,
        (rem % 60) as u32,
    );
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32, h, mi, s)
}

/// 清空前自动备份整库（VACUUM INTO 单文件快照；保留最近 3 份）。
/// Agent 会清理/压实自己的源文件，清空后部分历史无法重采（2026-09-27
/// zcode 数据丢失教训）；备份给恢复留一条路。无数据时不备份。
fn backup_before_clear(state: &Arc<AppState>) -> Result<Option<String>, String> {
    let storage = state.storage.lock().unwrap();
    let event_count: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap_or(0);
    if event_count == 0 {
        return Ok(None);
    }
    let backup_dir = state
        .db_path
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("backups");
    std::fs::create_dir_all(&backup_dir).map_err(|e| err("io", e.to_string()))?;
    let (y, mo, d, h, mi, s) = time_datetime(crate::scanner::now_ms() / 1000);
    let backup = backup_dir.join(format!(
        "llm-usage-backup-{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}.sqlite"
    ));
    storage
        .conn()
        .execute(
            "VACUUM INTO ?1",
            rusqlite::params![backup.to_string_lossy()],
        )
        .map_err(|e| err("db", format!("backup failed: {e}")))?;
    // 只保留最近 3 份（文件名含时间戳，字典序即时间序）。
    let mut olds: Vec<_> = std::fs::read_dir(&backup_dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| {
                    let n = e.file_name().to_string_lossy().to_string();
                    n.starts_with("llm-usage-backup-") && n.ends_with(".sqlite")
                })
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default();
    olds.sort();
    while olds.len() > 3 {
        let oldest = olds.remove(0);
        let _ = std::fs::remove_file(oldest);
    }
    Ok(Some(backup.to_string_lossy().to_string()))
}

/// 清理全部数据（所有归档层+诊断+游标），下次刷新触发全量重新采集计算。
/// 主机身份、用户、设置保留；source_files 状态重置为 new 使探测重新执行。
/// 清空前自动备份（见 backup_before_clear）。
#[tauri::command]
pub fn clear_all_data(state: tauri::State<'_, Arc<AppState>>) -> Result<serde_json::Value, String> {
    let backup_path = backup_before_clear(&state)?;
    let storage = state.storage.lock().unwrap();
    let tx = storage
        .conn()
        .unchecked_transaction()
        .map_err(|e| err("db", e.to_string()))?;
    let mut cleared = serde_json::Map::new();
    for table in [
        "usage_events",
        "hourly_usage",
        "daily_usage",
        "period_usage",
        "diagnostics",
        "ingestion_checkpoints",
        "event_aliases",
        "source_aggregates",
        "ingest_runs",
        "quota_snapshots",
    ] {
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
        OR key LIKE 'zcode_archive_day:%' OR key LIKE 'zcode_archive_authority:%'",
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
    Ok(serde_json::json!({
        "cleared": cleared,
        "data_revision": revision,
        "backup": backup_path,
    }))
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
