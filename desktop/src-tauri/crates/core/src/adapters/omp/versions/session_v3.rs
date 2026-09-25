//! oh-my-pi（omp）session JSONL 格式实现（`session_v3`）。
//!
//! 格式证据（本机真实数据，18.2.7 scoop 安装，58 个会话文件 2026-08-21 至 2026-09-24，
//! 逐类型白名单实读核验；另有固定源码 oh-my-pi 62bc57b 与 omp.exe 二进制字符串佐证）：
//! - 布局：`~/.omp/agent/sessions/<encoded-cwd>/<ts>_<uuid>.jsonl`（主会话）；
//!   子 Agent 文件在 `<ts>_<父uuid>/<Name>.jsonl`，嵌套子 Agent 再深一层
//!   `<ts>_<父uuid>/<Name>/<Name>.<sub>.jsonl`；会话目录内还有 .json/.md/.log 伴生文件
//!   （不读）。全部 58 个文件首行均为 `type:"title"`（{v:1,title,updatedAt,pad,...}，
//!   标题正文不读），session 头在其后。
//! - session 头 `version`=3（58/58）；无 fork（parentSession 字段本机未出现，
//!   仍按 pi 同口径支持）；条目基座 {type,id,parentId,timestamp}。
//! - model_change 落盘为组合字段 `model`="provider/model"（3 个脱敏 fixture 一致），
//!   非 pi 的分字段 modelId/provider（分字段形状作后备解析，omp 本机未观测）。
//! - assistant 条目（8752 条实读）：model/provider/stopReason/responseId 自带；
//!   usage{input,output,cacheRead,cacheWrite,totalTokens,cost} 全部在场（含 error/aborted）；
//!   `reasoningTokens`⊆output 可选（85 条）；`duration`/`ttft` 浮点毫秒（omp 特有）。
//!   不变量逐条成立：totalTokens = input+output+cacheRead+cacheWrite。
//! - toolResult 无 usage（6643/6643）；compaction（37）与 branch_summary（2）均无 usage
//!   （tokensBefore/tokensAfter 是上下文估算，不计账）；独立 usage 条目未出现（仍支持）。
//! - 其他已观测类型（忽略）：title/title_change/credential_pin/session_init/
//!   ttsr_injection/service_tier_change/custom(tool_execution_start 等)/custom_message。
//! - 日志侧写：~/.omp/logs/omp.*.log 仅见上下文估算 debug 行，无逐次用量；
//!   title-generator 调用在本机日志未观测，不存在与会话记录的重叠证据。
//!
//! 版本策略（architecture.md#unknown-version，V30）：session 头经
//! [`super::super::versions::select`] 分派；已收录版本（3）按映射用本实现
//! （KnownVersion），未收录/缺失版本用本实现（当前最新）兼容尝试并带
//! `parse_basis` 标记，不因版本号未收录直接拒绝（omp 旧版落盘格式未取证，
//! 与 pi 不同，无 evidenced-incompatible 分支）。
//!
//! V30 目录迁移自根级 adapters/omp.rs，扫描行为不变，不重建来源、不重置游标。

use crate::domain::{CallCategory, EventInput, ModelAttribution, VersionBasis};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::jsonl::{read_jsonl, JsonlCursor, StopReason};
use crate::adapters::pi::{
    build_pi_family_event, diag, family_entry_key, json_str, parse_entry_ts, parse_usage,
    UsageEventBase,
};
use crate::adapters::usage_map::PiFamilyUsage;

pub const OMP_PARSER_VERSION: &str = "omp-session-1";

/// 持久化解析上下文（会话身份、模型状态、未知类型登记、版本选择依据）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct OmpParseContext {
    session_id: Option<String>,
    parent_session: Option<String>,
    header_version: Option<i64>,
    model: Option<String>,
    model_provider: Option<String>,
    #[serde(default)]
    unknown_types: Vec<String>,
    /// 版本选择依据（known_version / latest_fallback）；旧解析上下文缺省为 None，
    /// V30 目录迁移不重建来源、不重置游标。
    #[serde(default)]
    version_basis: Option<VersionBasis>,
}

