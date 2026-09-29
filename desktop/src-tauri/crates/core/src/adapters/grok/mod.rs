//! Grok Build 适配器（xAI，闭源；独立目录合同）。载体：
//! `$GROK_HOME`（默认 ~/.grok）下 sessions/&lt;workspace&gt;/&lt;session&gt;/updates.jsonl
//! 的显式 usage 块；累计差额/压缩补偿/子代理 PID 归因等推断路径不采纳。

pub mod detect;
pub mod versions;

pub use detect::GROK_FORMAT;
pub use versions::updates_doc1;
pub use versions::{GROK_FORMAT_VERSION, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// Grok 适配器（无状态）。
pub struct GrokAdapter;

impl Default for GrokAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl GrokAdapter {
    pub fn new() -> Self {
        GrokAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for GrokAdapter {
    fn adapter_id(&self) -> &'static str {
        "grok"
    }

    fn agent(&self) -> &'static str {
        "grok-build"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        if let Some(home) = ctx.env.get("GROK_HOME").filter(|v| !v.trim().is_empty()) {
            roots.push((
                std::path::PathBuf::from(home.trim()).join("sessions"),
                RootBasis::EnvOverride("GROK_HOME".to_string()),
            ));
        }
        if let Some(home) = &ctx.home_dir {
            roots.push((home.join(".grok").join("sessions"), RootBasis::DefaultHome));
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out = Vec::new();
        let mut seen: std::collections::BTreeSet<std::path::PathBuf> =
            std::collections::BTreeSet::new();
        for (root, basis) in roots {
            let files = crate::adapters::framework::enumerate_files_bounded(&root, 2, &|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n == "updates.jsonl")
                    .unwrap_or(false)
            });
            if !files.is_empty() && seen.insert(root.clone()) {
                out.push(DiscoveredRoot { root, basis, files });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "grok@{}",
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
        versions::updates_doc1::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let note =
            "第三方解析器证据（tokscale 1d9a939）；闭源 + 本机未安装，待真实样本核验".to_string();
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(
                Availability::Partial(note.clone()),
                "params.update.usage 显式块五桶（多别名组）；只采显式计数",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Partial(note.clone()),
                "cachedReadTokens 等别名组",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial(note.clone()),
                "cacheCreationTokens 等别名组",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Partial(note.clone()),
                "每个 usage 块一次调用证据",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial(note.clone()),
                "_meta.modelId 等别名或 modelUsage 单键",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial(note.clone()),
                "agentTimestampMs/timestamp 别名（数字毫秒或 RFC3339）",
            ),
        );
        fields.insert(
            "cost".into(),
            field(Availability::Unavailable("无费用字段".into()), "无"),
        );
        fields.insert(
            "latency".into(),
            field(Availability::Unavailable("无延迟字段".into()), "无"),
        );
        CapabilityTable {
            adapter_id: "grok".to_string(),
            product: "Grok Build（xAI）".to_string(),
            surfaces: vec!["cli".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["~/.grok/sessions"],
                "env_override": "GROK_HOME",
                "manual_roots": "sessions 目录或其父目录",
                "bounded": true,
                "pattern": "sessions/<workspace>/<session>/updates.jsonl（深度 2；workspace 目录名 percent-encoded 不解码，仅枚举）",
                "profile": "无",
            }),
            detection: serde_json::json!({
                "magic": "JSONL 含 jsonrpc + update/sessionUpdate 指纹",
                "version_field": "无；文档级锚点 grok-updates-doc-1",
                "registry": "adapters/grok/versions 注册表（唯一条目）",
                "fail_closed": true,
                "unknown_version": "格式偏离 fail closed",
            }),
            fields,
            lifecycle: serde_json::json!({
                "explicit_usage_only": "只读 params.update.usage；_meta.totalTokens 累计差值、signals.json 补偿、unified.jsonl/PID 归因、events/summary 汇总均不采纳（估算路径）",
                "inclusion": "第三方证据称 inputTokens 含 cachedRead、outputTokens 含 reasoning：并列报告不派生总量，无双计",
            }),
            incremental: serde_json::json!({
                "cursor": "JSONL 字节偏移；键含行号（eventId 不唯一）",
            }),
            dedup: serde_json::json!({
                "primary": "grok:<session>:usage:<行号>:<eventId>",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "子代理调用模型归属复杂（第三方证据）：未归属的保持当前模型字段，不猜",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::updates_doc1::GROK_PARSER_VERSION,
                "format_evidence": "tokscale 固定提交 1d9a939 sessions/grok.rs",
                "evidence_level": "third-party-parser（无本机样本）",
                "upgrade_policy": "真实样本后核验别名组与包含关系",
            }),
            scheduling: serde_json::json!({ "entry": "统一 run_adapter_scan" }),
            limitations: vec![
                "闭源 + 本机未安装：文档级实现".into(),
                "估算路径全部不采纳：累计差额/压缩补偿/字节分摊/PID 归因".into(),
                "缺时间戳别名的行不入账".into(),
                "provider 字段未见（第三方证据）：provider 维度未知".into(),
            ],
        }
    }
}
