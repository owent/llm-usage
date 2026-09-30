//! 本地 CodeBuddy Code / WorkBuddy 会话 JSONL。两产品保留独立来源身份。
//! 字段依据：CodeBuddy 官方目录文档；tokmesh-core tencent_buddy.rs 的
//! 公开解析器和测试；aiusage v1.5.8 对 CodeBuddy 缓存重复计数的修正。
//! 无本机真实样本时仅承诺下列已见字段，偏离格式跳过并记录诊断。

use crate::adapters::framework::{
    Availability, CapabilityTable, DetectOutcome, DiscoverContext, DiscoveredRoot, RootBasis,
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, SourceAdapter, StoredScanState,
};
use crate::adapters::jsonl::{read_jsonl, JsonlCursor, StopReason};
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, FieldQuality, Lifecycle, ModelAttribution,
    RecordKind, TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use std::path::{Path, PathBuf};

const FORMAT: &str = "tencent-buddy-session-doc1";
const PARSER: &str = "tencent-buddy-session-doc1";
const MAX_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

pub struct BuddyAdapter<const WORK: bool>;

impl<const WORK: bool> Default for BuddyAdapter<WORK> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const WORK: bool> BuddyAdapter<WORK> {
    pub fn new() -> Self {
        Self
    }
    fn id() -> &'static str {
        if WORK {
            "workbuddy"
        } else {
            "codebuddy"
        }
    }
    fn dirname() -> &'static str {
        if WORK {
            ".workbuddy"
        } else {
            ".codebuddy"
        }
    }
}

fn diag(code: &str, line: u64, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.into(),
        field: None,
        position: Some(format!("line:{line}")),
        message: message.into(),
    }
}

fn token(obj: &serde_json::Value, names: &[&str]) -> Result<Option<i64>, ()> {
    for name in names {
        if let Some(value) = obj.get(*name) {
            return value
                .as_i64()
                .filter(|n| (0..=MAX_TOKEN).contains(n))
                .map(Some)
                .ok_or(());
        }
    }
    Ok(None)
}

