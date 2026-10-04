//! jcode 会话存储格式实现（`session_v1`，jcode-session-1）。
//!
//! 格式依据（1jehuang/jcode 固定源码 4f6bf8e044bc175d5d4659ed54583f5512320235
//! （master，2026-09-29）；官方源码核验；本机未安装、无真实样本）：
//! - 路径：`$JCODE_HOME`（默认 ~/.jcode）/sessions/session_*.json 快照 +
//!   同 stem `.journal.jsonl` 追加（storage_paths.rs:7-30）。
//! - 加载语义（persistence.rs:269-341）：快照 + 逐条 journal 回放；
//!   journal `meta` **逐字段覆盖**快照元数据（provider_key/model 以最后一条
//!   journal 为准）；`append_messages` 追加。checkpoint 时整写快照并**删除**
//!   journal（512 KiB 上限，session.rs:264）⇒ 两载体消息互斥；崩溃窗口
//!   （写快照后删 journal 前）重放会重复 extend，源码无 id 去重 ⇒
//!   适配器按消息 id upsert 保持幂等（本方防御，如实标注）。
//! - StoredMessage（session-types lib.rs:229-272）：`id`/`role`/`timestamp`
//!   （RFC3339，消息完成时刻）/`tool_duration_ms?`/
//!   `token_usage?{input_tokens(必),output_tokens(必),
//!   cache_read_input_tokens?,cache_creation_input_tokens?}`；
//!   `prompt_tokens` 是上下文规模非计费桶，**不采**；无 cost/reasoning 落盘。
//! - **缓存字段语义按 provider 原样保留**（官方注释，OpenAI stream.rs:988-1018）：
//!   provider_key=="openai" ⇒ input_tokens 是总量，cache 读/写是其**子集**；
//!   =="anthropic" ⇒ 三列分立互斥（API 原生）；其他/未知 ⇒ 包含关系未知，
//!   hermes 同型并列不派生。`ResponseStats` 官方注释 "Missing telemetry is
//!   unknown, not zero" 规则相同。
//! - 回合计数库 model-usage-v1.sqlite3 只记计数不记 token，不读。

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::usage_map::{finish, sub_checked, MappedUsage};
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use std::io::Read as _;

use super::JCODE_FORMAT_VERSION;

pub const JCODE_PARSER_VERSION: &str = "jcode-session-1";
pub const JCODE_MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

/// journal 分页时保存位置及源文件指纹；读到末页后从头重读以处理
/// 会话级 meta 更新。事件按消息 id upsert 幂等。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct JcodeCursor {
    generation: i64,
    snapshot_len: u64,
    journal_offset: u64,
    #[serde(default = "first_journal_line")]
    journal_line: u64,
    #[serde(default)]
    journal_probe: Option<JournalProbe>,
}