fn restore_cursor(stored: &StoredScanState, generation: i64, rescan: bool) -> JsonlCursor {
    if rescan {
        return JsonlCursor {
            generation,
            offset: 0,
            line_number: 1,
        };
    }
    stored
        .cursor
        .as_ref()
        .and_then(|v| serde_json::from_value::<JsonlCursor>(v.clone()).ok())
        .filter(|c| c.generation == generation)
        .unwrap_or(JsonlCursor {
            generation,
            offset: 0,
            line_number: 1,
        })
}

fn restore_context(stored: &StoredScanState, rescan: bool) -> OmpParseContext {
    if rescan {
        return OmpParseContext::default();
    }
    stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<OmpParseContext>(v.clone()).ok())
        .unwrap_or_default()
}

/// omp 命名空间下的条目键。
fn entry_key(prefix: &str, entry: &serde_json::Value) -> String {
    family_entry_key("omp", prefix, entry)
}

/// 浮点毫秒 → i64 毫秒（四舍五入）；非有限/负值/溢出返回 None（调用方记诊断）。
fn float_ms(value: Option<&serde_json::Value>) -> Option<Option<i64>> {
    match value {
        None => Some(None),
        Some(v) => {
            let f = v.as_f64()?;
            if !f.is_finite() || f < 0.0 || f > i64::MAX as f64 {
                return None;
            }
            Some(Some(f.round() as i64))
        }
    }
}

/// `<ts>_<uuid>` 形状判定（如 `2026-08-21T02-19-22-638Z_01a0221d-...`）：
/// 首个下划线前是 ISO 形时间戳（数字开头、含 'T'），后是 UUID 形 ID。
/// 本机 58 文件实读：主会话文件名全部匹配，子 Agent 文件名与中间目录均不匹配。
fn is_session_dir_shape(name: &str) -> bool {
    let Some(pos) = name.find('_') else {
        return false;
    };
    let (ts, uuid) = name.split_at(pos);
    let uuid = &uuid[1..];
    ts.len() >= 20
        && ts.starts_with(|c: char| c.is_ascii_digit())
        && ts.contains('T')
        && uuid.len() >= 8
        && uuid.starts_with(|c: char| c.is_ascii_hexdigit())
}

/// 主会话文件名 `<ts>_<uuid>.jsonl`（剥扩展名后同上形状）。
fn is_main_session_file(path: &Path) -> bool {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(is_session_dir_shape)
        .unwrap_or(false)
}

/// 子 Agent 判定：文件名不是 `<ts>_<uuid>.jsonl` 形状时，父会话 UUID 取最近的
/// `<ts>_<uuid>` 形状祖先目录名（首个下划线之后部分）。嵌套子 Agent（Named/ 目录）
/// 隔代不归名，归到最近的会话目录。
fn subagent_parent_from_path(path: &Path) -> Option<String> {
    if is_main_session_file(path) {
        return None;
    }
    for ancestor in path.ancestors().skip(1) {
        let Some(name) = ancestor.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if is_session_dir_shape(name) {
            let pos = name.find('_').unwrap();
            return Some(name[pos + 1..].to_string());
        }
    }
    None
}

/// omp 会话事件构造：duration/ttft 浮点毫秒取整；其余与 pi 共享。
/// V30：版本选择依据（known_version / latest_fallback）取自解析上下文，
/// 经 pi 家族共享构造器随事件落库。
#[allow(clippy::too_many_arguments)]
fn build_usage_event(
    target: &ScanTarget,
    context: &OmpParseContext,
    base: &UsageEventBase<'_>,
    usage: Option<(&PiFamilyUsage, &serde_json::Value)>,
    occurred_ms: i64,
    source_time: &str,
    line: u64,
    now_ms: i64,
    duration_ms: Option<i64>,
    ttft_ms: Option<i64>,
    parent_from_path: Option<&str>,
    diagnostics: &mut Vec<DiagnosticInput>,
) -> EventInput {
    build_pi_family_event(
        target,
        "oh-my-pi",
        OMP_PARSER_VERSION,
        context.session_id.as_deref(),
        // 头里的 parentSession（fork）优先；否则用目录推定的父会话。
        context.parent_session.as_deref().or(parent_from_path),
        context.header_version,
        context.version_basis,
        base,
        duration_ms,
        ttft_ms,
        usage,
        occurred_ms,
        source_time,
        line,
        now_ms,
        diagnostics,
    )
}

