//! Gemini CLI 适配器：本机会话整写 JSON（文档级证据，待真实样本）。
//!
//! 格式证据（官方文档，A10）：
//! - 路径：`~/.gemini/tmp/<project_hash>/chats/session-<date>T<time>-<hash>.json`
//!   （单文件 JSON，非 JSONL；无文档化环境覆盖）。
//! - 顶层 `{sessionId, projectHash, startTime, lastUpdated, messages[]}`；
//!   "Token usage statistics (input, output, cached, etc.)"。
//! - telemetry `gemini_cli.token.usage` type ∈ {input, output, thought, cache, tool}；
//!   `api_response` 六字段 input/output/cached_content/thoughts/tool/total
//!   _token_count + model + duration_ms（telemetry 侧，未接入）。
//! - 消息 token 形状按文档分类实现：`tokens{input, output, cached, thoughts, tool,
//!   total}` 各自可选；thoughts/cached/tool 与 input/output 的包含关系未核验
//!   （未知不猜：total 只取直报，thoughts/tool 不并入任何字段）。
//!
//! 增量语义（整写 JSON）：全量有界读取（32 MiB 初值）；游标存已消费字节数复用
//! 框架无变化短路；改写/截断走 generation 重扫，事件按稳定身份 upsert 幂等；
//! 半程写入（parse 失败）不推进游标，下轮确定性重试。
//!
//! fail closed（V17）：未文档化消息 type、或 user 消息携带 tokens，整文件拒绝
//! （游标不推进、下轮确定性再拒），不猜格式。

use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind, TimeBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use std::io::Read;
use std::path::{Path, PathBuf};

use super::framework::{
    Availability, CapabilityTable, DetectOutcome, DiscoverContext, DiscoveredRoot, RootBasis,
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, SourceAdapter, StoredScanState,
};
use super::usage_map::{map_genai_usage, GenaiUsage};

pub const GEMINI_FORMAT: &str = "gemini-session-json";
/// 文档级格式版本（非 CLI 版本）：会话 JSON 形状按 A10 文档口径实现，待真实样本。
pub const GEMINI_FORMAT_VERSION: &str = "session-doc-1";
pub const GEMINI_PARSER_VERSION: &str = "gemini-session-doc1";
/// 单文件有界读取上限（初值 32 MiB）。
pub const GEMINI_MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;
const DETECT_HEAD_BYTES: usize = 64 * 1024;
const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";

/// Gemini CLI 适配器（无状态）。
pub struct GeminiAdapter;

impl Default for GeminiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl GeminiAdapter {
    pub fn new() -> Self {
        GeminiAdapter
    }
}

/// 文档化消息类型（info/error/warning 等真实存在但未文档化 ⇒ fail closed 待扩）。
const MESSAGE_TYPES: &[&str] = &["user", "gemini"];

/// 消息 tokens 六键（各自可选；包含关系未核验，total 只取直报）。
const TOKEN_KEYS: &[&str] = &["input", "output", "cached", "thoughts", "tool", "total"];

fn diag(code: &str, field: Option<&str>, position: &str, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: field.map(str::to_string),
        position: Some(position.to_string()),
        message: message.to_string(),
    }
}

fn json_str<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str()
}

/// 解析消息 tokens 六可选字段；存在的值必须非负有界（违例 None，调用方记诊断）。
/// 未知额外键返回 true（保留已映射字段，每轮一次性诊断）。
fn parse_tokens(value: &serde_json::Value) -> Option<(GenaiUsage, bool)> {
    let obj = value.as_object()?;
    let get = |key: &str| -> Option<Option<i64>> {
        match obj.get(key) {
            None => Some(None),
            Some(v) => {
                let n = v.as_i64()?;
                if !(0..=MAX_REASONABLE_TOKEN).contains(&n) {
                    return None;
                }
                Some(Some(n))
            }
        }
    };
    let usage = GenaiUsage {
        prompt_tokens: get("input")?,
        candidates_tokens: get("output")?,
        cached_tokens: get("cached")?,
        thoughts_tokens: get("thoughts")?,
        tool_tokens: get("tool")?,
        total_tokens: get("total")?,
    };
    let unknown = obj.keys().any(|k| !TOKEN_KEYS.contains(&k.as_str()));
    Some((usage, unknown))
}

