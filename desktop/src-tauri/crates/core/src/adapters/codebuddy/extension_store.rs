//! CodeBuddy IDE/插件（CodeBuddyExtension）会话存储的请求级用量载体。
//! 字段依据：2026-09-30 本机只读核验（Windows，扩展数据目录
//! `%LOCALAPPDATA%\CodeBuddyExtension\Data`）：
//! - `Data/<profile>/<host>/<workspace>/history/<session>/<conversation>/index.json`
//!   含 `requests[]`，每项 `{id, type, messages, state, startedAt, usage}`；
//!   `usage` 为该请求聚合：`inputTokens = cacheTokens(读命中) +
//!   cachedMissTokens(未缓存)`，`totalTokens = inputTokens + outputTokens`，
//!   另有 `cachedWriteTokens`、`lastTokens`（语义尚未验证，不映射）与 `credit`
//!   （平台积分，非货币，不映射）。
//! - 上层 `history/<session>/index.json` 只有 `conversations[]/current`
//!   注册表，不含用量，不作为扫描目标。
//! - 消息体在 conversation 目录 `messages/<id>.json`，其 `extra`（字符串
//!   内嵌 JSON）携带 `modelId/modelName/isHelperMessage/requestId`；仅按
//!   requests[].messages 引用做有界读取提取模型，不读正文。
//!
//! 同一请求可能随会话复制到多个 profile/workspace 树（本机已见同 session
//! hash 的多树副本），记录键只用请求 id 让单实例内幂等合并，不双计。
//! 偏离已核验形态（非 craft、非 complete、分桶不一致）跳过并记诊断；
//! 全零用量视为无模型调用，不入账。

use crate::adapters::framework::{
    DetectOutcome, DiscoverContext, DiscoveredRoot, RootBasis, ScanOutcome, ScanStatus, ScanTarget,
    StoredScanState,
};
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, FieldQuality, Lifecycle, ModelAttribution,
    RecordKind, TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const EXT_FORMAT: &str = "codebuddy-extension-requests-doc1";
pub const EXT_PARSER: &str = "codebuddy-extension-requests-doc1";
const MAX_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;
const MAX_INDEX_BYTES: u64 = 8 * 1024 * 1024;
const MAX_MESSAGE_FILES: usize = 2_000;
const MAX_MESSAGE_BYTES: u64 = 1024 * 1024;
const DATA_DIR: &str = "CodeBuddyExtension";
const STORE_DIR: &str = "Data";

/// 整文件游标：offset 记录扫描时的文件长度，供运行器做无变化短路；
/// 文件被改写（首/尾指纹变化）触发整文件重扫，事件按键幂等 upsert。
#[derive(serde::Serialize, serde::Deserialize)]
struct ExtCursor {
    generation: i64,
    offset: u64,
}

fn diag(code: &str, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.into(),
        field: None,
        position: None,
        message: message.into(),
    }
}

fn token(obj: &serde_json::Value, name: &str) -> Result<Option<i64>, ()> {
    match obj.get(name) {
        None => Ok(None),
        Some(value) => value
            .as_i64()
            .filter(|n| (0..=MAX_TOKEN).contains(n))
            .map(Some)
            .ok_or(()),
    }
}

