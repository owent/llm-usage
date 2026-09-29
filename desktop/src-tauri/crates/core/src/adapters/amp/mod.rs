//! Amp 适配器（Sourcegraph，闭源；独立目录合同）。载体：
//! `~/.local/share/amp/threads/T-*.json` 的 usageLedger.events（主，有显式
//! 时间戳）+ messages[].usage（对照，无时间戳不入账）。credits 是计费
//! 单位非美元，不映射 cost（额度类）。

pub mod detect;
pub mod versions;

pub use detect::AMP_FORMAT;
pub use versions::threads_doc1;
pub use versions::{AMP_FORMAT_VERSION, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// Amp 适配器（无状态）。
pub struct AmpAdapter;

impl Default for AmpAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl AmpAdapter {
    pub fn new() -> Self {
        AmpAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for AmpAdapter {
    fn adapter_id(&self) -> &'static str {
        "amp"
    }

    fn agent(&self) -> &'static str {
        "amp"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        if let Some(home) = &ctx.home_dir {
            // tokscale PathRoot::XdgData：XDG_DATA_HOME 或 ~/.local/share。
            let xdg = ctx
                .env
                .get("XDG_DATA_HOME")
                .filter(|v| !v.trim().is_empty())
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| home.join(".local").join("share"));
            roots.push((xdg.join("amp").join("threads"), RootBasis::DefaultHome));
            if cfg!(windows) {
                // Windows 真实布局未见证据：LOCALAPPDATA 候选根，指纹过滤。
                if let Some(appdata) = ctx.env.get("LOCALAPPDATA") {
                    roots.push((
                        std::path::PathBuf::from(appdata)
                            .join("amp")
                            .join("threads"),
                        RootBasis::DefaultHome,
                    ));
                }
            }
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
                    .map(|n| n.starts_with("T-") && n.ends_with(".json"))
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
            "amp@{}",
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
        versions::threads_doc1::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let note =
            "第三方解析器证据（tokscale 1d9a939）；闭源 + 本机未安装，待真实样本核验".to_string();
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(Availability::Partial(note.clone()), "usageLedger.events[].tokens 四桶（input/output/cacheReadInputTokens/cacheCreationInputTokens），逐事件"),
        );
        fields.insert(
            "cache_read".into(),
            field(Availability::Partial(note.clone()), "cacheReadInputTokens"),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial(note.clone()),
                "cacheCreationInputTokens",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Partial(note.clone()),
                "每条 ledger 事件一次计量（operationType 区分用途）",
            ),
        );
        fields.insert(
            "model".into(),
            field(Availability::Partial(note.clone()), "events[].model"),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial(note.clone()),
                "events[].timestamp（RFC3339）",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Unavailable("credits 是 Amp 计费单位非美元（额度类），不映射".into()),
                "不映射",
            ),
        );
        fields.insert(
            "latency".into(),
            field(Availability::Unavailable("无延迟字段".into()), "无"),
        );
        CapabilityTable {
            adapter_id: "amp".to_string(),
            product: "Amp（Sourcegraph）".to_string(),
            surfaces: vec!["cli".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["~/.local/share/amp/threads（XdgData）", "Windows %LOCALAPPDATA%\\amp\\threads（候选，布局未见证据）"],
                "env_override": "XDG_DATA_HOME（间接）",
                "manual_roots": "threads 目录",
                "bounded": true,
                "pattern": "threads/T-*.json（深度 1）",
                "profile": "无",
            }),
            detection: serde_json::json!({
                "magic": "JSON 含 messages + usage/usageLedger + created 指纹",
                "version_field": "无；文档级锚点 amp-threads-doc-1",
                "registry": "adapters/amp/versions 注册表（唯一条目）",
                "fail_closed": true,
                "unknown_version": "schema 随版本滚动（第三方警示）：格式偏离 fail closed，真实样本后逐版本锚定",
            }),
            fields,
            lifecycle: serde_json::json!({
                "dual_carrier": "usageLedger（主，有 timestamp）与 messages[].usage（对照，无 timestamp）按 toMessageId==messageId 对账防双计",
                "no_time_fabrication": "无 ledger 对应的消息 usage 不入账（tokscale 的 created+messageId×1000 推造时间不采纳）",
            }),
            incremental: serde_json::json!({
                "cursor": "整写 JSON：已消费字节数；事件键 amp:<thread>:ledger:<index>",
            }),
            dedup: serde_json::json!({
                "primary": "amp:<threadId>:ledger:<数组索引>",
                "cross_carrier": "toMessageId 命中的消息 usage 不再入账",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "未落 ledger 的调用不可见；schema 滚动风险如实标注",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::threads_doc1::AMP_PARSER_VERSION,
                "format_evidence": "tokscale 固定提交 1d9a939 sessions/amp.rs",
                "evidence_level": "third-party-parser（无本机样本）",
                "upgrade_policy": "真实样本后逐版本锚定 schema",
            }),
            scheduling: serde_json::json!({ "entry": "统一 run_adapter_scan" }),
            limitations: vec![
                "闭源 + 本机未安装：文档级实现".into(),
                "messages[].usage 无时间戳：仅对账不入账（禁止推造时间）".into(),
                "credits 计费单位不映射 cost".into(),
                "Amp schema 随版本滚动（第三方证据警示）：偏离即 fail closed".into(),
            ],
        }
    }
}