/// 增量扫描一个 omp session JSONL 文件（统一入口 `OmpAdapter::scan` 分派到本实现）。
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let cursor = restore_cursor(stored, target.generation, target.rescan);
    let mut context = restore_context(stored, target.rescan);
    let mut events: Vec<EventInput> = Vec::new();
    let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
    let mut records_seen: u64 = 0;
    // 子 Agent 判定：按文件名/祖先目录的 <ts>_<uuid> 形状（不依赖根路径，
    // 避免规范化路径与枚举路径的分隔符差异）。
    let parent_from_path = subagent_parent_from_path(&target.path);
    let outcome = read_jsonl(
        &target.path,
        cursor.offset,
        cursor.line_number,
        &limits.jsonl,
    )?;
    for bad in &outcome.bad_lines {
        diagnostics.push(diag(
            bad.code,
            None,
            bad.number,
            "line is not valid UTF-8; isolated, content not stored",
        ));
    }
    for raw in &outcome.lines {
        records_seen += 1;
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(&raw.text) else {
            diagnostics.push(diag(
                "bad_json_line",
                None,
                raw.number,
                "line is not valid JSON; isolated, content not stored",
            ));
            continue;
        };
        let entry_type = entry.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match entry_type {
            "session" => {
                if context.session_id.is_some() {
                    diagnostics.push(diag(
                        "unexpected_session_header",
                        Some("type"),
                        raw.number,
                        "second session header in one file; first kept",
                    ));
                    continue;
                }
                // 版本分派（探测/扫描同一注册表）：已收录按映射；未收录/缺失回退
                // 本实现（当前最新）继续解析并带兼容标记，不直接拒绝（V30；
                // omp 旧版落盘格式未取证，与 pi 的 fail closed 分支不同）。
                let found = entry.get("version").and_then(|v| v.as_i64());
                let selection = super::super::versions::select(found);
                context.version_basis = Some(selection.basis);
                context.header_version = found;
                context.session_id = json_str(&entry, "id").map(str::to_string);
                context.parent_session = json_str(&entry, "parentSession").map(str::to_string);
                if context.session_id.is_none() {
                    diagnostics.push(diag(
                        "missing_session_id",
                        Some("id"),
                        raw.number,
                        "session header without id; events fall back to no session identity",
                    ));
                }
            }
            "model_change" => {
                // 本机真实形状（3 个脱敏 fixture 一致）：`model` 为
                // "provider/model" 组合字段；pi 继承形状 modelId/provider
                // 分字段作后备（omp 本机未观测）。
                if let Some(combined) = json_str(&entry, "model") {
                    let (provider, model) = combined.split_once('/').unwrap_or(("", combined));
                    context.model = Some(model.to_string());
                    if !provider.is_empty() {
                        context.model_provider = Some(provider.to_string());
                    }
                } else {
                    if let Some(model) = json_str(&entry, "modelId") {
                        context.model = Some(model.to_string());
                    }
                    if let Some(provider) = json_str(&entry, "provider") {
                        context.model_provider = Some(provider.to_string());
                    }
                }
            }
            "message" => {
                let message = entry
                    .get("message")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let role = message.get("role").and_then(|r| r.as_str()).unwrap_or("");
                match role {
                    "assistant" => {
                        let usage_json = message.get("usage").cloned();
                        let Some((occurred_ms, source_time)) = parse_entry_ts(&entry) else {
                            diagnostics.push(diag(
                                "timestamp_unparseable",
                                Some("timestamp"),
                                raw.number,
                                "entry timestamp missing or unparseable; record skipped",
                            ));
                            continue;
                        };
                        let stop_reason = message
                            .get("stopReason")
                            .and_then(|s| s.as_str())
                            .unwrap_or("");
                        let error_status = match stop_reason {
                            "error" => Some("error".to_string()),
                            "aborted" => Some("aborted".to_string()),
                            _ => None,
                        };
                        let duration_ms = match float_ms(message.get("duration")) {
                            Some(v) => v,
                            None => {
                                diagnostics.push(diag(
                                    "duration_shape_deviation",
                                    Some("message.duration"),
                                    raw.number,
                                    "duration not a finite non-negative number; left unknown",
                                ));
                                None
                            }
                        };
                        let ttft_ms = match float_ms(message.get("ttft")) {
                            Some(v) => v,
                            None => {
                                diagnostics.push(diag(
                                    "ttft_shape_deviation",
                                    Some("message.ttft"),
                                    raw.number,
                                    "ttft not a finite non-negative number; left unknown",
                                ));
                                None
                            }
                        };
                        let base = UsageEventBase {
                            key: entry_key("message", &entry),
                            category: if parent_from_path.is_some() {
                                CallCategory::SubAgent
                            } else {
                                CallCategory::Primary
                            },
                            provider: json_str(&message, "provider"),
                            model: json_str(&message, "model"),
                            attribution: ModelAttribution::RequestField,
                            origin_call_id: json_str(&message, "responseId"),
                            error_status,
                        };
                        match usage_json.as_ref().and_then(parse_usage) {
                            Some(usage) => {
                                events.push(build_usage_event(
                                    target,
                                    &context,
                                    &base,
                                    Some((&usage, usage_json.as_ref().unwrap())),
                                    occurred_ms,
                                    &source_time,
                                    raw.number,
                                    now_ms,
                                    duration_ms,
                                    ttft_ms,
                                    parent_from_path.as_deref(),
                                    &mut diagnostics,
                                ));
                            }
                            None => {
                                // 无 usage 的 assistant 消息仍是一次调用的证据：
                                // 计调用数，token 全未知（不补零）。本机 8752 条均带 usage。
                                diagnostics.push(diag(
                                    "usage_shape_deviation",
                                    Some("message.usage"),
                                    raw.number,
                                    "assistant message without complete usage; call counted, tokens unknown",
                                ));
                                events.push(build_usage_event(
                                    target,
                                    &context,
                                    &base,
                                    None,
                                    occurred_ms,
                                    &source_time,
                                    raw.number,
                                    now_ms,
                                    duration_ms,
                                    ttft_ms,
                                    parent_from_path.as_deref(),
                                    &mut diagnostics,
                                ));
                            }
                        }
                    }
                    "toolResult" => {
                        // 工具执行自身的 usage（不进主上下文记账）→ 辅助调用。
                        // 本机 6643 条均无 usage。
                        let Some(usage_json) = message.get("usage").cloned() else {
                            continue;
                        };
                        let Some(usage) = parse_usage(&usage_json) else {
                            diagnostics.push(diag(
                                "usage_shape_deviation",
                                Some("message.usage"),
                                raw.number,
                                "toolResult usage missing required numeric fields; skipped",
                            ));
                            continue;
                        };
                        let Some((occurred_ms, source_time)) = parse_entry_ts(&entry) else {
                            diagnostics.push(diag(
                                "timestamp_unparseable",
                                Some("timestamp"),
                                raw.number,
                                "entry timestamp missing or unparseable; record skipped",
                            ));
                            continue;
                        };
                        let base = UsageEventBase {
                            key: entry_key("toolresult", &entry),
                            category: CallCategory::Auxiliary,
                            provider: None,
                            model: None,
                            attribution: ModelAttribution::Unknown,
                            origin_call_id: None,
                            error_status: None,
                        };
                        events.push(build_usage_event(
                            target,
                            &context,
                            &base,
                            Some((&usage, &usage_json)),
                            occurred_ms,
                            &source_time,
                            raw.number,
                            now_ms,
                            None,
                            None,
                            parent_from_path.as_deref(),
                            &mut diagnostics,
                        ));
                    }
                    _ => {}
                }
            }
            "usage" => {
                // 独立 usage 条目（kind 如 cache_warm）：辅助调用，provider/model 自有字段。
                // 本机未出现，与 pi 同口径支持。
                let usage_json = entry
                    .get("usage")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let Some(usage) = parse_usage(&usage_json) else {
                    diagnostics.push(diag(
                        "usage_shape_deviation",
                        Some("usage"),
                        raw.number,
                        "usage entry missing required numeric fields; skipped",
                    ));
                    continue;
                };
                let Some((occurred_ms, source_time)) = parse_entry_ts(&entry) else {
                    diagnostics.push(diag(
                        "timestamp_unparseable",
                        Some("timestamp"),
                        raw.number,
                        "entry timestamp missing or unparseable; record skipped",
                    ));
                    continue;
                };
                let base = UsageEventBase {
                    key: entry_key("usage", &entry),
                    category: CallCategory::Auxiliary,
                    provider: json_str(&entry, "provider"),
                    model: json_str(&entry, "model"),
                    attribution: ModelAttribution::RequestField,
                    origin_call_id: None,
                    error_status: None,
                };
                events.push(build_usage_event(
                    target,
                    &context,
                    &base,
                    Some((&usage, &usage_json)),
                    occurred_ms,
                    &source_time,
                    raw.number,
                    now_ms,
                    None,
                    None,
                    parent_from_path.as_deref(),
                    &mut diagnostics,
                ));
            }
            "compaction" | "branch_summary" => {
                // 总结/分支总结调用（usage 可选）：无模型字段，按不晚于它的
                // model_change 归属。本机 37 条 compaction / 2 条 branch_summary
                // 均无 usage（tokensBefore/tokensAfter 是上下文估算，不计账）。
                let Some(usage_json) = entry.get("usage").cloned() else {
                    continue;
                };
                let Some(usage) = parse_usage(&usage_json) else {
                    diagnostics.push(diag(
                        "usage_shape_deviation",
                        Some("usage"),
                        raw.number,
                        "compaction/branch_summary usage missing required numeric fields; skipped",
                    ));
                    continue;
                };
                let Some((occurred_ms, source_time)) = parse_entry_ts(&entry) else {
                    diagnostics.push(diag(
                        "timestamp_unparseable",
                        Some("timestamp"),
                        raw.number,
                        "entry timestamp missing or unparseable; record skipped",
                    ));
                    continue;
                };
                let (attribution, model, provider) = match context.model.clone() {
                    Some(model) => (
                        ModelAttribution::StructuredChange,
                        Some(model),
                        context.model_provider.clone(),
                    ),
                    None => (ModelAttribution::Unknown, None, None),
                };
                let base = UsageEventBase {
                    key: entry_key(entry_type, &entry),
                    category: CallCategory::Auxiliary,
                    provider: provider.as_deref(),
                    model: model.as_deref(),
                    attribution,
                    origin_call_id: None,
                    error_status: None,
                };
                events.push(build_usage_event(
                    target,
                    &context,
                    &base,
                    Some((&usage, &usage_json)),
                    occurred_ms,
                    &source_time,
                    raw.number,
                    now_ms,
                    None,
                    None,
                    parent_from_path.as_deref(),
                    &mut diagnostics,
                ));
            }
            "title"
            | "title_change"
            | "credential_pin"
            | "session_init"
            | "ttsr_injection"
            | "service_tier_change"
            | "thinking_level_change"
            | "custom"
            | "label"
            | "session_info"
            | "custom_message"
            | "context_edit" => {}
            other => {
                if !context.unknown_types.iter().any(|t| t == other) {
                    context.unknown_types.push(other.to_string());
                    diagnostics.push(diag(
                        "unknown_record_type",
                        Some("type"),
                        raw.number,
                        "record type not mapped by this parser version; ignored",
                    ));
                }
            }
        }
    }
    let status = match &outcome.stop {
        StopReason::Eof => ScanStatus::Complete,
        StopReason::LineBudget | StopReason::TimeBudget => ScanStatus::BudgetExhausted,
        StopReason::LineTooLong { number, .. } => {
            diagnostics.push(diag(
                "line_too_long",
                None,
                *number,
                "line exceeds the 8 MiB limit; cursor held at line start for controlled retry",
            ));
            ScanStatus::LineTooLong
        }
    };
    let new_cursor = JsonlCursor {
        generation: target.generation,
        offset: outcome.next_offset,
        line_number: outcome.next_line_number,
    };
    let degraded = !outcome.bad_lines.is_empty()
        || diagnostics.iter().any(|d| {
            matches!(
                d.code.as_str(),
                "bad_json_line" | "usage_shape_deviation" | "line_too_long"
            )
        });
    Ok(ScanOutcome {
        status,
        cursor: Some(serde_json::to_value(new_cursor)?),
        parse_context: Some(serde_json::to_value(&context)?),
        events,
        aggregates: Vec::new(),
        diagnostics,
        lines_read: outcome.lines.len() as u64,
        records_seen,
        reconciliations: Vec::new(),
        health: if degraded {
            "degraded".to_string()
        } else {
            "active".to_string()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_ms_rounds_and_rejects_bad_values() {
        let v = serde_json::json!(17309.34289999993);
        assert_eq!(float_ms(Some(&v)), Some(Some(17309)));
        let half = serde_json::json!(1.5);
        assert_eq!(float_ms(Some(&half)), Some(Some(2)));
        let neg = serde_json::json!(-1.0);
        assert_eq!(float_ms(Some(&neg)), None);
        let text = serde_json::json!("abc");
        assert_eq!(float_ms(Some(&text)), None);
        assert_eq!(float_ms(None), Some(None));
    }

    #[test]
    fn subagent_parent_requires_session_dir_shape() {
        let main = Path::new(
            "/home/u/.omp/agent/sessions/--C--x--/2026-08-21T02-19-22-638Z_01a0221d-a28e-7000-a2a6-a32976b3696f.jsonl",
        );
        assert_eq!(subagent_parent_from_path(main), None);
        let sub = Path::new(
            "/home/u/.omp/agent/sessions/--C--x--/2026-08-21T02-19-22-638Z_01a0221d-a28e-7000-a2a6-a32976b3696f/CommunityResearch.jsonl",
        );
        assert_eq!(
            subagent_parent_from_path(sub),
            Some("01a0221d-a28e-7000-a2a6-a32976b3696f".to_string())
        );
        let nested = Path::new(
            "/home/u/.omp/agent/sessions/--C--x--/2026-09-12T05-01-52-715Z_01a093fe-50cb-72cd-97c8-20029d52ad34/PostgresStoreSlice/PostgresStoreSlice.AwaitifyStoreTests.jsonl",
        );
        assert_eq!(
            subagent_parent_from_path(nested),
            Some("01a093fe-50cb-72cd-97c8-20029d52ad34".to_string())
        );
        // 非 <ts>_<uuid> 形状的中间目录不判为子 Agent 父级。
        let stray = Path::new("/home/u/.omp/agent/sessions/--C--x--/random_dir/file.jsonl");
        assert_eq!(subagent_parent_from_path(stray), None);
        // 带下划线但非时间戳形状的目录名不误判。
        let not_ts = Path::new("/home/u/.omp/agent/sessions/--C--x--/Foo_12345678/file.jsonl");
        assert_eq!(subagent_parent_from_path(not_ts), None);
    }

    #[test]
    fn entry_key_uses_omp_namespace() {
        let entry = serde_json::json!({
            "type": "message",
            "id": "a1b2c3d4",
            "parentId": null,
            "timestamp": "2026-01-05T15:00:02.000Z"
        });
        assert_eq!(
            entry_key("message", &entry),
            "omp:message:a1b2c3d4:-:2026-01-05T15:00:02.000Z"
        );
    }

    #[test]
    fn old_parse_context_without_basis_still_restores() {
        // 旧解析上下文（无 version_basis 字段）反序列化不失败，basis 为 None；
        // V30 目录迁移不重建来源、不重置游标。
        let legacy = serde_json::json!({"session_id": null, "header_version": null});
        let ctx: OmpParseContext = serde_json::from_value(legacy).expect("restore");
        assert_eq!(ctx.version_basis, None);
    }
}