fn started_at_ms(value: &serde_json::Value) -> Option<i64> {
    let ms = value.get("startedAt")?.as_i64()?;
    (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
        .contains(&ms)
        .then_some(ms)
}

/// 是否为 conversation 级 index.json（发现与分派共用）：
/// `history/<session>/<conversation>/index.json`；上层 session 级
/// index.json（conversations 注册表）不在其列。
pub fn is_conversation_index_path(path: &Path) -> bool {
    path.file_name().and_then(|s| s.to_str()) == Some("index.json")
        && path
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            == Some("history")
}

fn data_roots(ctx: &DiscoverContext) -> Vec<(PathBuf, RootBasis)> {
    let mut candidates = Vec::new();
    if let Some(local) = ctx.env.get("LOCALAPPDATA").filter(|s| !s.trim().is_empty()) {
        candidates.push((
            PathBuf::from(local).join(DATA_DIR).join(STORE_DIR),
            RootBasis::DefaultHome,
        ));
    }
    for manual in &ctx.manual_roots {
        // 手工根只接受扩展存储的 Data 目录，防止与其他产品目录互抢。
        if manual.file_name().and_then(|s| s.to_str()) == Some(STORE_DIR)
            && manual
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
                == Some(DATA_DIR)
        {
            candidates.push((manual.clone(), RootBasis::Manual));
        }
    }
    candidates
}

/// 有界枚举 conversation 级 index.json：
/// Data/<profile>/<host>/<workspace>/history/<session>/<conversation>/。
pub fn discover(ctx: &DiscoverContext) -> Vec<DiscoveredRoot> {
    let mut seen = std::collections::BTreeSet::new();
    data_roots(ctx)
        .into_iter()
        .filter_map(|(root, basis)| {
            if !seen.insert(root.clone()) {
                return None;
            }
            let files = crate::adapters::framework::enumerate_files_bounded(&root, 6, &|p| {
                is_conversation_index_path(p)
            });
            (!files.is_empty()).then_some(DiscoveredRoot { root, basis, files })
        })
        .collect()
}

pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    let Some(head) = crate::adapters::framework::read_detect_head(path, 64 * 1024)? else {
        return Ok(DetectOutcome::Pending);
    };
    if head.is_empty() {
        return Ok(DetectOutcome::Pending);
    }
    let text = String::from_utf8_lossy(&head);
    if text.trim_start().starts_with('{')
        && text.contains("\"requests\"")
        && text.contains("\"messages\"")
    {
        Ok(DetectOutcome::Supported {
            format: EXT_FORMAT.into(),
            format_version: Some(EXT_FORMAT.into()),
            basis: VersionBasis::KnownVersion,
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "missing CodeBuddy extension conversation fingerprint".into(),
        })
    }
}

