//! Claude Code 适配器：本机会话 transcript JSONL（文档级证据，待真实样本）。
//!
//! 格式证据（官方文档，A01）：
//! - 路径：`$CLAUDE_CONFIG_DIR/projects/<project>/<session>.jsonl`（默认
//!   `~/.claude/projects/...`）；子 Agent transcript 在
//!   `projects/<project>/<session>/subagents/`；被替换的旧 transcript 以
//!   `<session>.orphaned-<ts>-<suffix>.jsonl` 与 `<session>.jsonl.superseded-<ts>`
//!   变体保留（内容与现行 transcript 重叠，按稳定身份 upsert 防双计）。
//! - monitoring-usage：usage 分类 input/output/cache_read/cache_creation；
//!   `requestId` 持久化在 assistant 条目上；"An API response is persisted as one
//!   transcript entry per content block"（同一响应多条目、usage 重复，按 requestId
//!   去重）；`query_source` ∈ {main, subagent, auxiliary}（OTel 侧，未接入）。
//! - 官方明示 "transcript entry format is internal ... not a stable contract"：
//!   条目形状按 Anthropic API usage 块口径（input_tokens/output_tokens/
//!   cache_read_input_tokens/cache_creation_input_tokens）实现，标注待真实样本。
//!
//! fail closed（V17）：未文档化记录 type、或非 usage 载体记录携带 usage 字段，
//! 整文件拒绝（游标不推进、下轮确定性再拒），不猜格式。

use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind, TimeBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::framework::{
    Availability, CapabilityTable, DetectOutcome, DiscoverContext, DiscoveredRoot, RootBasis,
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, SourceAdapter, StoredScanState,
};
use super::jsonl::{read_jsonl, JsonlCursor, StopReason};
use super::usage_map::{map_claude_transcript, ClaudeTranscriptUsage};

pub const CLAUDE_FORMAT: &str = "claude-transcript-jsonl";
/// 文档级格式版本（非 CLI 版本）：transcript 条目格式官方明示不稳定，
/// 本适配器按 A01 文档口径实现，待真实样本核验。
pub const CLAUDE_FORMAT_VERSION: &str = "transcript-doc-1";
pub const CLAUDE_PARSER_VERSION: &str = "claude-transcript-doc1";
pub const CLAUDE_ENV_HOME: &str = "CLAUDE_CONFIG_DIR";
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

/// Claude Code 适配器（无状态）。
pub struct ClaudeAdapter;

impl Default for ClaudeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeAdapter {
    pub fn new() -> Self {
        ClaudeAdapter
    }
}

/// 持久化解析上下文（跨增量轮次的"每文件一次性"诊断标志）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ClaudeParseContext {
    #[serde(default)]
    unmapped_usage_keys_reported: bool,
    #[serde(default)]
    assistant_without_usage_reported: bool,
}

/// 从存储的游标 JSON 还原；重扫或无效时回到文件头。
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

fn restore_context(stored: &StoredScanState, rescan: bool) -> ClaudeParseContext {
    if rescan {
        return ClaudeParseContext::default();
    }
    stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<ClaudeParseContext>(v.clone()).ok())
        .unwrap_or_default()
}

fn diag(code: &str, field: Option<&str>, line: u64, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: field.map(str::to_string),
        position: Some(format!("line {line}")),
        message: message.to_string(),
    }
}

fn json_str<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str()
}

fn parse_ts_ms(value: &serde_json::Value) -> Option<(i64, String)> {
    let raw = json_str(value, "timestamp")?;
    let ts = raw.parse::<jiff::Timestamp>().ok()?.as_millisecond();
    Some((ts, raw.to_string()))
}

/// 解析 assistant 条目 message.usage 四字段；全部必需、非负、有界。
/// 缺失/类型错误/负值/超限返回 None（调用方记诊断）；未知额外键另行诊断但保留记录。
fn parse_usage(value: &serde_json::Value) -> Option<ClaudeTranscriptUsage> {
    let obj = value.as_object()?;
    let get = |key: &str| -> Option<i64> {
        let v = obj.get(key)?.as_i64()?;
        if !(0..=MAX_REASONABLE_TOKEN).contains(&v) {
            return None;
        }
        Some(v)
    };
    Some(ClaudeTranscriptUsage {
        input_tokens: get("input_tokens")?,
        output_tokens: get("output_tokens")?,
        cache_read_input_tokens: get("cache_read_input_tokens")?,
        cache_creation_input_tokens: get("cache_creation_input_tokens")?,
    })
}

