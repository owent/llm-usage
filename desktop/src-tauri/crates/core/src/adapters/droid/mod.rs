//! Droid 适配器（Factory.ai，闭源；独立目录约定）。载体：
//! `~/.factory/sessions/<uuid>.settings.json` 的 tokenUsage 会话级累计快照
//! （IntervalAggregate；转录按字节分摊属估计不采纳）。

pub mod detect;
pub mod versions;

pub use detect::DROID_FORMAT;
pub use versions::settings_doc1;
pub use versions::{DROID_FORMAT_VERSION, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// Droid 适配器（无状态）。
pub struct DroidAdapter;

impl Default for DroidAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl DroidAdapter {
    pub fn new() -> Self {
        DroidAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for DroidAdapter {
    fn adapter_id(&self) -> &'static str {
        "droid"
    }

    fn agent(&self) -> &'static str {
        "droid"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        if let Some(home) = &ctx.home_dir {
            roots.push((
                home.join(".factory").join("sessions"),
                RootBasis::DefaultHome,
            ));
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out = Vec::new();
        let mut seen: std::collections::BTreeSet<std::path::PathBuf> =
            std::collections::BTreeSet::new();
        for (root, basis) in roots {
            let files = crate::adapters::framework::enumerate_files_bounded(&root, 1, &|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.ends_with(".settings.json"))
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
            "droid@{}",
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
        versions::settings_doc1::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let note =
            "第三方解析器证据（tokscale 1d9a939）；闭源 + 本机未安装，待真实样本核验".to_string();
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(Availability::Partial(note.clone()), "tokenUsage 五桶（inputTokens/outputTokens/cacheReadTokens/cacheCreationTokens/thinkingTokens）会话级累计"),
        );
        fields.insert(
            "cache_read".into(),
            field(Availability::Partial(note.clone()), "cacheReadTokens"),
        );
        fields.insert(
            "cache_write".into(),
            field(Availability::Partial(note.clone()), "cacheCreationTokens"),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Unavailable(
                    "累计快照；转录不含 token，按字节分摊属估计不采纳".into(),
                ),
                "无",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial(note.clone()),
                "settings.model（会话级单模型）",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial(note.clone()),
                "端点 = max(mtime, providerLockTimestamp)；起始未知",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Unavailable("无费用字段（第三方证据）".into()),
                "无",
            ),
        );
        fields.insert(
            "latency".into(),
            field(Availability::Unavailable("无逐次时间".into()), "无"),
        );
        CapabilityTable {
            adapter_id: "droid".to_string(),
            product: "Droid（Factory.ai）".to_string(),
            surfaces: vec!["cli".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["~/.factory/sessions"],
                "env_override": null,
                "manual_roots": "sessions 目录",
                "bounded": true,
                "pattern": "sessions/<uuid>.settings.json（深度 1）；同名 <uuid>.jsonl 转录不读（无 token）",
                "profile": "无",
            }),
            detection: serde_json::json!({
                "magic": "JSON 含 tokenUsage + model/providerLock 指纹",
                "version_field": "无；文档级锚点 droid-settings-doc-1",
                "registry": "adapters/droid/versions 注册表（唯一条目）",
                "fail_closed": true,
                "unknown_version": "格式偏离 fail closed",
            }),
            fields,
            lifecycle: serde_json::json!({
                "cumulative_snapshot": "每会话一条 session 级聚合；不展开伪造逐次",
                "no_allocation": "tokscale 的按回复字节分摊是估计路径，不采纳",
            }),
            incremental: serde_json::json!({
                "cursor": "整写 JSON：已消费字节数；source_revision=端点毫秒",
            }),
            dedup: serde_json::json!({ "primary": "droid:<uuid>" }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "转录中的失败/未计量调用不可见（settings 只存累计）",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::settings_doc1::DROID_PARSER_VERSION,
                "format_evidence": "tokscale 固定提交 1d9a939 sessions/droid.rs",
                "evidence_level": "third-party-parser（无本机样本）",
                "upgrade_policy": "真实样本后核验字段形状",
            }),
            scheduling: serde_json::json!({ "entry": "统一 run_adapter_scan" }),
            limitations: vec![
                "会话级累计：无逐次/延迟/费用".into(),
                "闭源 + 本机未安装：文档级实现".into(),
                "区间起始未知（无 created 字段证据）".into(),
            ],
        }
    }
}