/// 模型归属：按 requests[].messages 引用的消息 id 在 conversation 目录下
/// 做有界读取，只取 extra 内嵌 JSON 的 modelId/modelName，不读正文。
fn message_models(
    conversation_dir: &Path,
    referenced: &BTreeSet<String>,
) -> BTreeMap<String, String> {
    let mut models = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(conversation_dir.join("messages")) else {
        return models;
    };
    let mut scanned = 0usize;
    for entry in entries.flatten() {
        if scanned >= MAX_MESSAGE_FILES {
            break;
        }
        let path = entry.path();
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if !referenced.contains(stem) {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.len() > MAX_MESSAGE_BYTES {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = crate::adapters::run_policy::json_from_str::<serde_json::Value>(&text)
        else {
            continue;
        };
        let model = value
            .get("extra")
            .and_then(|v| v.as_str())
            .and_then(|s| crate::adapters::run_policy::json_from_str::<serde_json::Value>(s).ok())
            .and_then(|extra| {
                extra
                    .get("modelId")
                    .or_else(|| extra.get("modelName"))
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
            });
        if let Some(model) = model {
            models.insert(stem.to_string(), model);
        }
        scanned += 1;
    }
    models
}

/// 请求内多个模型时无法把聚合用量归到单一模型；取最后一条有模型信息的
/// 消息（能力声明记录该限制），全部缺失保持 unknown。
fn request_model(ids: &[String], models: &BTreeMap<String, String>) -> Option<String> {
    ids.iter().rev().find_map(|id| models.get(id).cloned())
}

/// 解析单个请求；Ok(None) = 无模型调用（全零），Err = 偏离已核验形态。
#[allow(clippy::too_many_arguments)]
fn parse_request(
    value: &serde_json::Value,
    conversation: &str,
    models: &BTreeMap<String, String>,
    instance_id: &str,
    generation: i64,
    host: Option<&str>,
    now_ms: i64,
) -> Result<Option<EventInput>, &'static str> {
    if value.get("type").and_then(|v| v.as_str()) != Some("craft") {
        return Err("unverified request type");
    }
    if value
        .get("state")
        .and_then(|v| v.as_str())
        .is_some_and(|state| state != "complete")
    {
        return Err("unverified request state");
    }
    let request_id = value
        .get("id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or("missing request identity")?;
    let usage = value.get("usage").ok_or("missing usage")?;
    if !usage.is_object() {
        return Err("usage is not an object");
    }
    let input = token(usage, "inputTokens").map_err(|_| "invalid input token count")?;
    let output = token(usage, "outputTokens").map_err(|_| "invalid output token count")?;
    let cache_read = token(usage, "cacheTokens").map_err(|_| "invalid cache read token count")?;
    let cache_write =
        token(usage, "cachedWriteTokens").map_err(|_| "invalid cache write token count")?;
    let cache_miss =
        token(usage, "cachedMissTokens").map_err(|_| "invalid cache miss token count")?;
    let reported_total = token(usage, "totalTokens").map_err(|_| "invalid total token count")?;
    // 全零/缺失：请求没有发生模型调用（本机样本中的空回合），不入账。
    if input.unwrap_or(0) == 0
        && output.unwrap_or(0) == 0
        && cache_miss.unwrap_or(0) == 0
        && reported_total.unwrap_or(0) == 0
    {
        return Ok(None);
    }
    let occurred_ms = started_at_ms(value).ok_or("missing or implausible startedAt")?;
    // inputTokens 含缓存读：miss + read 可核对时 uncached 取 miss；
    // 分桶不可核对时保留 input_total，uncached 置未知。
    let uncached = match (cache_miss, input, cache_read) {
        (Some(miss), Some(total), Some(hit)) if miss + hit == total => Some(miss),
        (None, Some(_), None) => None,
        (Some(miss), Some(total), None) if miss <= total => Some(miss),
        _ => return Err("cache buckets disagree"),
    };
    let input_total = input.or_else(|| {
        cache_miss
            .zip(cache_read)
            .and_then(|(a, b)| a.checked_add(b))
    });
    if input_total.is_none() && reported_total.is_none() {
        return Err("no input evidence");
    }
    let derived_total = input_total.zip(output).and_then(|(a, b)| a.checked_add(b));
    if reported_total
        .zip(derived_total)
        .is_some_and(|(reported, derived)| reported != derived)
    {
        return Err("reported total disagrees with input plus output");
    }
    let total = derived_total.or(reported_total);
    let message_ids: Vec<String> = value
        .get("messages")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let model = request_model(&message_ids, models);
    let mapped = crate::adapters::usage_map::finish(
        crate::domain::TokenUsage {
            input_uncached: uncached,
            input_cache_read: cache_read,
            input_cache_write: cache_write,
            input_total,
            output_total: output,
            output_reasoning: None,
            total_tokens: total,
            source_total: reported_total,
        },
        crate::domain::TokenQuality {
            input_uncached: uncached
                .map(|_| FieldQuality::Reported)
                .unwrap_or(FieldQuality::Unknown),
            input_cache_read: cache_read
                .map(|_| FieldQuality::Reported)
                .unwrap_or(FieldQuality::Unknown),
            input_cache_write: cache_write
                .map(|_| FieldQuality::Reported)
                .unwrap_or(FieldQuality::Unknown),
            input_total: input_total
                .map(|_| FieldQuality::Reported)
                .unwrap_or(FieldQuality::Unknown),
            output_total: output
                .map(|_| FieldQuality::Reported)
                .unwrap_or(FieldQuality::Unknown),
            output_reasoning: FieldQuality::Unknown,
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
        source_instance_id: instance_id.to_string(),
        // 只按请求 id（UUID）构造键：同一请求复制到其他 profile/workspace
        // 树时 conversation id 会变，键保持稳定才能幂等合并不双计。
        source_record_key: format!("codebuddy:request:{request_id}"),
        record_kind: RecordKind::ModelCall,
        schema_version: EXT_FORMAT.into(),
        parser_version: EXT_PARSER.into(),
        parse_basis: Some(VersionBasis::KnownVersion),
        origin_call_id: None,
        attempt_id: None,
        session_id: Some(conversation.into()),
        parent_session_id: None,
        host_application: host.map(str::to_string),
        agent: "codebuddy".into(),
        call_category: CallCategory::Primary,
        occurred_at_ms: occurred_ms,
        observed_at_ms: Some(now_ms),
        source_time: Some(occurred_ms.to_string()),
        time_basis: TimeBasis::SourceStart,
        interval_start_ms: Some(occurred_ms),
        interval_end_ms: None,
        provider_id: None,
        model_raw: model,
        model_canonical: None,
        model_attribution: ModelAttribution::RequestField,
        usage: mapped.usage,
        quality: mapped.quality,
        lifecycle: Lifecycle::Final,
        source_revision: Some(generation),
        error_status: None,
        duration_ms: None,
        ttft_ms: None,
        attribution_status: AttributionStatus::Verified,
        exclusion_reason: None,
        cost: None,
    }))
}

pub fn scan(
    target: &ScanTarget,
    _stored: &StoredScanState,
    _limits: &crate::adapters::framework::ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    if target.probe.len > MAX_INDEX_BYTES {
        return Ok(ScanOutcome {
            status: ScanStatus::LineTooLong,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "index_exceeds_cap",
                "conversation index exceeds the 8 MiB cap",
            )],
            lines_read: 0,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "degraded".into(),
        });
    }
    use std::io::Read as _;
    let mut text = String::new();
    crate::adapters::run_policy::checked_file(&target.path)?
        .take(MAX_INDEX_BYTES + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > MAX_INDEX_BYTES {
        return Err(CoreError::Validation(
            "conversation index exceeds the 8 MiB cap".into(),
        ));
    }
    let value: serde_json::Value = crate::adapters::run_policy::json_from_str(&text)
        .map_err(|_| CoreError::Validation("conversation index is not JSON".into()))?;
    let requests = value
        .get("requests")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if !value.get("requests").is_some_and(|v| v.is_array()) {
        return Ok(ScanOutcome {
            status: ScanStatus::Complete,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "index_shape_unexpected",
                "conversation index missing requests array",
            )],
            lines_read: 0,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "degraded".into(),
        });
    }
    let conversation_dir = target
        .path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let conversation = conversation_dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string();
    // host 组件（VSCode/CodeBuddyIDE）只作展示性归属，不参与身份：
    // index.json 的五级祖先依次是 conversation/session/history/workspace/host。
    let host = target
        .path
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str());
    let referenced: BTreeSet<String> = requests
        .iter()
        .filter_map(|r| r.get("messages")?.as_array().cloned())
        .flatten()
        .filter_map(|m| m.as_str().map(str::to_string))
        .collect();
    let models = message_models(&conversation_dir, &referenced);
    let mut events = Vec::new();
    let mut diagnostics = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut records_seen = 0u64;
    for request in &requests {
        crate::adapters::run_policy::check()?;
        records_seen += 1;
        match parse_request(
            request,
            &conversation,
            &models,
            &target.instance_id,
            target.generation,
            host,
            now_ms,
        ) {
            Ok(Some(event)) => {
                // 同文件重复请求 id 只入账一次（跨树复制由记录键幂等合并）。
                if seen.insert(event.source_record_key.clone()) {
                    events.push(event);
                } else {
                    diagnostics.push(diag(
                        "duplicate_request",
                        "duplicate request id in conversation index",
                    ));
                }
            }
            Ok(None) => {}
            Err(reason) => {
                diagnostics.push(diag("codebuddy_request_invalid", reason));
            }
        }
    }
    Ok(ScanOutcome {
        status: ScanStatus::Complete,
        cursor: Some(serde_json::to_value(ExtCursor {
            generation: target.generation,
            offset: target.probe.len,
        })?),
        parse_context: None,
        events,
        aggregates: Vec::new(),
        diagnostics,
        lines_read: 1,
        records_seen,
        reconciliations: Vec::new(),
        health: "active".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::framework::{ScanLimits, SourceAdapter};
    use crate::adapters::jsonl::probe_file;
    use crate::adapters::tencent_buddy_wire::BuddyAdapter;
    use std::path::PathBuf;

    /// 测试目录：<base>/<name>-pid 充当 LOCALAPPDATA，其下建 CodeBuddyExtension/Data。
    fn store_base(name: &str) -> PathBuf {
        let base = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../build/codebuddy-ext-tests")
            .join(format!("{name}-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        base
    }

    fn store_root(name: &str) -> PathBuf {
        let root = store_base(name).join("CodeBuddyExtension").join("Data");
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    /// history/<session>/<conversation> 目录（conversation 级 index 所在地）。
    fn conversation_dir(root: &Path, host: &str) -> PathBuf {
        let dir = root
            .join("profile-1")
            .join(host)
            .join("workspace-1")
            .join("history")
            .join("sess-a")
            .join("conv-1");
        std::fs::create_dir_all(dir.join("messages")).unwrap();
        std::fs::create_dir_all(dir.parent().unwrap()).unwrap();
        dir
    }

    fn message_file(dir: &Path, id: &str, model: &str) {
        let extra =
            serde_json::json!({ "modelId": model, "isHelperMessage": false, "requestId": "r1" })
                .to_string();
        std::fs::write(
            dir.join("messages").join(format!("{id}.json")),
            serde_json::json!({ "id": id, "role": "assistant", "message": "<redacted>", "extra": extra, "createdAt": "2026-09-30T04:54:48.621Z" }).to_string(),
        )
        .unwrap();
    }

    fn scan_target(path: PathBuf) -> ScanTarget {
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

    /// LOCALAPPDATA 指向 Data 根的上两级（CodeBuddyExtension 的父目录）。
    fn ctx_with_localappdata(data_root: &Path) -> DiscoverContext {
        let base = data_root
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_default();
        DiscoverContext {
            home_dir: None,
            env: [("LOCALAPPDATA".to_string(), base.to_string_lossy().into())]
                .into_iter()
                .collect(),
            manual_roots: vec![],
        }
    }

    #[test]
    fn parses_requests_with_model_and_skips_zero_and_unverified() {
        let root = store_root("parse");
        let conversation = conversation_dir(&root, "VSCode");
        message_file(&conversation, "m1", "kimi-k3-1");
        let index = serde_json::json!({
            "messages": [],
            "requests": [
                { "id": "r1", "type": "craft", "state": "complete", "startedAt": 1790743555715_i64,
                  "messages": ["m1"],
                  "usage": { "inputTokens": 64339, "outputTokens": 1822, "totalTokens": 66161,
                             "lastTokens": 23694, "cacheTokens": 53504, "cachedWriteTokens": 0,
                             "cachedMissTokens": 10835, "credit": 12.29 } },
                { "id": "r2", "type": "craft", "state": "complete", "startedAt": 1790744198350_i64,
                  "messages": ["m0"],
                  "usage": { "inputTokens": 0, "outputTokens": 0, "totalTokens": 0,
                             "lastTokens": 0, "cacheTokens": 0, "cachedWriteTokens": 0,
                             "cachedMissTokens": 0, "credit": 0 } },
                { "id": "r3", "type": "craft", "state": "aborted", "startedAt": 1790744300000_i64,
                  "messages": [],
                  "usage": { "inputTokens": 5, "outputTokens": 1, "totalTokens": 6 } },
                { "id": "r4", "type": "craft", "state": "complete", "startedAt": 1790744400000_i64,
                  "messages": [],
                  "usage": { "inputTokens": 10, "outputTokens": 2, "totalTokens": 99 } }
            ]
        });
        let path = conversation.join("index.json");
        std::fs::write(&path, index.to_string()).unwrap();
        let outcome = scan(
            &scan_target(path),
            &StoredScanState::default(),
            &ScanLimits::default(),
            1_800_000_000_000,
        )
        .unwrap();
        assert_eq!(outcome.events.len(), 1, "{:?}", outcome.diagnostics);
        let event = &outcome.events[0];
        assert_eq!(event.source_record_key, "codebuddy:request:r1");
        assert_eq!(event.session_id.as_deref(), Some("conv-1"));
        assert_eq!(event.model_raw.as_deref(), Some("kimi-k3-1"));
        assert_eq!(event.host_application.as_deref(), Some("VSCode"));
        assert_eq!(event.usage.input_total, Some(64339));
        assert_eq!(event.usage.input_cache_read, Some(53504));
        assert_eq!(event.usage.input_uncached, Some(10835));
        assert_eq!(event.usage.total_tokens, Some(66161));
        assert_eq!(event.time_basis, TimeBasis::SourceStart);
        assert_eq!(outcome.records_seen, 4);
        assert_eq!(outcome.diagnostics.len(), 2);
        std::fs::remove_dir_all(root.parent().unwrap().parent().unwrap()).unwrap();
    }

    #[test]
    fn duplicate_request_ids_collapse_per_file() {
        let root = store_root("dup");
        let conversation = conversation_dir(&root, "CodeBuddyIDE");
        let index = serde_json::json!({
            "messages": [],
            "requests": [
                { "id": "r1", "type": "craft", "state": "complete", "startedAt": 1790743555715_i64,
                  "messages": [], "usage": { "inputTokens": 7, "outputTokens": 3, "totalTokens": 10 } },
                { "id": "r1", "type": "craft", "state": "complete", "startedAt": 1790743555715_i64,
                  "messages": [], "usage": { "inputTokens": 7, "outputTokens": 3, "totalTokens": 10 } }
            ]
        });
        let path = conversation.join("index.json");
        std::fs::write(&path, index.to_string()).unwrap();
        let outcome = scan(
            &scan_target(path),
            &StoredScanState::default(),
            &ScanLimits::default(),
            1_800_000_000_000,
        )
        .unwrap();
        assert_eq!(outcome.events.len(), 1);
        assert_eq!(
            outcome.events[0].host_application.as_deref(),
            Some("CodeBuddyIDE")
        );
        assert_eq!(outcome.diagnostics.len(), 1);
        std::fs::remove_dir_all(root.parent().unwrap().parent().unwrap()).unwrap();
    }

    #[test]
    fn same_request_in_two_trees_shares_record_key() {
        // 扩展存储可能把同一会话复制到多个 profile/workspace 树
        // （conversation id 随之不同）；单一根实例 + 请求 id 记录键
        // 让 ingest 幂等合并，不双计。
        let root = store_root("crosstree");
        for (profile, workspace, conversation_id) in [
            ("profile-1", "workspace-1", "conv-a"),
            ("default", "workspace-2", "conv-b"),
        ] {
            let conversation = root
                .join(profile)
                .join("VSCode")
                .join(workspace)
                .join("history")
                .join("sess-shared")
                .join(conversation_id);
            std::fs::create_dir_all(&conversation).unwrap();
            let index = serde_json::json!({
                "messages": [],
                "requests": [
                    { "id": "r-shared", "type": "craft", "state": "complete", "startedAt": 1790743555715_i64,
                      "messages": [], "usage": { "inputTokens": 7, "outputTokens": 3, "totalTokens": 10 } }
                ]
            });
            std::fs::write(conversation.join("index.json"), index.to_string()).unwrap();
        }
        let roots = discover(&ctx_with_localappdata(&root));
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].files.len(), 2);
        let mut keys = BTreeSet::new();
        for file in &roots[0].files {
            let outcome = scan(
                &scan_target(file.clone()),
                &StoredScanState::default(),
                &ScanLimits::default(),
                1_800_000_000_000,
            )
            .unwrap();
            assert_eq!(outcome.events.len(), 1);
            keys.insert(outcome.events[0].source_record_key.clone());
        }
        assert_eq!(
            keys.len(),
            1,
            "same request across trees must share one key"
        );
        std::fs::remove_dir_all(root.parent().unwrap().parent().unwrap()).unwrap();
    }

    /// CLI JSONL 载体与扩展载体并存时分派正确；WorkBuddy 不发现扩展存储。
    #[test]
    fn composite_dispatch_and_workbuddy_guard() {
        let root = store_root("dispatch");
        let conversation = conversation_dir(&root, "VSCode");
        message_file(&conversation, "m1", "kimi-k3-1");
        std::fs::write(
            conversation.join("index.json"),
            r#"{"messages":[],"requests":[]}"#,
        )
        .unwrap();
        // 上层 session 级 index（conversations 注册表）不得被发现/探测为扩展载体。
        let session_index = conversation.parent().unwrap().join("index.json");
        std::fs::write(&session_index, r#"{"conversations":[],"current":"c"}"#).unwrap();
        let home = root.parent().unwrap().parent().unwrap().join("home");
        let cli_projects = home.join(".codebuddy").join("projects").join("demo");
        std::fs::create_dir_all(&cli_projects).unwrap();
        let cli_file = cli_projects.join("s.jsonl");
        std::fs::write(
            &cli_file,
            "{\"id\":\"a\",\"timestamp\":1780000000100,\"type\":\"message\",\"role\":\"assistant\",\"sessionId\":\"s\",\"message\":{\"usage\":{\"input_tokens\":12,\"output_tokens\":2}}}\n",
        )
        .unwrap();
        let mut ctx = ctx_with_localappdata(&root);
        ctx.home_dir = Some(home);
        let ext_roots = discover(&ctx);
        assert_eq!(ext_roots.len(), 1);
        assert_eq!(
            ext_roots[0].files.len(),
            1,
            "session-level registry index must not be discovered"
        );
        assert!(ext_roots[0].root.ends_with("CodeBuddyExtension/Data"));
        let composite = crate::adapters::codebuddy::CodeBuddyAdapter::new();
        let all_roots = composite.discover(&ctx);
        assert_eq!(all_roots.len(), 2, "{all_roots:?}");
        assert!(matches!(
            composite.detect(&ext_roots[0].files[0]).unwrap(),
            DetectOutcome::Supported { .. }
        ));
        assert!(matches!(
            detect(&session_index).unwrap(),
            DetectOutcome::UnknownFormat { .. }
        ));
        assert!(matches!(
            composite.detect(&cli_file).unwrap(),
            DetectOutcome::Supported { .. }
        ));
        let work = BuddyAdapter::<true>::new();
        assert!(work.discover(&ctx).is_empty());
        std::fs::remove_dir_all(root.parent().unwrap().parent().unwrap()).unwrap();
    }
}