fn first_journal_line() -> u64 {
    1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct JournalProbe {
    len: u64,
    created_ms: Option<i64>,
    head_hash: u64,
    head_len: u64,
    tail_hash: u64,
}

impl From<&crate::adapters::jsonl::FileProbe> for JournalProbe {
    fn from(p: &crate::adapters::jsonl::FileProbe) -> Self {
        Self {
            len: p.len,
            created_ms: p.created_ms,
            head_hash: p.head_hash,
            head_len: p.head_len,
            tail_hash: p.tail_hash,
        }
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct JcodeParseContext {
    #[serde(default)]
    version_basis: Option<VersionBasis>,
    #[serde(default)]
    journal_provider_key: Option<String>,
    #[serde(default)]
    journal_model: Option<String>,
    #[serde(default)]
    journal_updated_at_ms: Option<i64>,
}

fn diag(code: &str, position: &str, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: None,
        position: Some(position.to_string()),
        message: message.to_string(),
    }
}

fn rfc3339_ms(value: Option<&serde_json::Value>) -> Option<i64> {
    let s: &str = value?.as_str()?;
    let ts: jiff::Timestamp = s.trim().parse().ok()?;
    let ms = ts.as_millisecond();
    (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
        .contains(&ms)
        .then_some(ms)
}

fn opt_token(value: Option<&serde_json::Value>) -> Option<Option<i64>> {
    match value {
        None => Some(None),
        Some(v) => {
            let n = v.as_i64()?;
            Some((0..=MAX_REASONABLE_TOKEN).contains(&n).then_some(n))
        }
    }
}

/// jcode 缓存字段语义（provider 原样保留）：
/// - openai：input 是总量，cache 读/写为子集 ⇒ uncached 减法派生；
/// - anthropic：三列互斥 ⇒ uncached=input，input_total=三列之和（派生）；
/// - 其他/未知：包含关系未知 ⇒ 并列报告不派生。
fn map_jcode(
    provider_key: Option<&str>,
    input: Option<i64>,
    output: Option<i64>,
    cache_read: Option<i64>,
    cache_write: Option<i64>,
) -> MappedUsage {
    let mut contradictions = Vec::new();
    let (input_uncached, input_total, uncached_quality) = match provider_key {
        Some("openai") => {
            let uncached = match (input, cache_read, cache_write) {
                (Some(total), Some(r), Some(w)) => sub_checked(
                    "input_uncached",
                    total,
                    r.saturating_add(w),
                    &mut contradictions,
                ),
                _ => None,
            };
            (uncached, input, crate::domain::FieldQuality::Derived)
        }
        Some("anthropic") => {
            let total = input
                .zip(cache_read)
                .and_then(|(a, b)| a.checked_add(b))
                .and_then(|v| cache_write.and_then(|w| v.checked_add(w)));
            (input, total, crate::domain::FieldQuality::Reported)
        }
        _ => (None, input, crate::domain::FieldQuality::Unknown),
    };
    let usage = crate::domain::TokenUsage {
        input_uncached,
        input_cache_read: cache_read,
        input_cache_write: cache_write,
        input_total,
        output_total: output,
        output_reasoning: None,
        total_tokens: None,
        source_total: None,
    };
    let quality = crate::domain::TokenQuality {
        input_uncached: if input_uncached.is_some() {
            uncached_quality
        } else {
            crate::domain::FieldQuality::Unknown
        },
        input_cache_read: cache_read
            .map(|_| crate::domain::FieldQuality::Reported)
            .unwrap_or(crate::domain::FieldQuality::Unknown),
        input_cache_write: cache_write
            .map(|_| crate::domain::FieldQuality::Reported)
            .unwrap_or(crate::domain::FieldQuality::Unknown),
        input_total: input_total
            .map(|_| crate::domain::FieldQuality::Reported)
            .unwrap_or(crate::domain::FieldQuality::Unknown),
        output_total: output
            .map(|_| crate::domain::FieldQuality::Reported)
            .unwrap_or(crate::domain::FieldQuality::Unknown),
        ..Default::default()
    };
    finish(usage, quality, contradictions)
}

/// 一条合并后的消息（快照或 journal append）。
struct MergedMessage {
    id: Option<String>,
    role: String,
    timestamp: Option<i64>,
    tool_duration_ms: Option<i64>,
    usage: Option<FiveOptional>,
}

struct FiveOptional {
    input: Option<i64>,
    output: Option<i64>,
    cache_read: Option<i64>,
    cache_write: Option<i64>,
}

fn parse_message(value: &serde_json::Value) -> Option<MergedMessage> {
    let obj = value.as_object()?;
    let usage = obj
        .get("token_usage")
        .and_then(|v| v.as_object())
        .map(|u| FiveOptional {
            input: opt_token(u.get("input_tokens")).unwrap_or(None),
            output: opt_token(u.get("output_tokens")).unwrap_or(None),
            cache_read: opt_token(u.get("cache_read_input_tokens")).unwrap_or(None),
            cache_write: opt_token(u.get("cache_creation_input_tokens")).unwrap_or(None),
        });
    Some(MergedMessage {
        id: obj.get("id").and_then(|v| v.as_str()).map(str::to_string),
        role: obj
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        timestamp: rfc3339_ms(obj.get("timestamp")),
        tool_duration_ms: obj
            .get("tool_duration_ms")
            .and_then(|v| v.as_i64())
            .filter(|d| *d >= 0),
        usage,
    })
}

pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let mut context = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<JcodeParseContext>(v.clone()).ok())
        .unwrap_or_default();
    if target.rescan {
        context = JcodeParseContext::default();
    }
    context.version_basis = Some(VersionBasis::KnownVersion);
    let session_id = target
        .path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown-session")
        .to_string();
    // 快照 + journal sidecar（同 stem）。
    let journal_path = target.path.with_file_name(format!(
        "{}.journal.jsonl",
        target
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .trim_end_matches(".json")
    ));
    if target.probe.len > JCODE_MAX_FILE_BYTES {
        return Ok(ScanOutcome {
            status: ScanStatus::LineTooLong,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "file_exceeds_size_cap",
                "snapshot",
                "session snapshot exceeds the 64 MiB cap; cursor held",
            )],
            lines_read: 0,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    }
    let mut bytes = Vec::new();
    std::io::Read::take(
        &mut std::fs::File::open(&target.path)?,
        JCODE_MAX_FILE_BYTES + 1,
    )
    .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > JCODE_MAX_FILE_BYTES {
        return Ok(ScanOutcome {
            status: ScanStatus::LineTooLong,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "file_exceeds_size_cap",
                "snapshot",
                "session snapshot grew past the cap during read; cursor held",
            )],
            lines_read: 0,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    }
    let snapshot: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => {
            return Ok(ScanOutcome {
                status: ScanStatus::Pending,
                cursor: None,
                parse_context: None,
                events: Vec::new(),
                aggregates: Vec::new(),
                diagnostics: vec![diag(
                    "snapshot_unparseable",
                    "snapshot",
                    "session snapshot does not parse; retry next round",
                )],
                lines_read: 1,
                records_seen: 0,
                reconciliations: Vec::new(),
                health: "active".to_string(),
            });
        }
    };
    let previous_cursor = if target.rescan {
        None
    } else {
        stored
            .cursor
            .as_ref()
            .and_then(|v| serde_json::from_value::<JcodeCursor>(v.clone()).ok())
            .filter(|c| c.generation == target.generation && c.snapshot_len == bytes.len() as u64)
    };
    let journal_probe = if journal_path.is_file() {
        Some(crate::adapters::jsonl::probe_file(&journal_path)?)
    } else {
        None
    };
    let resume = previous_cursor.filter(|c| {
        c.journal_offset > 0
            && journal_probe.as_ref().is_some_and(|now| {
                c.journal_probe.is_some_and(|old| {
                    matches!(
                        crate::adapters::jsonl::decide_generation(
                            &crate::adapters::jsonl::StoredFileState {
                                generation: 0,
                                len: old.len,
                                created_ms: old.created_ms,
                                head_hash: old.head_hash,
                                head_len: old.head_len,
                                tail_hash: old.tail_hash,
                                cursor_offset: c.journal_offset,
                            },
                            now,
                        ),
                        crate::adapters::jsonl::GenerationDecision::Continue
                    )
                })
            })
    });
    if resume.is_none() {
        context.journal_provider_key = None;
        context.journal_model = None;
        context.journal_updated_at_ms = None;
    }
    // 元数据：快照值 + journal 逐条覆盖（最后一条 journal 为准）。
    let mut provider_key = snapshot
        .get("provider_key")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let mut model = snapshot
        .get("model")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let mut session_updated_at_ms = rfc3339_ms(snapshot.get("updated_at"));
    if resume.is_some() {
        provider_key = context.journal_provider_key.clone().or(provider_key);
        model = context.journal_model.clone().or(model);
        session_updated_at_ms = context.journal_updated_at_ms.or(session_updated_at_ms);
    }
    let mut messages: Vec<MergedMessage> = snapshot
        .get("messages")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(parse_message).collect())
        .unwrap_or_default();
    let mut records_seen: u64 = messages.len() as u64;
    let mut diagnostics = Vec::new();
    let mut next_journal_offset: u64 = 0;
    let mut next_journal_line: u64 = 1;
    let mut journal_stop: Option<crate::adapters::jsonl::StopReason> = None;
    if journal_path.is_file() {
        let read = crate::adapters::jsonl::read_jsonl(
            &journal_path,
            resume.map(|c| c.journal_offset).unwrap_or(0),
            resume.map(|c| c.journal_line).unwrap_or(1),
            &crate::adapters::jsonl::JsonlLimits {
                max_lines: Some(100_000),
                ..limits.jsonl.clone()
            },
        )?;
        next_journal_offset = read.next_offset;
        next_journal_line = read.next_line_number;
        for bad in &read.bad_lines {
            diagnostics.push(diag(
                bad.code,
                &format!("journal:line:{}", bad.number),
                "journal line not valid UTF-8; skipped",
            ));
        }
        for line in &read.lines {
            records_seen += 1;
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&line.text) else {
                diagnostics.push(diag(
                    "invalid_journal_line",
                    &format!("journal:line:{}", line.number),
                    "journal line is not JSON; skipped (upstream salvages by prefix)",
                ));
                continue;
            };
            if let Some(meta) = value.get("meta") {
                if let Some(pk) = meta.get("provider_key").and_then(|v| v.as_str()) {
                    provider_key = Some(pk.to_string());
                }
                if let Some(m) = meta.get("model").and_then(|v| v.as_str()) {
                    model = Some(m.to_string());
                }
                if let Some(updated_at_ms) = rfc3339_ms(meta.get("updated_at")) {
                    session_updated_at_ms = Some(updated_at_ms);
                }
            }
            if let Some(appended) = value.get("append_messages").and_then(|v| v.as_array()) {
                for message in appended {
                    if let Some(parsed) = parse_message(message) {
                        messages.push(parsed);
                    }
                }
            }
        }
        // journal 未读完（行数/超时时间或超限行）：尾部消息缺失，不能报
        // Complete。分页游标从本轮末尾续读；读到末页后下一轮从头
        // 重读，以便会话级 meta 更新能修订前页事件。
        match read.stop {
            crate::adapters::jsonl::StopReason::Eof => {
                next_journal_offset = 0;
                next_journal_line = 1;
            }
            stop => {
                journal_stop = Some(stop);
            }
        }
    }
    context.journal_provider_key = provider_key.clone();
    context.journal_model = model.clone();
    context.journal_updated_at_ms = session_updated_at_ms;
    // 处理崩溃窗口：同 id 消息后者覆盖前者（journal 权威，persistence 同语义）。
    let mut events = Vec::new();
    let mut seen_ids: std::collections::BTreeMap<String, ()> = Default::default();
    for message in messages.iter().rev() {
        if message.role != "assistant" {
            continue;
        }
        let Some(usage) = &message.usage else {
            continue; // 无 token_usage 的 assistant 消息：按官方规则保持未知，不补零。
        };
        // StoredMessage.id 是必填字段（官方类型）：缺失按格式偏离跳过记诊断。
        let Some(message_id) = &message.id else {
            diagnostics.push(diag(
                "message_without_id",
                &session_id,
                "assistant message lacks the required id field; skipped",
            ));
            continue;
        };
        if seen_ids.insert(message_id.clone(), ()).is_some() {
            continue; // journal 覆盖快照的同 id 消息：取 journal 版（先迭代）。
        }
        let key = format!("jcode:{session_id}:{message_id}");
        let Some(occurred_ms) = message.timestamp else {
            diagnostics.push(diag(
                "timestamp_unparseable",
                &key,
                "message timestamp missing/implausible; skipped",
            ));
            continue;
        };
        if usage.input.is_none() && usage.output.is_none() {
            continue;
        }
        let mapped = map_jcode(
            provider_key.as_deref(),
            usage.input,
            usage.output,
            usage.cache_read,
            usage.cache_write,
        );
        events.push(EventInput {
            source_instance_id: target.instance_id.clone(),
            source_record_key: key,
            record_kind: RecordKind::ModelCall,
            schema_version: JCODE_FORMAT_VERSION.to_string(),
            parser_version: JCODE_PARSER_VERSION.to_string(),
            parse_basis: Some(VersionBasis::KnownVersion),
            origin_call_id: None,
            attempt_id: None,
            session_id: Some(session_id.clone()),
            parent_session_id: None,
            host_application: None,
            agent: "jcode".to_string(),
            call_category: CallCategory::Primary,
            occurred_at_ms: occurred_ms,
            observed_at_ms: Some(now_ms),
            source_time: Some(occurred_ms.to_string()),
            // timestamp 是消息完成时刻（官方 storage_types 注释）。
            time_basis: TimeBasis::SourceCompletion,
            interval_start_ms: message
                .tool_duration_ms
                .and_then(|d| occurred_ms.checked_sub(d)),
            interval_end_ms: Some(occurred_ms),
            provider_id: provider_key.clone(),
            model_raw: model.clone(),
            model_canonical: None,
            model_attribution: ModelAttribution::StructuredChange,
            usage: mapped.usage,
            quality: mapped.quality,
            lifecycle: Lifecycle::Final,
            // journal meta.updated_at 是会话保存时间；消息完成时间不会随
            // 后续同 id 更正必然变化。缺失修订依据时由 ingest 保留冲突。
            source_revision: session_updated_at_ms,
            error_status: None,
            duration_ms: message.tool_duration_ms,
            ttft_ms: None,
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            cost: None,
        });
    }
    let status = match journal_stop {
        None => ScanStatus::Complete,
        Some(crate::adapters::jsonl::StopReason::Eof) => ScanStatus::Complete,
        Some(crate::adapters::jsonl::StopReason::LineBudget)
        | Some(crate::adapters::jsonl::StopReason::TimeBudget) => {
            diagnostics.push(diag(
                "journal_budget_exhausted",
                &session_id,
                "journal read stopped on budget; tail messages pending next round",
            ));
            ScanStatus::BudgetExhausted
        }
        Some(crate::adapters::jsonl::StopReason::LineTooLong { number, offset }) => {
            diagnostics.push(diag(
                "line_exceeds_cap",
                &session_id,
                &format!("journal line {number} at byte {offset} exceeds the cap; held for retry"),
            ));
            ScanStatus::LineTooLong
        }
    };
    Ok(ScanOutcome {
        status,
        cursor: Some(serde_json::to_value(JcodeCursor {
            generation: target.generation,
            snapshot_len: bytes.len() as u64,
            journal_offset: next_journal_offset,
            journal_line: next_journal_line,
            journal_probe: journal_probe.as_ref().map(JournalProbe::from),
        })?),
        parse_context: Some(serde_json::to_value(&context)?),
        events,
        aggregates: Vec::new(),
        diagnostics,
        lines_read: 1,
        records_seen,
        reconciliations: Vec::new(),
        health: "active".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_input_includes_cache() {
        let mapped = map_jcode(Some("openai"), Some(100), Some(20), Some(30), Some(10));
        assert_eq!(mapped.usage.input_total, Some(100));
        assert_eq!(mapped.usage.input_uncached, Some(60));
    }

    #[test]
    fn anthropic_buckets_mutually_exclusive() {
        let mapped = map_jcode(Some("anthropic"), Some(60), Some(20), Some(30), Some(10));
        assert_eq!(mapped.usage.input_uncached, Some(60));
        assert_eq!(mapped.usage.input_total, Some(100));
    }

    #[test]
    fn unknown_provider_no_derivation() {
        let mapped = map_jcode(None, Some(60), Some(20), Some(30), None);
        assert_eq!(mapped.usage.input_uncached, None);
        assert_eq!(mapped.usage.input_total, Some(60));
    }

    #[test]
    fn message_parse_optional_usage() {
        let message = parse_message(&serde_json::json!({
            "id": "m1", "role": "assistant", "timestamp": "2026-09-29T00:00:00Z",
            "token_usage": {"input_tokens": 5, "output_tokens": 2,
                            "cache_read_input_tokens": 3}
        }))
        .unwrap();
        assert_eq!(message.usage.as_ref().unwrap().cache_write, None);
        assert!(parse_message(&serde_json::json!("x")).is_none());
    }
}