const USAGE_KEYS: &[&str] = &[
    "input_tokens",
    "output_tokens",
    "cache_read_input_tokens",
    "cache_creation_input_tokens",
];

fn has_unknown_usage_keys(value: &serde_json::Value) -> bool {
    value
        .as_object()
        .map(|obj| obj.keys().any(|k| !USAGE_KEYS.contains(&k.as_str())))
        .unwrap_or(false)
}

/// 文件是否位于 `.../<session>/subagents/` 下；是则返回父会话目录名。
fn subagent_parent_session(path: &Path) -> Option<String> {
    let parent = path.parent()?;
    if parent.file_name()?.to_str()? != "subagents" {
        return None;
    }
    Some(parent.parent()?.file_name()?.to_str()?.to_string())
}

impl SourceAdapter for ClaudeAdapter {
    fn adapter_id(&self) -> &'static str {
        "claude"
    }

    fn agent(&self) -> &'static str {
        "claude-code"
    }

    fn discover(&self, ctx: &DiscoverContext) -> Vec<DiscoveredRoot> {
        let mut roots: Vec<(PathBuf, RootBasis)> = Vec::new();
        if let Some(home) = ctx.env.get(CLAUDE_ENV_HOME) {
            roots.push((
                PathBuf::from(home),
                RootBasis::EnvOverride(CLAUDE_ENV_HOME.to_string()),
            ));
        }
        if let Some(home) = &ctx.home_dir {
            roots.push((home.join(".claude"), RootBasis::DefaultHome));
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out = Vec::new();
        for (root, basis) in roots {
            let projects = root.join("projects");
            if !projects.is_dir() {
                continue;
            }
            // projects/<project>/<session>.jsonl 深度 2；subagents/ 内 transcript 深度 3。
            let files = super::framework::enumerate_files_bounded(&projects, 3, &|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.ends_with(".jsonl") || n.contains(".jsonl.superseded-"))
                    .unwrap_or(false)
            });
            if !files.is_empty() {
                out.push(DiscoveredRoot { root, basis, files });
            }
        }
        out
    }

    fn instance_id(&self, root: &DiscoveredRoot) -> String {
        format!("claude@{}", super::framework::normalize_path(&root.root))
    }

    fn detect(&self, path: &Path) -> Result<DetectOutcome, CoreError> {
        let limits = super::jsonl::JsonlLimits {
            chunk_bytes: 64 * 1024,
            max_line_bytes: super::jsonl::DEFAULT_MAX_LINE_BYTES,
            max_lines: Some(1),
            time_budget: Some(std::time::Duration::from_secs(5)),
        };
        let outcome = read_jsonl(path, 0, 1, &limits)?;
        let Some(first) = outcome.lines.first() else {
            return Ok(DetectOutcome::Pending);
        };
        let Ok(line) = serde_json::from_str::<serde_json::Value>(&first.text) else {
            return Ok(DetectOutcome::UnknownFormat {
                reason: "first line is not JSON".to_string(),
            });
        };
        match line.get("type").and_then(|t| t.as_str()) {
            Some("user") | Some("assistant") | Some("system") => Ok(DetectOutcome::Supported {
                format: CLAUDE_FORMAT.to_string(),
                format_version: CLAUDE_FORMAT_VERSION.to_string(),
            }),
            other => Ok(DetectOutcome::UnknownFormat {
                reason: format!("first record type {other:?} not in documented set"),
            }),
        }
    }

    fn scan(
        &self,
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
        let mut fail_closed: Option<(u64, String, &'static str)> = None;
        let parent_from_path = subagent_parent_session(&target.path);
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
            let Ok(line) = serde_json::from_str::<serde_json::Value>(&raw.text) else {
                diagnostics.push(diag(
                    "bad_json_line",
                    None,
                    raw.number,
                    "line is not valid JSON; isolated, content not stored",
                ));
                continue;
            };
            let record_type = line.get("type").and_then(|t| t.as_str()).unwrap_or("");
            match record_type {
                "assistant" => {
                    let usage_value = line.get("message").and_then(|m| m.get("usage"));
                    let Some(usage_value) = usage_value else {
                        // assistant 条目无 usage：无用量证据，不产事件（每文件一次性诊断）。
                        if !context.assistant_without_usage_reported {
                            context.assistant_without_usage_reported = true;
                            diagnostics.push(diag(
                                "assistant_without_usage",
                                Some("message.usage"),
                                raw.number,
                                "assistant entry without message.usage; no usage evidence, no event",
                            ));
                        }
                        continue;
                    };
                    let Some(usage) = parse_usage(usage_value) else {
                        diagnostics.push(diag(
                            "usage_shape_deviation",
                            Some("message.usage"),
                            raw.number,
                            "assistant usage missing required numeric fields or out of range; record skipped",
                        ));
                        continue;
                    };
                    if !context.unmapped_usage_keys_reported && has_unknown_usage_keys(usage_value)
                    {
                        context.unmapped_usage_keys_reported = true;
                        diagnostics.push(diag(
                            "unmapped_usage_keys",
                            Some("message.usage"),
                            raw.number,
                            "usage object carries keys beyond the documented four; mapped fields kept",
                        ));
                    }
                    let Some((occurred_ms, source_time)) = parse_ts_ms(&line) else {
                        diagnostics.push(diag(
                            "timestamp_unparseable",
                            Some("timestamp"),
                            raw.number,
                            "entry timestamp missing or unparseable; record skipped",
                        ));
                        continue;
                    };
                    let session_id = json_str(&line, "sessionId").unwrap_or("unknown-session");
                    let request_id = json_str(&line, "requestId");
                    let message_id = line.get("message").and_then(|m| json_str(m, "id"));
                    let entry_uuid = json_str(&line, "uuid");
                    let (source_record_key, origin_call_id) = if let Some(rid) = request_id {
                        (format!("req:{rid}"), Some(rid.to_string()))
                    } else if let Some(mid) = message_id {
                        diagnostics.push(diag(
                            "missing_request_id",
                            Some("requestId"),
                            raw.number,
                            "assistant entry without requestId; fallback identity message.id",
                        ));
                        (format!("msg:{mid}"), Some(mid.to_string()))
                    } else if let Some(uuid) = entry_uuid {
                        diagnostics.push(diag(
                            "missing_request_id",
                            Some("requestId"),
                            raw.number,
                            "assistant entry without requestId/message.id; fallback identity entry uuid",
                        ));
                        (format!("uuid:{uuid}"), None)
                    } else {
                        diagnostics.push(diag(
                            "missing_request_id",
                            Some("requestId"),
                            raw.number,
                            "assistant entry without requestId/message.id/uuid; fallback identity session + line",
                        ));
                        (format!("seq:{session_id}:{}", raw.number), None)
                    };
                    let is_sidechain = line
                        .get("isSidechain")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let model = line
                        .get("message")
                        .and_then(|m| json_str(m, "model"))
                        .map(str::to_string);
                    let mapped = map_claude_transcript(&usage);
                    for contradiction in &mapped.diagnostics {
                        diagnostics.push(diag(
                            contradiction.code,
                            Some(contradiction.field),
                            raw.number,
                            &contradiction.detail,
                        ));
                    }
                    events.push(EventInput {
                        source_instance_id: target.instance_id.clone(),
                        source_record_key,
                        record_kind: RecordKind::ModelCall,
                        schema_version: CLAUDE_FORMAT_VERSION.to_string(),
                        parser_version: CLAUDE_PARSER_VERSION.to_string(),
                        origin_call_id,
                        attempt_id: None,
                        session_id: Some(session_id.to_string()),
                        parent_session_id: parent_from_path.clone(),
                        host_application: None,
                        agent: self.agent().to_string(),
                        call_category: if is_sidechain || parent_from_path.is_some() {
                            CallCategory::SubAgent
                        } else {
                            CallCategory::Primary
                        },
                        occurred_at_ms: occurred_ms,
                        observed_at_ms: Some(now_ms),
                        source_time: Some(source_time),
                        time_basis: TimeBasis::SourceCompletion,
                        interval_start_ms: None,
                        interval_end_ms: None,
                        provider_id: Some("anthropic".to_string()),
                        model_raw: model.clone(),
                        model_canonical: None,
                        model_attribution: if model.is_some() {
                            ModelAttribution::RequestField
                        } else {
                            ModelAttribution::Unknown
                        },
                        usage: mapped.usage,
                        quality: mapped.quality,
                        lifecycle: Lifecycle::Final,
                        source_revision: None,
                        error_status: None,
                        duration_ms: None,
                        ttft_ms: None,
                        attribution_status: AttributionStatus::Verified,
                        exclusion_reason: None,
                        cost: None,
                    });
                }
                "user" | "system" => {
                    // 非 usage 载体记录携带 usage 字段：格式偏离，整文件 fail closed。
                    let carries_usage = line.get("usage").is_some()
                        || line.get("message").and_then(|m| m.get("usage")).is_some();
                    if carries_usage {
                        fail_closed = Some((
                            raw.number,
                            format!("record type {record_type:?} carries a usage field"),
                            "usage_on_unexpected_record_type",
                        ));
                        break;
                    }
                }
                other => {
                    fail_closed = Some((
                        raw.number,
                        format!("record type {other:?} not in documented set"),
                        "undocumented_record_type",
                    ));
                    break;
                }
            }
        }
        let mut status = match &outcome.stop {
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
        let mut health_degraded = !outcome.bad_lines.is_empty()
            || diagnostics.iter().any(|d| {
                matches!(
                    d.code.as_str(),
                    "bad_json_line" | "usage_shape_deviation" | "line_too_long"
                )
            });
        // fail closed：本轮事件清空、游标不推进（不提交 checkpoint），下轮确定性再拒。
        let (cursor_out, context_out) = if let Some((line_no, detail, code)) = fail_closed {
            diagnostics.push(diag(code, Some("type"), line_no, &detail));
            events.clear();
            status = ScanStatus::Pending;
            health_degraded = true;
            (None, None)
        } else {
            (
                Some(serde_json::to_value(new_cursor)?),
                Some(serde_json::to_value(&context)?),
            )
        };
        Ok(ScanOutcome {
            status,
            cursor: cursor_out,
            parse_context: context_out,
            events,
            aggregates: Vec::new(),
            diagnostics,
            lines_read: outcome.lines.len() as u64,
            records_seen,
            reconciliations: Vec::new(),
            health: if health_degraded {
                "degraded".to_string()
            } else {
                "active".to_string()
            },
        })
    }

    fn capability(&self) -> CapabilityTable {
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, note: &str| {
            serde_json::json!({
                "availability": availability,
                "note": note,
            })
        };
        fields.insert(
            "tokens".into(),
            field(
                Availability::Partial(
                    "文档级证据，待真实样本：四字段互斥口径派生 input_total/total".into(),
                ),
                "message.usage 四字段 input/output/cache_read/cache_creation（全必填）",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Partial("文档级证据，待真实样本".into()),
                "cache_read_input_tokens reported",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial("文档级证据，待真实样本".into()),
                "cache_creation_input_tokens reported",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Available,
                "同一 API 响应按内容块持久化为多条目（官方文档）；按 requestId 去重后每条是一次调用",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial("文档级证据，待真实样本".into()),
                "message.model 记录自带字段（request_field）",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial("文档级证据，待真实样本".into()),
                "条目 ISO8601 时间戳，source_completion 口径",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Unavailable("本地无费用字段；远端账单/账号不接入".into()),
                "无",
            ),
        );
        fields.insert(
            "latency".into(),
            field(
                Availability::Unavailable(
                    "transcript 条目无逐次延迟字段（OTel api_response 有 duration_ms，未接入）"
                        .into(),
                ),
                "无",
            ),
        );
        CapabilityTable {
            adapter_id: "claude".to_string(),
            product: "Claude Code CLI".to_string(),
            surfaces: vec!["cli".into()],
            supported_versions: vec![CLAUDE_FORMAT_VERSION.to_string()],
            discovery: serde_json::json!({
                "default_roots": ["$CLAUDE_CONFIG_DIR", "<home>/.claude"],
                "env_override": CLAUDE_ENV_HOME,
                "manual_roots": true,
                "bounded": true,
                "pattern": "projects/<project>/*.jsonl（含 orphaned/superseded 变体）与 projects/<project>/<session>/subagents/*.jsonl",
                "profile": "无 profile 概念",
            }),
            detection: serde_json::json!({
                "magic": "首行 JSONL type ∈ {user, assistant, system}",
                "version_field": "无可靠版本字段；格式版本为文档级 transcript-doc-1",
                "fail_closed": true,
                "unknown_version": "未文档化记录 type 或载体外 usage 字段：整文件拒绝，不猜格式",
            }),
            fields,
            lifecycle: serde_json::json!({
                "model_call": "assistant 条目 message.usage（final，requestId 身份）",
                "one_response_many_entries": "一个 API 响应按内容块持久化为多条目（官方文档）；usage 重复，按 requestId upsert 去重",
                "cumulative_snapshot": "transcript 无累计快照；goal/账单侧写不接入",
                "aborted": "格式内无证据；未观测",
                "retries": "格式内未观测到 transport 重试记录",
                "subagent": "isSidechain=true 或 subagents/ 路径 ⇒ sub_agent；子 Agent transcript 是独立文件，跨文件同 requestId 靠 upsert 防双计",
            }),
            incremental: serde_json::json!({
                "cursor": "文件身份 + generation + 完整行字节偏移 + 解析上下文",
                "rewrite_detection": ["截断", "同长替换", "改名重探测", "重建（创建时间变化）"],
                "budget": "单源每轮 30s 初值；单行 8 MiB；单块 4 MiB",
                "half_line": "半行不前移游标",
            }),
            dedup: serde_json::json!({
                "primary": "req:{requestId}（实例命名空间）",
                "fallback": "msg:{message.id} → uuid:{条目 uuid} → seq:{sessionId}:{行号}（逐级记诊断）",
                "orphaned_superseded": "旧 transcript 变体与现行文件内容重叠；稳定身份 upsert 防双计",
                "cross_source": "OTel query_source 汇总（main/subagent/auxiliary）未接入，不与 transcript 相加",
            }),
            integrity: serde_json::json!({
                "success_only": "格式内无失败调用证据；只统计已持久化的 assistant usage 条目",
                "hidden_calls": "auxiliary（compact 等）请求若写入 transcript 则按同口径计入；OTel 侧未接入",
                "sampling": "未观测到采样；坏行逐条隔离记诊断",
                "source_retention": "cleanupPeriodDays 清扫（默认 30 天）；可回填范围以现存文件为准",
                "prompt_content": "只读白名单字段（type/timestamp/sessionId/requestId/uuid/isSidechain/message.{id,model,usage}），正文不提取",
            }),
            maintenance: serde_json::json!({
                "parser_version": CLAUDE_PARSER_VERSION,
                "format_evidence": "官方文档 A01（transcript 路径与变体、requestId、一条响应多条目、usage 四分类）；条目形状为 Anthropic API usage 口径，待真实样本",
                "upgrade_policy": "transcript 条目格式官方明示不稳定；偏离（未知 type、载体外 usage）fail closed，取得真实样本后扩展",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan；手动/间隔/监听触发按源合并",
                "incremental_cost": "字节偏移续读；无变化文件探测短路",
                "pause_cancel": "文件间可停；单轮预算有界",
            }),
            limitations: vec![
                "全部字段口径为文档级证据（A01），本机无真实样本（not_found）；首份真实 fixture 到达后逐字段核验".into(),
                "transcript 条目格式官方明示非稳定合同，任何偏离 fail closed 而非猜测".into(),
                "usage 四字段缺一不可（缺失是未知不补零）；input_total/total 由互斥拆分派生".into(),
                "OTel telemetry（query_source 分类、duration_ms）不接入，不与 transcript 相加".into(),
                "无 requestId/message.id/uuid 记录用 sessionId+行号身份，文件同位替换后可能形成新键".into(),
                "符号链接/junction 不跟随；Windows 无稳定文件索引号，身份靠创建时间+首采样".into(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_usage_requires_four_fields() {
        let v = serde_json::json!({
            "input_tokens": 10,
            "output_tokens": 2,
            "cache_read_input_tokens": 4,
            "cache_creation_input_tokens": 3
        });
        let usage = parse_usage(&v).unwrap();
        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.cache_creation_input_tokens, 3);
        assert!(parse_usage(&serde_json::json!({"input_tokens": 10})).is_none());
        assert!(parse_usage(&serde_json::json!({
            "input_tokens": -1,
            "output_tokens": 0,
            "cache_read_input_tokens": 0,
            "cache_creation_input_tokens": 0
        }))
        .is_none());
    }

    #[test]
    fn unknown_usage_keys_detected() {
        let v = serde_json::json!({
            "input_tokens": 1,
            "output_tokens": 1,
            "cache_read_input_tokens": 0,
            "cache_creation_input_tokens": 0,
            "service_tier": "standard"
        });
        assert!(has_unknown_usage_keys(&v));
        assert!(parse_usage(&v).is_some(), "mapped fields kept");
        let clean = serde_json::json!({
            "input_tokens": 1,
            "output_tokens": 1,
            "cache_read_input_tokens": 0,
            "cache_creation_input_tokens": 0
        });
        assert!(!has_unknown_usage_keys(&clean));
    }

    #[test]
    fn subagent_parent_from_path() {
        let p = Path::new("/home/u/.claude/projects/proj/sess-1/subagents/agent-a.jsonl");
        assert_eq!(subagent_parent_session(p), Some("sess-1".to_string()));
        let main = Path::new("/home/u/.claude/projects/proj/sess-1.jsonl");
        assert_eq!(subagent_parent_session(main), None);
    }
}
