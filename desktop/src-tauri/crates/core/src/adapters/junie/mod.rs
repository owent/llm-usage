//! Junie CLI adapter (JetBrains, independent directory). Reads
//! ~/.junie/sessions/<session-id>/events.jsonl, specifically
//! LlmResponseMetadataEvent.modelUsage[] per round; initialized zeros remain unknown.
//! JetBrains AI Assistant IDE usage remains unverified in F1; this adapter covers CLI only.

pub mod detect;
pub mod versions;

pub use detect::JUNIE_FORMAT;
pub use versions::events_doc1;
pub use versions::{JUNIE_FORMAT_VERSION, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// Stateless Junie adapter.
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
        if let Some(home) = ctx.env.get("JUNIE_HOME").filter(|v| !v.trim().is_empty()) {
            roots.push((
                std::path::PathBuf::from(home.trim()).join("sessions"),
                RootBasis::EnvOverride("JUNIE_HOME".to_string()),
            ));
        }
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
            // Manual roots may be sessions, one session directory or the events.jsonl directory.
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
        let note = "官方 26.9.22（3419.29）发行包字节码及隔离真实 OpenAICompletion/CLI/事件七次调用已核对；任务最终失败，其他版本/API 运行未验收".to_string();
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(
                Availability::Partial(note.clone()),
                "inputTokens 为非缓存输入，正分项保留；默认零未知，无 API/版本字段不派生完整总量",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Partial(note.clone()),
                "cacheInputTokens|cacheReadInputTokens|cacheRead；零可能是客户端默认值，保留未知",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial(note.clone()),
                "cacheCreateTokens|cacheCreationInputTokens|cacheWrite；实际 OpenAICompletion 不报告缓存写，默认零未知",
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
                "modelUsage[].model；真实载体未报告 provider，不按模型名推断",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial(note.clone()),
                "timestampMs 是响应结束时刻；正 time 补 interval_start = 结束−延迟，默认零不认证耗时",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Partial(note.clone()),
                "正 cost 为客户端 Estimated（micro-USD；USD 沿用文档证据）；calcTokenCost 依赖客户端价目，零未知，付费渠道未验收",
            ),
        );
        fields.insert(
            "latency".into(),
            field(
                Availability::Partial(note.clone()),
                "正 time（ms）→ duration_ms；零为未知",
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
                "env_override": "JUNIE_HOME/sessions（官方配置及实际运行核验）",
                "manual_roots": "sessions 目录、会话目录或 events.jsonl 所在目录",
                "bounded": true,
                "pattern": "sessions/<session-id>/events.jsonl（深度 2）",
                "profile": "无",
            }),
            detection: serde_json::json!({
                "magic": "事件指纹在头 64 KiB；已确认事件日志但无用量指纹时分块搜索至 4 MiB，仍无则 Pending 重探（不误报 UnknownFormat）",
                "version_field": "无；格式锚点 junie-events-doc-1，26.9.22 实际载体/发行包核验；不认证同目录其他版本",
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
                "primary": "junie:<session>:<ts>:<model>:<原始五桶>:<原始cost>:<行号>:<index>；修正规则不改变原键",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "events.jsonl 未记录的调用不可见；无采样证据",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::events_doc1::JUNIE_PARSER_VERSION,
                "format_evidence": "官方 3419.29 发行包 LlmMetadata/UsageTokens/API 转换/calcTokenCost + API/CLI/events.jsonl 真实七次调用；旧别名沿用 tokscale 1d9a939",
                "evidence_level": "real-local（失败任务的真实非空调用；无付费渠道验收）",
                "upgrade_policy": "doc1→2 完整旧 canonical/legacy 摘要允许归一输入/零默认/时间/费用质量纠正；原键/修订不变，冲突历史保留，事件/汇总/游标同事务",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan",
                "incremental_cost": "追加式字节偏移增量",
            }),
            limitations: vec![
                "真实任务最终因小模型结构化响应失败；已发生的七次调用已核验，不认证任务成功、其他版本/API 或完整调用覆盖".into(),
                "原生字段无产品版本/API/provider；正分项保留，零默认及完整总量未知，旧别名仍为文档证据".into(),
                "缺 timestampMs 的行不入账（会话目录名兜底时间未采用，避免推造时间）".into(),
                "JetBrains AI Assistant IDE 插件仍留 F1".into(),
            ],
        }
    }
}
