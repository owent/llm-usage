//! Command Code v3 树形会话 JSONL 格式实现（`tree_v3`，commandcode-tree-v3）。
//!
//! 格式证据（官方 npm 分发物 command-code@1.69.0 dist/cli.mjs 逐行核对；
//! 仓库无产品源码（仅 readme），闭源；本机未安装、无真实样本）：
//! - 路径：`~/.commandcode/projects/<slug>/*.jsonl`（slug=@sindresorhus/slugify(cwd)；
//!   **HOME 优先于 USERPROFILE**——与采集器取根顺序可能不同，同用户目录时一致）；
//!   会话文件名排除 `.checkpoints.`/`.prompts.`/`.v2.bak`。
//! - 首行 header `{type:"session", version:3, id, timestamp(ISO), cwd,
//!   parentSession?}`；entry `{type, id(8hex), parentId, timestamp(ISO)}`，
//!   类型集：message/model_change/effort_change/compaction/branch_summary/
//!   custom/custom_message/label/session_info。
//! - assistant message 携带 `usage{inputTokens, outputTokens, cacheReadTokens,
//!   cacheWriteTokens, [cacheWriteTokens1h], [costUsd]}`（四桶完成请求必写，
//!   `?? 0` 归一化 ⇒ **全零=已报告零**；缺 usage 字段=未完成/中断，未知）。
//!   **inputTokens 含 cache 读/写**（官方成本公式 max(0,input−cacheR−cacheW)
//!   证实子集关系）；costUsd 为本地费率估算（费率缺失时不写字段）。
//! - 有效路径口径（官方 buildSessionPath/getTree）：从**文件最后一条 entry**
//!   回溯 parentId 链至根；rewind 只把 head 指回旧 entry，被弃分支留在文件中
//!   （append-only）⇒ **孤儿分支不计**。compaction 折叠不剔除真实调用：
//!   链上条目（含压缩前）全计。
//! - fork：根→目标 leaf 的 entries 原样复制进新文件（**id 与 timestamp 保留
//!   原值**）⇒ 跨文件去重键 = `cmd:<entry id>:<timestamp>`（8hex id 仅 32bit
//!   跨文件可能碰撞，加时间戳增强）。
//! - 子代理 usage 不写盘（subagentProgressTranslator 过滤）：文件内 usage 仅
//!   主对话请求。
//! - 时间戳 ISO（v2 是毫秒 number，v1 anthropic 旧形状——非 v3 不读）。

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::usage_map::{finish, sub_checked, MappedUsage};
use crate::domain::{
    AttributionStatus, CallCategory, CostAmount, CostKind, EventInput, Lifecycle, ModelAttribution,
    RecordKind, TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use std::io::Read as _;

use super::COMMANDCODE_FORMAT_VERSION;

pub const COMMANDCODE_PARSER_VERSION: &str = "commandcode-tree-v3";
/// 单文件有界读取上限（整文件树重建）。
pub const CMD_MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

const DOCUMENTED_ENTRY_TYPES: &[&str] = &[
    "message",
    "model_change",
    "effort_change",
    "compaction",
    "branch_summary",
    "custom",
    "custom_message",
    "label",
    "session_info",
];

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct WholeFileCursor {
    generation: i64,
    offset: u64,
    #[allow(dead_code)]
    line_number: u64,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct CmdParseContext {
    #[serde(default)]
    version_basis: Option<VersionBasis>,
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

fn usd_cost(value: Option<&serde_json::Value>) -> Option<CostAmount> {
    let amount = value?.as_f64()?;
    if !amount.is_finite() || amount < 0.0 {
        return None;
    }
    let micros = amount * 1_000_000.0;
    if micros > i64::MAX as f64 {
        return None;
    }
    Some(CostAmount {
        amount_minor: micros.round() as i64,
        currency: "USD".to_string(),
        // 本地费率估算（estimateSessionCostUsd，费率缺失时不写字段）。
        kind: CostKind::Estimated,
        price_version: None,
        billing_scope: None,
    })
}

/// inputTokens 含 cache 读/写（官方成本公式证实）⇒ uncached 派生。
fn map_cmd_usage(
    input: Option<i64>,
    output: Option<i64>,
    cache_read: Option<i64>,
    cache_write: Option<i64>,
) -> MappedUsage {
    let mut contradictions = Vec::new();
    let uncached = match (input, cache_read, cache_write) {
        (Some(total), Some(r), Some(w)) => sub_checked(
            "input_uncached",
            total,
            r.saturating_add(w),
            &mut contradictions,
        ),
        _ => None,
    };
    let total = match (input, output) {
        (Some(i), Some(o)) => i.checked_add(o),
        _ => None,
    };
    let usage = crate::domain::TokenUsage {
        input_uncached: uncached,
        input_cache_read: cache_read,
        input_cache_write: cache_write,
        input_total: input,
        output_total: output,
        output_reasoning: None,
        total_tokens: total,
        source_total: None,
    };
    let quality = crate::domain::TokenQuality {
        input_uncached: if uncached.is_some() {
            crate::domain::FieldQuality::Derived
        } else {
            crate::domain::FieldQuality::Unknown
        },
        input_cache_read: cache_read
            .map(|_| crate::domain::FieldQuality::Reported)
            .unwrap_or(crate::domain::FieldQuality::Unknown),
        input_cache_write: cache_write
            .map(|_| crate::domain::FieldQuality::Reported)
            .unwrap_or(crate::domain::FieldQuality::Unknown),
        input_total: input
            .map(|_| crate::domain::FieldQuality::Reported)
            .unwrap_or(crate::domain::FieldQuality::Unknown),
        output_total: output
            .map(|_| crate::domain::FieldQuality::Reported)
            .unwrap_or(crate::domain::FieldQuality::Unknown),
        total_tokens: if total.is_some() {
            crate::domain::FieldQuality::Derived
        } else {
            crate::domain::FieldQuality::Unknown
        },
        ..Default::default()
    };
    finish(usage, quality, contradictions)
}

struct EntryLite {
    id: String,
    parent_id: Option<String>,
    entry_type: String,
    /// assistant message 位置：usage 四桶（Some=字段在场）。
    usage_input: Option<i64>,
    usage_output: Option<i64>,
    usage_cache_read: Option<i64>,
    usage_cache_write: Option<i64>,
    cost: Option<f64>,
    model: Option<String>,
    timestamp: Option<i64>,
    is_assistant: bool,
}

pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    _limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let mut context = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<CmdParseContext>(v.clone()).ok())
        .unwrap_or_default();
    if target.rescan {
        context = CmdParseContext::default();
    }
    context.version_basis = Some(VersionBasis::KnownVersion);
    let mut diagnostics = Vec::new();
    if target.probe.len > CMD_MAX_FILE_BYTES {
        return Ok(ScanOutcome {
            status: ScanStatus::LineTooLong,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "file_exceeds_size_cap",
                "document",
                "session transcript exceeds the 32 MiB cap; cursor held",
            )],
            lines_read: 0,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    }
    // 整文件读取（树重建需要全局 parentId 图；追加式文件无变化时框架短路）。
    let mut bytes = Vec::new();
    std::io::Read::take(
        &mut std::fs::File::open(&target.path)?,
        CMD_MAX_FILE_BYTES + 1,
    )
    .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > CMD_MAX_FILE_BYTES {
        return Ok(ScanOutcome {
            status: ScanStatus::LineTooLong,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "file_exceeds_size_cap",
                "document",
                "session transcript grew past the cap during read; cursor held",
            )],
            lines_read: 0,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    }
    let text = String::from_utf8_lossy(&bytes);
    let mut entries: Vec<EntryLite> = Vec::new();
    let mut session_id = String::new();
    let mut records_seen: u64 = 0;
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        records_seen += 1;
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            // crash 截断行：上游 safeParseRecord 计入 corrupted 不中断。
            diagnostics.push(diag(
                "corrupted_line",
                &format!("line:{}", index + 1),
                "line does not parse (crash truncation?); counted as corrupted, skipped",
            ));
            continue;
        };
        let entry_type = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if index == 0 || entry_type == "session" {
            session_id = value
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            continue;
        }
        if !DOCUMENTED_ENTRY_TYPES.contains(&entry_type) {
            return Ok(ScanOutcome {
                status: ScanStatus::Pending,
                cursor: None,
                parse_context: None,
                events: Vec::new(),
                aggregates: Vec::new(),
                diagnostics: vec![diag(
                    "undocumented_entry_type",
                    &format!("line:{}", index + 1),
                    &format!("entry type {entry_type:?} not in the documented v3 set"),
                )],
                lines_read: records_seen,
                records_seen,
                reconciliations: Vec::new(),
                health: "degraded".to_string(),
            });
        }
        let message = value.get("message").and_then(|v| v.as_object());
        let is_assistant =
            message.and_then(|m| m.get("role")).and_then(|r| r.as_str()) == Some("assistant");
        let usage = value.get("usage").and_then(|v| v.as_object());
        let bucket = |key: &str| -> Option<Option<i64>> {
            match usage.and_then(|u| u.get(key)) {
                None => Some(None),
                Some(v) => {
                    let n = v.as_i64()?;
                    Some((0..=MAX_REASONABLE_TOKEN).contains(&n).then_some(n))
                }
            }
        };
        let (u_in, u_out, u_cr, u_cw) = if is_assistant && usage.is_some() {
            match (
                bucket("inputTokens"),
                bucket("outputTokens"),
                bucket("cacheReadTokens"),
                bucket("cacheWriteTokens"),
            ) {
                (Some(a), Some(b), Some(c), Some(d)) => (a, b, c, d),
                _ => {
                    diagnostics.push(diag(
                        "token_shape_deviation",
                        &format!("line:{}", index + 1),
                        "usage bucket carries an out-of-range value; entry skipped",
                    ));
                    (None, None, None, None)
                }
            }
        } else {
            (None, None, None, None)
        };
        entries.push(EntryLite {
            id: value
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            parent_id: value
                .get("parentId")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            entry_type: entry_type.to_string(),
            usage_input: u_in,
            usage_output: u_out,
            usage_cache_read: u_cr,
            usage_cache_write: u_cw,
            cost: usage
                .and_then(|u| u.get("costUsd"))
                .and_then(|v| v.as_f64())
                .filter(|c| c.is_finite() && *c >= 0.0),
            model: value
                .get("model")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            timestamp: rfc3339_ms(value.get("timestamp")),
            is_assistant,
        });
    }
    // 有效路径：文件最后一条 entry 回溯 parentId 链（官方 buildSessionPath）。
    let by_id: std::collections::BTreeMap<&str, usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.id.is_empty())
        .map(|(i, e)| (e.id.as_str(), i))
        .collect();
    let mut on_path = vec![false; entries.len()];
    let mut cursor: Option<usize> = entries.len().checked_sub(1);
    let mut current_model: Option<String> = None;
    // 末条目回溯 parentId 链标记 on_path（环保护）；model_change 归属在
    // 下一步按文件顺序推进时完成。
    while let Some(index) = cursor {
        if on_path[index] {
            break; // 环保护。
        }
        on_path[index] = true;
        let entry = &entries[index];
        cursor = entry
            .parent_id
            .as_deref()
            .and_then(|p| by_id.get(p).copied());
    }
    // 按文件顺序输出链上 assistant usage 事件（模型状态沿文件顺序）。
    let mut events = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        if entry.entry_type == "model_change" && on_path[index] {
            if let Some(model) = &entry.model {
                current_model = Some(model.clone());
            }
        }
        if !entry.is_assistant || !on_path[index] {
            continue;
        }
        // usage 字段缺失 = 未完成/中断（官方未知口径）：不入账。
        if entry.usage_input.is_none()
            && entry.usage_output.is_none()
            && entry.usage_cache_read.is_none()
            && entry.usage_cache_write.is_none()
        {
            continue;
        }
        let Some(occurred_ms) = entry.timestamp else {
            diagnostics.push(diag(
                "timestamp_unparseable",
                &format!("entry:{}", entry.id),
                "entry timestamp missing/implausible; skipped",
            ));
            continue;
        };
        if entry.id.is_empty() {
            continue;
        }
        let mapped = map_cmd_usage(
            entry.usage_input,
            entry.usage_output,
            entry.usage_cache_read,
            entry.usage_cache_write,
        );
        events.push(EventInput {
            source_instance_id: target.instance_id.clone(),
            // fork 复制保留 id+timestamp ⇒ 跨文件折叠键（官方口径）。
            source_record_key: format!("cmd:{}:{}", entry.id, occurred_ms),
            record_kind: RecordKind::ModelCall,
            schema_version: COMMANDCODE_FORMAT_VERSION.to_string(),
            parser_version: COMMANDCODE_PARSER_VERSION.to_string(),
            parse_basis: Some(VersionBasis::KnownVersion),
            origin_call_id: None,
            attempt_id: None,
            session_id: Some(session_id.clone()),
            parent_session_id: None,
            host_application: None,
            agent: "command-code".to_string(),
            call_category: CallCategory::Primary,
            occurred_at_ms: occurred_ms,
            observed_at_ms: Some(now_ms),
            source_time: Some(occurred_ms.to_string()),
            time_basis: TimeBasis::SourceCompletion,
            interval_start_ms: None,
            interval_end_ms: None,
            provider_id: None,
            model_raw: entry.model.clone().or_else(|| current_model.clone()),
            model_canonical: None,
            model_attribution: ModelAttribution::StructuredChange,
            usage: mapped.usage,
            quality: mapped.quality,
            lifecycle: Lifecycle::Final,
            source_revision: None,
            error_status: None,
            duration_ms: None,
            ttft_ms: None,
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            cost: usd_cost(entry.cost.map(serde_json::Value::from).as_ref()),
        });
    }
    Ok(ScanOutcome {
        status: ScanStatus::Complete,
        cursor: Some(serde_json::to_value(WholeFileCursor {
            generation: target.generation,
            offset: bytes.len() as u64,
            line_number: 1,
        })?),
        parse_context: Some(serde_json::to_value(&context)?),
        events,
        aggregates: Vec::new(),
        diagnostics,
        lines_read: records_seen,
        records_seen,
        reconciliations: Vec::new(),
        health: "active".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_input_includes_cache() {
        let mapped = map_cmd_usage(Some(100), Some(20), Some(30), Some(10));
        assert_eq!(mapped.usage.input_total, Some(100));
        assert_eq!(mapped.usage.input_uncached, Some(60));
        assert_eq!(mapped.usage.total_tokens, Some(120));
    }

    #[test]
    fn cost_optional() {
        assert!(usd_cost(Some(&serde_json::json!(0.25))).is_some());
        assert!(usd_cost(None).is_none());
    }
}
