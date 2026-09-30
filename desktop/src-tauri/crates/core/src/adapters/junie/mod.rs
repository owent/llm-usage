//! Junie CLI 适配器（JetBrains，独立目录合同）。载体：
//! `~/.junie/sessions/<session-id>/events.jsonl` 的
//! LlmResponseMetadataEvent.modelUsage[]（逐轮、含 cost/延迟/provider）。
//! JetBrains AI Assistant IDE 插件本体仍缺证留 F1，本适配器只覆盖 CLI。

pub mod detect;
pub mod versions;

pub use detect::JUNIE_FORMAT;
pub use versions::events_doc1;
pub use versions::{JUNIE_FORMAT_VERSION, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// Junie 适配器（无状态）。
pub struct JunieAdapter;

impl Default for JunieAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl JunieAdapter {
    pub fn new() -> Self {
        JunieAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for JunieAdapter {
    fn adapter_id(&self) -> &'static str {
        "junie"
    }

    fn agent(&self) -> &'static str {
        "junie"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        if let Some(home) = &ctx.home_dir {
            roots.push((home.join(".junie").join("sessions"), RootBasis::DefaultHome));
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out = Vec::new();
        let mut seen: std::collections::BTreeSet<std::path::PathBuf> =
            std::collections::BTreeSet::new();
        for (root, basis) in roots {
            // 手工根可为 sessions 目录、某会话目录或 events.jsonl 所在目录。
            let base = if root.file_name().and_then(|n| n.to_str()) == Some("sessions") {
                root.clone()
            } else if root.join("events.jsonl").is_file() {
                root.parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or(root.clone())
            } else {
                root
            };
            let files = crate::adapters::framework::enumerate_files_bounded(&base, 2, &|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n == "events.jsonl")
                    .unwrap_or(false)
            });
            if !files.is_empty() && seen.insert(base.clone()) {
                out.push(DiscoveredRoot {
                    root: base,
                    basis,
                    files,
                });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "junie@{}",
            crate::adapters::framework::normalize_path(&root.root)
        )
    }

    fn detect(
        &self,
        path: &std::path::Path,
    ) -> Result<crate::adapters::framework::DetectOutcome, crate::error::CoreError> {
        detect::detect(path)
    }

    fn scan(
        &self,
        target: &crate::adapters::framework::ScanTarget,
        stored: &crate::adapters::framework::StoredScanState,
        limits: &crate::adapters::framework::ScanLimits,
        now_ms: i64,
    ) -> Result<crate::adapters::framework::ScanOutcome, crate::error::CoreError> {
        versions::events_doc1::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let note = "第三方解析器证据（tokscale 1d9a939）；闭源 + 本机未安装（2026-09-29 盘点），待真实样本核验".to_string();
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(
                Availability::Partial(note.clone()),
                "modelUsage[] 五桶（多别名组：inputTokens|input 等）；包含关系未知不派生总量",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Partial(note.clone()),
                "cacheInputTokens|cacheReadInputTokens|cacheRead",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial(note.clone()),
                "cacheCreateTokens|cacheCreationInputTokens|cacheWrite",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Partial(note.clone()),
                "每次 LLM 调用一行 modelUsage（数组每元素一事件）",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial(note.clone()),
                "modelUsage[].model + provider",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial(note.clone()),
                "timestampMs 是响应结束时刻；time 字段为延迟 ms（在场时补 interval_start = 结束−延迟）",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Partial(note.clone()),
                "cost（f64 USD，在场即 provider-reported，micro-USD 存储）",
            ),
        );
        fields.insert(
            "latency".into(),
            field(
                Availability::Partial(note.clone()),
                "time 字段（ms）→ duration_ms",
            ),
        );
        CapabilityTable {
            adapter_id: "junie".to_string(),
            product: "Junie CLI（JetBrains）".to_string(),
            surfaces: vec!["cli".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["~/.junie/sessions"],
                "env_override": null,
                "manual_roots": "sessions 目录、会话目录或 events.jsonl 所在目录",
                "bounded": true,
                "pattern": "sessions/<session-id>/events.jsonl（深度 2）",
                "profile": "无",
            }),
            detection: serde_json::json!({
                "magic": "事件指纹在头 64 KiB；已确认事件日志但无用量指纹时分块搜索至 4 MiB，仍无则 Pending 重探（不误报 UnknownFormat）",
                "version_field": "无；文档级锚点 junie-events-doc-1（tokscale 证据）",
                "registry": "adapters/junie/versions 注册表（唯一条目）",
                "fail_closed": true,
                "unknown_version": "格式偏离 fail closed，不走版本回退",
            }),
            fields,
            lifecycle: serde_json::json!({
                "per_call": "每次 LLM 调用一行；同数组多元素按索引区分",
                "skipped": "AgentStateUpdated/CurrentStatus/PatchCreated 等无 usage 事件跳过",
            }),
            incremental: serde_json::json!({
                "cursor": "JSONL 字节偏移；事件键含数组索引与桶值（字节级重放折叠，tokscale 同款）",
            }),
            dedup: serde_json::json!({
                "primary": "junie:<session>:<ts>:<model>:<五桶>:<cost>:<index>",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "events.jsonl 未记录的调用不可见；无采样证据",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::events_doc1::JUNIE_PARSER_VERSION,
                "format_evidence": "tokscale 固定提交 1d9a939 sessions/junie.rs（闭源产品第三方逆向证据）",
                "evidence_level": "third-party-parser（无官方源码、无本机样本）",
                "upgrade_policy": "真实样本出现后按实际字段核验并升级证据等级",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan",
                "incremental_cost": "追加式字节偏移增量",
            }),
            limitations: vec![
                "闭源 + 本机未安装：文档级实现，待真实脱敏 fixture 核验".into(),
                "字段证据来自第三方解析器：别名组按证据全收，包含关系未知不推导".into(),
                "缺 timestampMs 的行不入账（会话目录名兜底时间未采用，避免推造时间）".into(),
                "JetBrains AI Assistant IDE 插件仍留 F1".into(),
            ],
        }
    }
}