fn strip_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(UTF8_BOM).unwrap_or(bytes)
}

/// 整写 JSON 游标：复用框架 JsonlCursor 形状（offset=已消费字节数，line_number 恒 1），
/// 无变化短路依赖 probe.len == cursor.offset。
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct WholeFileCursor {
    generation: i64,
    offset: u64,
    line_number: u64,
}

impl SourceAdapter for GeminiAdapter {
    fn adapter_id(&self) -> &'static str {
        "gemini"
    }

    fn agent(&self) -> &'static str {
        "gemini-cli"
    }

    fn discover(&self, ctx: &DiscoverContext) -> Vec<DiscoveredRoot> {
        let mut roots: Vec<(PathBuf, RootBasis)> = Vec::new();
        if let Some(home) = &ctx.home_dir {
            roots.push((home.join(".gemini"), RootBasis::DefaultHome));
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out = Vec::new();
        for (root, basis) in roots {
            let tmp = root.join("tmp");
            if !tmp.is_dir() {
                continue;
            }
            // tmp/<project_hash>/chats/session-*.json：深度 2，有界枚举。
            let files = super::framework::enumerate_files_bounded(&tmp, 2, &|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.ends_with(".json"))
                    .unwrap_or(false)
            });
            if !files.is_empty() {
                out.push(DiscoveredRoot { root, basis, files });
            }
        }
        out
    }

    fn instance_id(&self, root: &DiscoveredRoot) -> String {
        format!("gemini@{}", super::framework::normalize_path(&root.root))
    }

    fn detect(&self, path: &Path) -> Result<DetectOutcome, CoreError> {
        let mut file = std::fs::File::open(path)?;
        let mut head = vec![0u8; DETECT_HEAD_BYTES];
        let n = file.read(&mut head)?;
        head.truncate(n);
        let text = String::from_utf8_lossy(strip_bom(&head));
        let trimmed = text.trim_start();
        if trimmed.is_empty() {
            return Ok(DetectOutcome::Pending);
        }
        if !trimmed.starts_with('{') {
            return Ok(DetectOutcome::UnknownFormat {
                reason: "session file does not start with a JSON object".to_string(),
            });
        }
        let has_session_id = trimmed.contains("\"sessionId\"");
        let has_messages = trimmed.contains("\"messages\"");
        if has_session_id && has_messages {
            Ok(DetectOutcome::Supported {
                format: GEMINI_FORMAT.to_string(),
                format_version: GEMINI_FORMAT_VERSION.to_string(),
            })
        } else if has_session_id {
            // 只有 sessionId：可能仍在首次写入中，下轮重探。
            Ok(DetectOutcome::Pending)
        } else {
            Ok(DetectOutcome::UnknownFormat {
                reason: "missing sessionId/messages fingerprint".to_string(),
            })
        }
    }

    fn scan(
        &self,
        target: &ScanTarget,
        _stored: &StoredScanState,
        _limits: &ScanLimits,
        now_ms: i64,
    ) -> Result<ScanOutcome, CoreError> {
        let mut events: Vec<EventInput> = Vec::new();
        let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
        let cursor_at = |offset: u64| -> Result<serde_json::Value, CoreError> {
            Ok(serde_json::to_value(WholeFileCursor {
                generation: target.generation,
                offset,
                line_number: 1,
            })?)
        };
        // 超限：受限，游标停在起点，受控重试（不静默丢弃）。
        if target.probe.len > GEMINI_MAX_FILE_BYTES {
            diagnostics.push(diag(
                "file_exceeds_size_cap",
                None,
                "document",
                "session JSON exceeds the 32 MiB cap; cursor held at start for controlled retry",
            ));
            return Ok(ScanOutcome {
                status: ScanStatus::LineTooLong,
                cursor: Some(cursor_at(0)?),
                parse_context: None,
                events,
                aggregates: Vec::new(),
                diagnostics,
                lines_read: 0,
                records_seen: 0,
                reconciliations: Vec::new(),
                health: "degraded".to_string(),
            });
        }
        let mut bytes = Vec::new();
        std::fs::File::open(&target.path)?
            .take(GEMINI_MAX_FILE_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > GEMINI_MAX_FILE_BYTES {
            diagnostics.push(diag(
                "file_exceeds_size_cap",
                None,
                "document",
                "session JSON grew past the 32 MiB cap during read; cursor held at start",
            ));
            return Ok(ScanOutcome {
                status: ScanStatus::LineTooLong,
                cursor: Some(cursor_at(0)?),
                parse_context: None,
                events,
                aggregates: Vec::new(),
                diagnostics,
                lines_read: 0,
                records_seen: 0,
                reconciliations: Vec::new(),
                health: "degraded".to_string(),
            });
        }
        let consumed = bytes.len() as u64;
        // 半程写入：parse 失败不推进游标，下轮确定性重试（暂态，非降级）。
        let document: serde_json::Value = match serde_json::from_slice(strip_bom(&bytes)) {
            Ok(v) => v,
            Err(_) => {
                diagnostics.push(diag(
                    "session_json_unparseable",
                    None,
                    "document",
                    "session JSON does not parse (mid-write or corrupt); cursor held for retry",
                ));
                return Ok(ScanOutcome {
                    status: ScanStatus::Pending,
                    cursor: None,
                    parse_context: None,
                    events,
                    aggregates: Vec::new(),
                    diagnostics,
                    lines_read: 1,
                    records_seen: 0,
                    reconciliations: Vec::new(),
                    health: "active".to_string(),
                });
            }
        };
        // fail closed 出口：事件清空、游标不推进、degraded，下轮确定性再拒。
        let fail_closed = |events: &mut Vec<EventInput>,
                         diagnostics: &mut Vec<DiagnosticInput>,
                         records_seen: u64,
                         code: &'static str,
                         detail: String|
         -> Result<ScanOutcome, CoreError> {
            events.clear();
            diagnostics.push(diag(code, None, "document", &detail));
            Ok(ScanOutcome {
                status: ScanStatus::Pending,
                cursor: None,
                parse_context: None,
                events: Vec::new(),
                aggregates: Vec::new(),
                diagnostics: std::mem::take(diagnostics),
                lines_read: 1,
                records_seen,
                reconciliations: Vec::new(),
                health: "degraded".to_string(),
            })
        };
        let Some(session_id) = json_str(&document, "sessionId") else {
            return fail_closed(
                &mut events,
                &mut diagnostics,
                0,
                "session_schema_deviation",
                "top-level sessionId missing or not a string".to_string(),
            );
        };
        let Some(messages) = document.get("messages").and_then(|m| m.as_array()) else {
            return fail_closed(
                &mut events,
                &mut diagnostics,
                0,
                "session_schema_deviation",
                "top-level messages missing or not an array".to_string(),
            );
        };
        let mut records_seen: u64 = 0;
        let mut unmapped_keys_reported = false;
        for (index, message) in messages.iter().enumerate() {
            records_seen += 1;
            let position = format!("messages[{index}]");
            let Some(message_obj) = message.as_object() else {
                return fail_closed(
                    &mut events,
                    &mut diagnostics,
                    records_seen,
                    "session_schema_deviation",
                    format!("{position} is not an object"),
                );
            };
            let message_type = message_obj
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("");
            if !MESSAGE_TYPES.contains(&message_type) {
                return fail_closed(
                    &mut events,
                    &mut diagnostics,
                    records_seen,
                    "undocumented_message_type",
                    format!("{position} type {message_type:?} not in documented set"),
                );
            }
            let tokens_value = message_obj.get("tokens");
            if message_type == "user" {
                if tokens_value.is_some() {
                    return fail_closed(
                        &mut events,
                        &mut diagnostics,
                        records_seen,
                        "usage_on_unexpected_message_type",
                        format!("{position} user message carries a tokens object"),
                    );
                }
                continue;
            }
            let Some(tokens_value) = tokens_value else {
                // gemini 消息无 tokens：无用量证据，不产事件。
                continue;
            };
            let Some((usage, unknown_keys)) = parse_tokens(tokens_value) else {
                diagnostics.push(diag(
                    "usage_shape_deviation",
                    Some("tokens"),
                    &position,
                    "tokens value negative, non-integer or out of range; message skipped",
                ));
                continue;
            };
            if unknown_keys && !unmapped_keys_reported {
                unmapped_keys_reported = true;
                diagnostics.push(diag(
                    "unmapped_usage_keys",
                    Some("tokens"),
                    &position,
                    "tokens object carries keys beyond the documented six; mapped fields kept",
                ));
            }
            let Some((occurred_ms, source_time)) = json_str(message, "timestamp").and_then(
                |raw_ts| {
                    raw_ts
                        .parse::<jiff::Timestamp>()
                        .ok()
                        .map(|t| (t.as_millisecond(), raw_ts.to_string()))
                },
            ) else {
                diagnostics.push(diag(
                    "timestamp_unparseable",
                    Some("timestamp"),
                    &position,
                    "message timestamp missing or unparseable; message skipped",
                ));
                continue;
            };
            let (source_record_key, origin_call_id) = match json_str(message, "id") {
                Some(id) => (
                    format!("gemini:{session_id}:{id}"),
                    Some(id.to_string()),
                ),
                None => {
                    diagnostics.push(diag(
                        "missing_message_id",
                        Some("id"),
                        &position,
                        "gemini message without id; fallback identity array index (append-only 假设)",
                    ));
                    (format!("gemini:{session_id}:idx-{index}"), None)
                }
            };
            let model = json_str(message, "model").map(str::to_string);
            let mapped = map_genai_usage(&usage);
            for contradiction in &mapped.diagnostics {
                diagnostics.push(diag(
                    contradiction.code,
                    Some(contradiction.field),
                    &position,
                    &contradiction.detail,
                ));
            }
            events.push(EventInput {
                source_instance_id: target.instance_id.clone(),
                source_record_key,
                record_kind: RecordKind::ModelCall,
                schema_version: GEMINI_FORMAT_VERSION.to_string(),
                parser_version: GEMINI_PARSER_VERSION.to_string(),
                origin_call_id,
                attempt_id: None,
                session_id: Some(session_id.to_string()),
                parent_session_id: None,
                host_application: None,
                agent: self.agent().to_string(),
                call_category: CallCategory::Primary,
                occurred_at_ms: occurred_ms,
                observed_at_ms: Some(now_ms),
                source_time: Some(source_time),
                time_basis: TimeBasis::SourceCompletion,
                interval_start_ms: None,
                interval_end_ms: None,
                provider_id: Some("google".to_string()),
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
        let health_degraded = diagnostics
            .iter()
            .any(|d| d.code == "usage_shape_deviation");
        Ok(ScanOutcome {
            status: ScanStatus::Complete,
            cursor: Some(cursor_at(consumed)?),
            parse_context: None,
            events,
            aggregates: Vec::new(),
            diagnostics,
            lines_read: 1,
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
                    "文档级证据，待真实样本：六键各自可选；thoughts/cached/tool 包含关系未核验，total 只取直报不派生".into(),
                ),
                "messages[].tokens: input/output/cached/thoughts/tool/total",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Partial("文档级证据，待真实样本".into()),
                "tokens.cached reported（可选字段）",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Unavailable("格式内无缓存创建字段".into()),
                "无",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Partial("文档级证据，待真实样本".into()),
                "每条带 tokens 的 gemini 消息按一次模型调用计（message.id 身份）",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial("文档级证据，待真实样本".into()),
                "消息自带 model 字段（request_field）；缺失 unknown",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial("文档级证据，待真实样本".into()),
                "消息 ISO8601 timestamp，source_completion 口径",
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
                Availability::Unavailable("会话 JSON 无逐次延迟字段（telemetry api_response 有 duration_ms，未接入）".into()),
                "无",
            ),
        );
        CapabilityTable {
            adapter_id: "gemini".to_string(),
            product: "Gemini CLI".to_string(),
            surfaces: vec!["cli".into()],
            supported_versions: vec![GEMINI_FORMAT_VERSION.to_string()],
            discovery: serde_json::json!({
                "default_roots": ["<home>/.gemini"],
                "env_override": null,
                "manual_roots": true,
                "bounded": true,
                "pattern": "tmp/<project_hash>/chats/session-*.json",
                "profile": "无 profile 概念",
            }),
            detection: serde_json::json!({
                "magic": "文件头 64 KiB 指纹：JSON object 且含 sessionId+messages 键（剥 UTF-8 BOM）",
                "version_field": "无版本字段；格式版本为文档级 session-doc-1",
                "fail_closed": true,
                "unknown_version": "未文档化消息 type 或载体外 tokens：整文件拒绝，不猜格式",
            }),
            fields,
            lifecycle: serde_json::json!({
                "model_call": "gemini 消息 tokens（final，message.id 身份）",
                "cumulative_snapshot": "会话 JSON 无累计快照；checkpoint/账单侧写不接入",
                "aborted": "格式内无证据；未观测",
                "retries": "格式内未观测到 transport 重试记录",
                "subagent": "文档化形状无子 Agent 概念；info/error/warning 消息真实存在但未文档化，fail closed 待扩",
            }),
            incremental: serde_json::json!({
                "cursor": "文件身份 + generation + 已消费字节数（整写 JSON 全量重读，事件 upsert 幂等）",
                "rewrite_detection": ["截断", "同长替换", "改名重探测", "重建（创建时间变化）", "首采样变化（前缀改写）"],
                "budget": "单文件 32 MiB 有界读取；超限受限受控重试",
                "mid_write": "半程写入 parse 失败不推进游标，下轮确定性重试",
            }),
            dedup: serde_json::json!({
                "primary": "gemini:{sessionId}:{message.id}（实例命名空间）",
                "fallback": "gemini:{sessionId}:idx-{数组下标}（缺 id，记诊断；依赖 append-only）",
                "cross_source": "telemetry（gemini_cli.token.usage/api_response）未接入，不与会话 JSON 相加",
            }),
            integrity: serde_json::json!({
                "success_only": "格式内无失败调用证据；只统计带 tokens 的 gemini 消息",
                "hidden_calls": "未观测",
                "sampling": "未观测到采样",
                "source_retention": "源端保留未知；可回填范围以现存文件为准",
                "prompt_content": "只读白名单字段（sessionId/messages[].{id,type,model,timestamp,tokens}），正文不提取",
            }),
            maintenance: serde_json::json!({
                "parser_version": GEMINI_PARSER_VERSION,
                "format_evidence": "官方文档 A10（会话路径与顶层形状、token 分类、telemetry 字段）；消息 tokens 形状按文档分类实现，待真实样本",
                "upgrade_policy": "未文档化消息 type/tokens 形状偏离 fail closed，取得真实样本后扩展",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan；手动/间隔/监听触发按源合并",
                "incremental_cost": "无变化探测短路；有变化全量重读（32 MiB 有界）+ upsert 幂等",
                "pause_cancel": "文件间可停；单文件读取有界",
            }),
            limitations: vec![
                "全部字段口径为文档级证据（A10），本机无真实样本（not_found）；首份真实 fixture 到达后逐字段核验".into(),
                "tokens 六键包含关系（cached/thoughts/tool 是否子集）未核验：total 只取直报，thoughts/tool 不并入任何字段".into(),
                "info/error/warning 消息类型真实存在但未文档化：fail closed，待真实样本后扩展接受集".into(),
                "整写 JSON 每次变化全量重读（32 MiB 有界），成本随文件大小线性；靠 upsert 幂等保证不双计".into(),
                "缺 message.id 记录用数组下标身份，历史非 append-only 时可能串号".into(),
                "符号链接/junction 不跟随；Windows 无稳定文件索引号，身份靠创建时间+首采样".into(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tokens_optional_fields() {
        let full = serde_json::json!({
            "input": 100,
            "output": 20,
            "cached": 30,
            "thoughts": 5,
            "tool": 7,
            "total": 120
        });
        let (usage, unknown) = parse_tokens(&full).unwrap();
        assert_eq!(usage.prompt_tokens, Some(100));
        assert_eq!(usage.thoughts_tokens, Some(5));
        assert!(!unknown);

        let partial = serde_json::json!({"input": 10});
        let (usage, unknown) = parse_tokens(&partial).unwrap();
        assert_eq!(usage.prompt_tokens, Some(10));
        assert_eq!(usage.total_tokens, None);
        assert!(!unknown);

        let negative = serde_json::json!({"input": -1});
        assert!(parse_tokens(&negative).is_none());

        let extra = serde_json::json!({"input": 1, "surprise": 2});
        let (usage, unknown) = parse_tokens(&extra).unwrap();
        assert_eq!(usage.prompt_tokens, Some(1));
        assert!(unknown);
    }

    #[test]
    fn bom_stripped() {
        let with_bom = b"\xEF\xBB\xBF{\"sessionId\": 1}";
        assert_eq!(strip_bom(with_bom), b"{\"sessionId\": 1}");
        let without = b"{}";
        assert_eq!(strip_bom(without), b"{}");
    }
}
