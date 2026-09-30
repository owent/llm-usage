//! CodeBuddy 的两个本机载体：CLI `~/.codebuddy/projects` 会话 JSONL（复用
//! tencent_buddy_wire，与 WorkBuddy 保持独立来源身份）和 IDE/插件
//! CodeBuddyExtension 存储的请求级 index.json（extension_store）。
pub mod detect;
pub mod extension_store;
pub mod versions;

use crate::adapters::framework::{
    Availability, CapabilityTable, DetectOutcome, DiscoverContext, DiscoveredRoot, ScanLimits,
    ScanOutcome, ScanTarget, SourceAdapter, StoredScanState,
};
use crate::error::CoreError;
use std::path::Path;

pub use crate::adapters::tencent_buddy_wire::BuddyAdapter as BuddyAdapterGeneric;

/// CLI JSONL 载体与扩展存储载体的复合入口。两个载体共用 agent 身份
/// `codebuddy`，但实例、文件与解析按路径形状分派，互不重叠。
pub struct CodeBuddyAdapter {
    cli: BuddyAdapterGeneric<false>,
}

impl Default for CodeBuddyAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeBuddyAdapter {
    pub fn new() -> Self {
        Self {
            cli: BuddyAdapterGeneric::new(),
        }
    }
}

impl SourceAdapter for CodeBuddyAdapter {
    fn adapter_id(&self) -> &'static str {
        self.cli.adapter_id()
    }
    fn agent(&self) -> &'static str {
        self.cli.agent()
    }

    fn discover(&self, ctx: &DiscoverContext) -> Vec<DiscoveredRoot> {
        let mut roots = self.cli.discover(ctx);
        roots.extend(extension_store::discover(ctx));
        roots
    }

    fn instance_id(&self, root: &DiscoveredRoot) -> String {
        self.cli.instance_id(root)
    }

    fn detect(&self, path: &Path) -> Result<DetectOutcome, CoreError> {
        if extension_store::is_conversation_index_path(path) {
            extension_store::detect(path)
        } else {
            self.cli.detect(path)
        }
    }

    fn scan(
        &self,
        target: &ScanTarget,
        stored: &StoredScanState,
        limits: &ScanLimits,
        now_ms: i64,
    ) -> Result<ScanOutcome, CoreError> {
        if extension_store::is_conversation_index_path(&target.path) {
            extension_store::scan(target, stored, limits, now_ms)
        } else {
            self.cli.scan(target, stored, limits, now_ms)
        }
    }

    fn capability(&self) -> CapabilityTable {
        let mut table = self.cli.capability();
        // 复合能力：登记扩展存储的发现根、格式锚点与限制（证据为本机只读核验）。
        if let Some(default_roots) = table
            .discovery
            .get_mut("default_roots")
            .and_then(|v| v.as_array_mut())
        {
            default_roots.push("%LOCALAPPDATA%\\CodeBuddyExtension\\Data".into());
        }
        if let Some(env_override) = table.discovery.get_mut("env_override") {
            *env_override = serde_json::json!(["CODEBUDDY_CONFIG_DIR", "LOCALAPPDATA"]);
        }
        table
            .supported_versions
            .push(extension_store::EXT_FORMAT.into());
        let ext_evidence =
            "2026-09-30 本机只读核验（CodeBuddyExtension 会话 index.json requests[].usage）";
        for name in [
            "tokens",
            "cache_read",
            "cache_write",
            "per_request_calls",
            "model",
            "time",
        ] {
            if let Some(field) = table.fields.get_mut(name) {
                if let Some(note) = field.get_mut("note") {
                    *note = serde_json::json!(format!(
                        "{}；扩展存储为请求级聚合 usage（inputTokens=cacheTokens+cachedMissTokens）",
                        note.as_str().unwrap_or_default()
                    ));
                }
                if let Some(availability) = field.get_mut("availability") {
                    *availability = serde_json::json!(Availability::Partial(ext_evidence.into()));
                }
            }
        }
        table.surfaces = vec!["cli".into(), "desktop".into()];
        table.maintenance.as_object_mut().map(|m| {
            m.insert(
                "parser_version".into(),
                serde_json::json!("tencent-buddy-session-doc1;codebuddy-extension-requests-doc1"),
            )
        });
        table.limitations.push(
            "扩展存储 usage 是请求级聚合（一次用户回合可含多次模型调用），非逐调用明细".into(),
        );
        table
            .limitations
            .push("扩展存储 credit 为平台积分（非货币），当前不映射成本".into());
        table
            .limitations
            .push("请求内多模型时取最后一条有模型信息的消息归属".into());
        table
    }
}