fn timestamp(value: &serde_json::Value) -> Option<i64> {
    let ts = value.get("timestamp")?;
    let ms = ts.as_i64().or_else(|| ts.as_str()?.parse().ok())?;
    (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
        .contains(&ms)
        .then_some(ms)
}

fn restore_cursor(stored: &StoredScanState, target: &ScanTarget) -> JsonlCursor {
    if !target.rescan {
        if let Some(cursor) = stored
            .cursor
            .as_ref()
            .and_then(|v| serde_json::from_value::<JsonlCursor>(v.clone()).ok())
            .filter(|c| c.generation == target.generation)
        {
            return cursor;
        }
    }
    JsonlCursor {
        generation: target.generation,
        offset: 0,
        line_number: 1,
    }
}

fn parse_event<const WORK: bool>(
    value: &serde_json::Value,
    target: &ScanTarget,
    line: u64,
    now_ms: i64,
) -> Result<Option<EventInput>, &'static str> {
    let kind = value.get("type").and_then(|v| v.as_str());
    if kind != Some("function_call")
        && !(kind == Some("message")
            && value.get("role").and_then(|v| v.as_str()) == Some("assistant"))
    {
        return Ok(None);
    }
    if value
        .get("status")
        .and_then(|v| v.as_str())
        .is_some_and(|s| s != "completed")
    {
        return Ok(None);
    }
    let provider_data = value.get("providerData");
    let raw = provider_data.and_then(|v| v.get("rawUsage"));
    let usage = raw
        .or_else(|| provider_data.and_then(|v| v.get("usage")))
        .or_else(|| value.get("message").and_then(|v| v.get("usage")));
    let Some(usage) = usage else { return Ok(None) };
    if !usage.is_object() {
        return Err("usage is not an object");
    }
    let input = token(usage, &["prompt_tokens", "input_tokens", "inputTokens"])
        .map_err(|_| "invalid input token count")?;
    let output = token(
        usage,
        &["completion_tokens", "output_tokens", "outputTokens"],
    )
    .map_err(|_| "invalid output token count")?;
    let cache_read = token(
        usage,
        &[
            "prompt_cache_hit_tokens",
            "cache_read_input_tokens",
            "cacheReadInputTokens",
        ],
    )
    .map_err(|_| "invalid cache read token count")?;
    let cache_write = token(
        usage,
        &[
            "prompt_cache_write_tokens",
            "cache_creation_input_tokens",
            "cacheCreationInputTokens",
        ],
    )
    .map_err(|_| "invalid cache write token count")?;
    let cache_miss = token(
        usage,
        &[
            "prompt_cache_miss_tokens",
            "cachedMissTokens",
            "cacheMissTokens",
        ],
    )
    .map_err(|_| "invalid cache miss token count")?;
    let reasoning = token(usage, &["completion_thinking_tokens", "reasoningTokens"])
        .map_err(|_| "invalid reasoning token count")?;
    if reasoning
        .zip(output)
        .is_some_and(|(thinking, total)| thinking > total)
    {
        return Err("reasoning exceeds output");
    }
    let reported_total =
        token(usage, &["total_tokens", "totalTokens"]).map_err(|_| "invalid total token count")?;
    if input.is_none() && output.is_none() && cache_miss.is_none() {
        return Ok(None);
    }
    let occurred_ms = timestamp(value).ok_or("missing or implausible timestamp")?;
    let session = value
        .get("sessionId")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .or_else(|| target.path.file_stem().and_then(|s| s.to_str()))
        .ok_or("missing session identity")?;
    let record = provider_data
        .and_then(|v| v.get("messageId").or_else(|| v.get("traceId")))
        .or_else(|| value.get("id"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or("missing message identity")?;
    // CodeBuddy 的 message.usage.input_tokens 已含 cache_read。rawUsage 的
    // prompt_cache_miss 是明确的未缓存桶；其余只在可核对时作减法。
    let uncached = match (cache_miss, input, cache_read) {
        (Some(miss), Some(total), Some(hit)) if miss.checked_add(hit) == Some(total) => Some(miss),
        (Some(_), Some(_), Some(_)) => return Err("input/cache buckets disagree"),
        (Some(miss), Some(total), None) if miss <= total => Some(miss),
        (Some(_), Some(_), None) => return Err("cache miss exceeds input"),
        (Some(miss), None, _) => Some(miss),
        (None, Some(total), Some(hit)) if total >= hit => Some(total - hit),
        (None, Some(_), Some(_)) => return Err("cache read exceeds input"),
        _ => None,
    };
    if input.is_some() && cache_read.is_some() && uncached.is_none() {
        return Err("cache read exceeds input");
    }
    let input_total = input.or_else(|| {
        cache_miss
            .zip(cache_read)
            .and_then(|(a, b)| a.checked_add(b))
    });
    if input.is_none() && cache_miss.is_some() && cache_read.is_some() && input_total.is_none() {
        return Err("derived input overflow");
    }
    if input_total.is_some_and(|v| v > MAX_TOKEN) {
        return Err("derived input exceeds token bound");
    }
    let derived_total = input_total.zip(output).and_then(|(a, b)| a.checked_add(b));
    if derived_total.is_some_and(|v| v > MAX_TOKEN) {
        return Err("derived total exceeds token bound");
    }
    if reported_total
        .zip(derived_total)
        .is_some_and(|(reported, derived)| reported != derived)
    {
        return Err("reported total disagrees with input plus output");
    }
    let total = derived_total.or(reported_total);
    let model = provider_data
        .and_then(|v| v.get("model").or_else(|| v.get("requestModelId")))
        .or_else(|| value.get("message").and_then(|v| v.get("model")))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let mapped = crate::adapters::usage_map::finish(
        crate::domain::TokenUsage {
            input_uncached: uncached,
            input_cache_read: cache_read,
            input_cache_write: cache_write,
            input_total,
            output_total: output,
            output_reasoning: reasoning,
            total_tokens: total,
            source_total: reported_total,
        },
        crate::domain::TokenQuality {
            input_uncached: uncached
                .map(|_| {
                    if cache_miss.is_some() {
                        FieldQuality::Reported
                    } else {
                        FieldQuality::Derived
                    }
                })
                .unwrap_or(FieldQuality::Unknown),
            input_cache_read: cache_read
                .map(|_| FieldQuality::Reported)
                .unwrap_or(FieldQuality::Unknown),
            input_cache_write: cache_write
                .map(|_| FieldQuality::Reported)
                .unwrap_or(FieldQuality::Unknown),
            input_total: input.map(|_| FieldQuality::Reported).unwrap_or(
                if input_total.is_some() {
                    FieldQuality::Derived
                } else {
                    FieldQuality::Unknown
                },
            ),
            output_total: output
                .map(|_| FieldQuality::Reported)
                .unwrap_or(FieldQuality::Unknown),
            output_reasoning: reasoning
                .map(|_| FieldQuality::Reported)
                .unwrap_or(FieldQuality::Unknown),
            total_tokens: if reported_total.is_some() {
                FieldQuality::Reported
            } else if derived_total.is_some() {
                FieldQuality::Derived
            } else {
                FieldQuality::Unknown
            },
            source_total: reported_total
                .map(|_| FieldQuality::Reported)
                .unwrap_or(FieldQuality::Unknown),
        },
        Vec::new(),
    );
    Ok(Some(EventInput {
        source_instance_id: target.instance_id.clone(),
        source_record_key: format!("{}:{session}:{record}", BuddyAdapter::<WORK>::id()),
        record_kind: RecordKind::ModelCall,
        schema_version: FORMAT.into(),
        parser_version: PARSER.into(),
        parse_basis: Some(VersionBasis::KnownVersion),
        origin_call_id: None,
        attempt_id: None,
        session_id: Some(session.into()),
        parent_session_id: None,
        host_application: None,
        agent: BuddyAdapter::<WORK>::id().into(),
        call_category: if kind == Some("function_call") {
            CallCategory::Auxiliary
        } else {
            CallCategory::Primary
        },
        occurred_at_ms: occurred_ms,
        observed_at_ms: Some(now_ms),
        source_time: Some(occurred_ms.to_string()),
        time_basis: TimeBasis::SourceCompletion,
        interval_start_ms: None,
        interval_end_ms: Some(occurred_ms),
        provider_id: None,
        model_raw: model,
        model_canonical: None,
        model_attribution: ModelAttribution::RequestField,
        usage: mapped.usage,
        quality: mapped.quality,
        lifecycle: Lifecycle::Final,
        source_revision: Some(
            target
                .generation
                .saturating_mul(1_000_000_000)
                .saturating_add(line as i64),
        ),
        error_status: None,
        duration_ms: None,
        ttft_ms: None,
        attribution_status: AttributionStatus::Verified,
        exclusion_reason: None,
        cost: None,
    }))
}

impl<const WORK: bool> SourceAdapter for BuddyAdapter<WORK> {
    fn adapter_id(&self) -> &'static str {
        Self::id()
    }
    fn agent(&self) -> &'static str {
        Self::id()
    }

    fn discover(&self, ctx: &DiscoverContext) -> Vec<DiscoveredRoot> {
        let mut candidates: Vec<(PathBuf, RootBasis)> = Vec::new();
        if !WORK {
            if let Some(home) = ctx
                .env
                .get("CODEBUDDY_CONFIG_DIR")
                .filter(|s| !s.trim().is_empty())
            {
                candidates.push((
                    PathBuf::from(home).join("projects"),
                    RootBasis::EnvOverride("CODEBUDDY_CONFIG_DIR".into()),
                ));
            }
        }
        if let Some(home) = &ctx.home_dir {
            candidates.push((
                home.join(Self::dirname()).join("projects"),
                RootBasis::DefaultHome,
            ));
        }
        for manual in &ctx.manual_roots {
            // 手工根只接受明确的产品 projects 目录，防止两个同形解析器互抢。
            if manual.file_name().and_then(|s| s.to_str()) == Some("projects")
                && manual
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|s| s.to_str())
                    == Some(Self::dirname())
            {
                candidates.push((manual.clone(), RootBasis::Manual));
            }
        }
        let mut seen = std::collections::BTreeSet::new();
        candidates
            .into_iter()
            .filter_map(|(root, basis)| {
                if !seen.insert(root.clone()) {
                    return None;
                }
                let files = crate::adapters::framework::enumerate_files_bounded(&root, 3, &|p| {
                    p.extension().and_then(|s| s.to_str()) == Some("jsonl")
                        && !p.components().any(|c| c.as_os_str() == "tool-results")
                });
                (!files.is_empty()).then_some(DiscoveredRoot { root, basis, files })
            })
            .collect()
    }

    fn instance_id(&self, root: &DiscoveredRoot) -> String {
        format!(
            "{}@{}",
            Self::id(),
            crate::adapters::framework::normalize_path(&root.root)
        )
    }

    fn detect(&self, path: &Path) -> Result<DetectOutcome, CoreError> {
        let Some(head) = crate::adapters::framework::read_detect_head(path, 64 * 1024)? else {
            return Ok(DetectOutcome::Pending);
        };
        if head.is_empty() {
            return Ok(DetectOutcome::Pending);
        }
        let text = String::from_utf8_lossy(&head);
        if text.contains("\"type\"") && text.contains("\"sessionId\"") {
            Ok(DetectOutcome::Supported {
                format: FORMAT.into(),
                format_version: Some(FORMAT.into()),
                basis: VersionBasis::KnownVersion,
            })
        } else {
            Ok(DetectOutcome::UnknownFormat {
                reason: "missing Tencent Buddy session fingerprint".into(),
            })
        }
    }

    fn scan(
        &self,
        target: &ScanTarget,
        stored: &StoredScanState,
        limits: &ScanLimits,
        now_ms: i64,
    ) -> Result<ScanOutcome, CoreError> {
        let cursor = restore_cursor(stored, target);
        let read = read_jsonl(
            &target.path,
            cursor.offset,
            cursor.line_number,
            &limits.jsonl,
        )?;
        let mut events = Vec::new();
        let mut diagnostics = Vec::new();
        for bad in &read.bad_lines {
            diagnostics.push(diag(bad.code, bad.number, "invalid UTF-8 line"));
        }
        for line in &read.lines {
            match serde_json::from_str::<serde_json::Value>(&line.text) {
                Ok(value) => match parse_event::<WORK>(&value, target, line.number, now_ms) {
                    Ok(Some(event)) => events.push(event),
                    Ok(None) => {}
                    Err(reason) => {
                        diagnostics.push(diag("tencent_buddy_record_invalid", line.number, reason))
                    }
                },
                Err(_) => {
                    diagnostics.push(diag("invalid_json_line", line.number, "line is not JSON"))
                }
            }
        }
        let status = match read.stop {
            StopReason::Eof => ScanStatus::Complete,
            StopReason::LineBudget | StopReason::TimeBudget => ScanStatus::BudgetExhausted,
            StopReason::LineTooLong { number, offset } => {
                diagnostics.push(diag(
                    "line_exceeds_cap",
                    number,
                    &format!("line at byte {offset} exceeds cap"),
                ));
                ScanStatus::LineTooLong
            }
        };
        Ok(ScanOutcome {
            status,
            cursor: Some(serde_json::to_value(JsonlCursor {
                generation: target.generation,
                offset: read.next_offset,
                line_number: read.next_line_number,
            })?),
            parse_context: None,
            events,
            aggregates: Vec::new(),
            diagnostics,
            lines_read: read.lines.len() as u64,
            records_seen: read.lines.len() as u64,
            reconciliations: Vec::new(),
            health: "active".into(),
        })
    }

    fn capability(&self) -> CapabilityTable {
        let evidence = "CodeBuddy 官方目录文档；tokmesh-core/agent-hud-open/aiusage 第三方源码；本机无真实样本";
        let mut fields = serde_json::Map::new();
        for name in [
            "tokens",
            "cache_read",
            "cache_write",
            "per_request_calls",
            "model",
            "time",
        ] {
            fields.insert(name.into(), serde_json::json!({ "availability": Availability::Partial(evidence.into()), "note": "本地 projects/**/*.jsonl；按消息身份去重，字段缺失保持未知" }));
        }
        CapabilityTable {
            adapter_id: Self::id().into(),
            product: if WORK {
                "WorkBuddy"
            } else {
                "CodeBuddy Code CLI"
            }
            .into(),
            surfaces: vec![if WORK { "desktop" } else { "cli" }.into()],
            supported_versions: vec![FORMAT.into()],
            discovery: serde_json::json!({ "default_roots": [format!("~/{}/projects", Self::dirname())], "env_override": if WORK { None } else { Some("CODEBUDDY_CONFIG_DIR") }, "manual_roots": "产品目录下的 projects", "bounded": true }),
            detection: serde_json::json!({ "magic": "sessionId + type", "version_field": null, "registry": "文档级格式锚点 tencent-buddy-session-doc1", "fail_closed": true }),
            fields,
            lifecycle: serde_json::json!({ "records": "已完成 assistant message/function_call；本地 JSONL 增量", "cache_semantics": "CodeBuddy input 含 cache read；优先 rawUsage 的 miss/hit 分桶" }),
            incremental: serde_json::json!({ "cursor": "JSONL 字节偏移和行号" }),
            dedup: serde_json::json!({ "primary": "产品:sessionId:providerData.messageId/traceId/id", "revision": "同文件后写行号" }),
            integrity: serde_json::json!({ "unknown": "缺 token/timestamp/身份不推算，记录诊断" }),
            maintenance: serde_json::json!({ "parser_version": PARSER, "evidence_level": "third-party-source; no local fixture" }),
            scheduling: serde_json::json!({ "entry": "统一 run_adapter_scan" }),
            limitations: vec![
                "闭源格式且当前本机无真实样本，需真实文件核对".into(),
                "CodeBuddy OTLP 与本地日志同时启用时可能重复计数".into(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::jsonl::probe_file;

    fn fixture(name: &str) -> PathBuf {
        let dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../build/tencent-buddy-tests");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(format!("{name}-{}.jsonl", std::process::id()))
    }

    fn target(path: PathBuf) -> ScanTarget {
        ScanTarget {
            instance_id: "codebuddy@test".into(),
            file_id: path.to_string_lossy().into(),
            file_identity: "fixture".into(),
            probe: probe_file(&path).unwrap(),
            generation: 0,
            rescan: false,
            path,
        }
    }

    #[test]
    fn codebuddy_cache_inclusive_input_and_raw_usage() {
        let path = fixture("codebuddy");
        std::fs::write(&path, concat!(
            "{\"id\":\"a\",\"timestamp\":1780000000100,\"type\":\"message\",\"role\":\"assistant\",\"sessionId\":\"s\",\"providerData\":{\"model\":\"glm-5.2\",\"messageId\":\"m1\"},\"message\":{\"usage\":{\"input_tokens\":24486,\"output_tokens\":3,\"cache_read_input_tokens\":14720}}}\n",
            "{\"id\":\"b\",\"timestamp\":1780000000200,\"type\":\"function_call\",\"status\":\"completed\",\"sessionId\":\"s\",\"providerData\":{\"messageId\":\"m2\",\"rawUsage\":{\"prompt_tokens\":100,\"completion_tokens\":20,\"prompt_cache_hit_tokens\":80,\"prompt_cache_miss_tokens\":20}}}\n"
        )).unwrap();
        let cleanup = path.clone();
        let outcome = BuddyAdapter::<false>::new()
            .scan(
                &target(path),
                &StoredScanState::default(),
                &ScanLimits::default(),
                1_800_000_000_000,
            )
            .unwrap();
        assert_eq!(outcome.events.len(), 2);
        assert_eq!(outcome.events[0].usage.input_total, Some(24486));
        assert_eq!(outcome.events[0].usage.input_uncached, Some(9766));
        assert_eq!(outcome.events[0].usage.total_tokens, Some(24489));
        assert_eq!(outcome.events[1].usage.input_total, Some(100));
        assert_eq!(outcome.events[1].usage.input_uncached, Some(20));
        assert_eq!(outcome.events[1].usage.total_tokens, Some(120));
        std::fs::remove_file(cleanup).unwrap();
    }

    #[test]
    fn workbuddy_separate_identity_and_invalid_records() {
        let path = fixture("workbuddy");
        std::fs::write(&path, concat!(
            "{\"id\":\"m1\",\"timestamp\":1780000000100,\"type\":\"function_call\",\"sessionId\":\"s\",\"message\":{\"usage\":{\"input_tokens\":10,\"output_tokens\":2,\"cache_read_input_tokens\":3}},\"providerData\":{\"messageId\":\"p1\",\"usage\":{\"inputTokens\":10,\"outputTokens\":2},\"rawUsage\":{\"prompt_tokens\":10,\"completion_tokens\":2,\"total_tokens\":12,\"prompt_cache_hit_tokens\":3,\"prompt_cache_miss_tokens\":7}}}\n",
            "{\"id\":\"m2\",\"timestamp\":1780000000100,\"type\":\"message\",\"role\":\"assistant\",\"sessionId\":\"s\",\"message\":{\"usage\":{\"input_tokens\":1,\"cache_read_input_tokens\":2}}}\n",
            "{\"id\":\"m3\",\"timestamp\":1780000000100,\"type\":\"message\",\"role\":\"assistant\",\"sessionId\":\"s\",\"message\":{\"usage\":{\"input_tokens\":10,\"output_tokens\":2,\"total_tokens\":99}}}\n"
        )).unwrap();
        let cleanup = path.clone();
        let outcome = BuddyAdapter::<true>::new()
            .scan(
                &target(path),
                &StoredScanState::default(),
                &ScanLimits::default(),
                1_800_000_000_000,
            )
            .unwrap();
        assert_eq!(
            outcome.events.len(),
            1,
            "{:?}",
            outcome
                .events
                .iter()
                .map(|e| (&e.source_record_key, &e.usage))
                .collect::<Vec<_>>()
        );
        assert_eq!(outcome.events[0].agent, "workbuddy");
        assert_eq!(outcome.events[0].usage.input_uncached, Some(7));
        assert_eq!(outcome.events[0].usage.total_tokens, Some(12));
        assert_eq!(outcome.diagnostics.len(), 2);
        std::fs::remove_file(cleanup).unwrap();
    }

    #[test]
    fn discovery_is_product_scoped_and_cursor_is_incremental() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../build/tencent-buddy-tests")
            .join(format!("discovery-{}", std::process::id()));
        let code_projects = root.join(".codebuddy/projects/demo");
        let work_projects = root.join(".workbuddy/projects/demo");
        std::fs::create_dir_all(&code_projects).unwrap();
        std::fs::create_dir_all(&work_projects).unwrap();
        let row = "{\"id\":\"m1\",\"timestamp\":1780000000100,\"type\":\"message\",\"role\":\"assistant\",\"sessionId\":\"s\",\"message\":{\"usage\":{\"input_tokens\":12,\"output_tokens\":2}}}\n";
        let code_file = code_projects.join("s.jsonl");
        let work_file = work_projects.join("s.jsonl");
        std::fs::write(&code_file, row).unwrap();
        std::fs::write(&work_file, row).unwrap();
        let ctx = DiscoverContext {
            home_dir: Some(root.clone()),
            env: Default::default(),
            manual_roots: vec![],
        };
        let code_roots = BuddyAdapter::<false>::new().discover(&ctx);
        let work_roots = BuddyAdapter::<true>::new().discover(&ctx);
        assert_eq!(code_roots.len(), 1);
        assert_eq!(work_roots.len(), 1);
        assert_eq!(code_roots[0].files, vec![code_file.clone()]);
        assert_eq!(work_roots[0].files, vec![work_file.clone()]);
        assert!(matches!(
            BuddyAdapter::<false>::new().detect(&code_file).unwrap(),
            DetectOutcome::Supported { .. }
        ));
        let scan_target = target(code_file.clone());
        let first = BuddyAdapter::<false>::new()
            .scan(
                &scan_target,
                &StoredScanState::default(),
                &ScanLimits::default(),
                1_800_000_000_000,
            )
            .unwrap();
        assert_eq!(first.events.len(), 1);
        let stored = StoredScanState {
            cursor: first.cursor,
            ..Default::default()
        };
        let second = BuddyAdapter::<false>::new()
            .scan(
                &scan_target,
                &stored,
                &ScanLimits::default(),
                1_800_000_000_000,
            )
            .unwrap();
        assert!(second.events.is_empty());
        std::fs::remove_file(code_file).unwrap();
        std::fs::remove_file(work_file).unwrap();
        for project in [&code_projects, &work_projects] {
            std::fs::remove_dir(project).unwrap();
            std::fs::remove_dir(project.parent().unwrap()).unwrap();
            std::fs::remove_dir(project.parent().unwrap().parent().unwrap()).unwrap();
        }
        std::fs::remove_dir(root).unwrap();
    }
}
