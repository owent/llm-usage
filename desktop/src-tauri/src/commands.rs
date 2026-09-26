//! Tauri IPC 命令：查询/来源/刷新/设置/导出。
//! DTO 合同（architecture.md）：token 等大数值用十进制字符串传输，
//! 避免前端浮点舍入；错误返回结构化 code+message，不回传内部 SQL/游标。

use crate::app_state::{load_settings, save_settings, AppSettings, AppState};
use crate::scanner::{now_ms, run_refresh};
use llm_usage_core::calendar::{parse_date, WeekStart};
use llm_usage_core::exchange::{
    build_export, ExchangeKind, ExportRequest, EXCHANGE_FORMAT_VERSION,
};
use llm_usage_core::jobs::TriggerKind;
use llm_usage_core::query::{
    agent_breakdown, heatmap_cells, hourly_breakdown, query_summary, Filters, Granularity,
    SummaryRequest,
};
use serde::Deserialize;
use std::io::Write;
use std::sync::Arc;
use tauri::Manager;

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

fn build_request(settings: &AppSettings, q: &SummaryQuery) -> Result<SummaryRequest, String> {
    let granularity = match q.granularity.as_str() {
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
        week_start: week_start_of(settings.week_start)?,
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
    })
}

#[tauri::command]
pub fn summary(
    state: tauri::State<'_, Arc<AppState>>,
    q: SummaryQuery,
) -> Result<serde_json::Value, String> {
    let settings = state.settings.lock().unwrap().clone();
    let request = build_request(&settings, &q)?;
    // 读路径：常驻只读连接（WAL 与后台扫描并发；失败回退写连接）。
    let storage = crate::app_state::read_conn(&state);
    let s = query_summary(&storage, &request).map_err(|e| err("query", e.to_string()))?;
    let agents = agent_breakdown(&storage, &request).map_err(|e| err("query", e.to_string()))?;
    let today_hourly = hourly_breakdown(
        &storage,
        &settings.timezone,
        request.today,
        &request.filters,
    )
    .map_err(|e| err("query", e.to_string()))?;
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
        "models": s.model_breakdown.iter().map(|m| serde_json::json!({
            "provider": m.provider_id,
            "model": m.model_raw,
            "sums": metric_sums_dto(&m.sums),
        })).collect::<Vec<_>>(),
        "agents": agents.iter().map(|a| serde_json::json!({
            "agent": a.agent,
            "sums": metric_sums_dto(&a.sums),
        })).collect::<Vec<_>>(),
        "today_hourly": today_hourly.iter().map(|h| serde_json::json!({
            "hour": h.hour,
            "calls": h.call_count,
            "total_tokens": opt_num(h.total_tokens_known),
            "input_total": opt_num(h.input_total_known),
            "output_total": opt_num(h.output_total_known),
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
    let request = build_request(&settings, &q)?;
    let storage = crate::app_state::read_conn(&state);
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
            "weekday": c.weekday,
            "hour": c.hour,
            "calls": c.call_count,
            "total_tokens": opt_num(c.total_tokens_known),
        })).collect::<Vec<_>>(),
    }))
}

/// 来源清单：实例注册 + 最近运行 + 兼容标记计数。
#[tauri::command]
pub fn list_sources(state: tauri::State<'_, Arc<AppState>>) -> Result<serde_json::Value, String> {
    let storage = crate::app_state::read_conn(&state);
    let mut stmt = storage
        .conn()
        .prepare(
            "SELECT s.instance_id, s.agent, s.format, s.health, s.enabled, s.origin_host_id,
                    (SELECT MAX(finished_ms) FROM ingest_runs r
                     WHERE r.instance_id = s.instance_id AND r.status = 'succeeded') AS last_ok_ms,
                    (SELECT COUNT(*) FROM source_files f WHERE f.instance_id = s.instance_id
                     AND f.status = 'active_compat') AS compat_files,
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
                "last_success_ms": r.get::<_, Option<i64>>(6)?,
                "compat_files": r.get::<_, i64>(7)?,
                "incompatible_files": r.get::<_, i64>(8)?,
            }))
        })
        .map_err(|e| err("db", e.to_string()))?;
    let sources: Vec<_> = rows
        .collect::<Result<_, _>>()
        .map_err(|e| err("db", e.to_string()))?;
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
    week_start_of(settings.week_start)?;
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
        save_settings(&storage, &settings)?;
        // 时区变更 ⇒ 在新时区重算日分区（事件仍在 ⇒ 推导；封存日跳过）。
        let old_tz = state.settings.lock().unwrap().timezone.clone();
        if old_tz != settings.timezone {
            crate::app_state::repair_tz_partitions(&storage, &settings.timezone);
        }
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
) -> Result<serde_json::Value, String> {
    let settings = state.settings.lock().unwrap().clone();
    let request = build_request(&settings, &q)?;
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
            Ok(serde_json::json!({ "path": path.to_string_lossy(), "kind": kind }))
        }
        "exchange" => {
            let storage = crate::app_state::read_conn(&state);
            let export = build_export(
                &storage,
                &ExportRequest {
                    timezone: settings.timezone.clone(),
                    from_ms: 0,
                    to_ms: now_ms() + 86_400_000,
                    instances: Vec::new(),
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
