//! GitHub Copilot CLI 适配器（独立目录合同）。载体：
//! `~/.copilot/session-store.db` 的 `assistant_usage_events`（逐 turn 全字段；
//! schema_version=8 真实数据核对 2026-09-29）。OTel 路径（events.jsonl/OTLP）
//! 是需启用的补充载体，见能力表边界。

pub mod common;
pub mod detect;
pub mod versions;

pub use common::{map_copilot, CopilotUsage};
pub use detect::COPILOT_FORMAT;
pub use versions::usage_events_v8;
pub use versions::{COPILOT_FORMAT_VERSION, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// Copilot CLI 适配器（无状态）。
pub struct CopilotAdapter;

impl Default for CopilotAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CopilotAdapter {
    pub fn new() -> Self {
        CopilotAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for CopilotAdapter {
    fn adapter_id(&self) -> &'static str {
        "copilot"
    }

    fn agent(&self) -> &'static str {
        "copilot-cli"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        if let Some(home) = &ctx.home_dir {
            roots.push((home.join(".copilot"), RootBasis::DefaultHome));
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out = Vec::new();
        let mut seen: std::collections::BTreeSet<std::path::PathBuf> =
            std::collections::BTreeSet::new();
        for (root, basis) in roots {
            // 手工根可为 .copilot 目录或 session-store.db 文件本身。
            let db = if root.join("session-store.db").is_file() {
                root.join("session-store.db")
            } else if root.is_file() {
                root.clone()
            } else {
                continue;
            };
            if seen.insert(db.clone()) {
                out.push(DiscoveredRoot {
                    root: db.parent().map(|p| p.to_path_buf()).unwrap_or(root),
                    basis,
                    files: vec![db],
                });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "copilot@{}",
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
        versions::usage_events_v8::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(
                Availability::Available,
                "input/output/cache_read/cache_write/reasoning 五列（input=未缓存+读+写，uncached 派生）",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(Availability::Available, "cache_read_tokens"),
        );
        fields.insert(
            "cache_write".into(),
            field(Availability::Available, "cache_write_tokens"),
        );
        fields.insert(
            "per_request_calls".into(),
            field(Availability::Available, "逐 turn 事件（assistant 消息级）"),
        );
        fields.insert(
            "model".into(),
            field(Availability::Available, "model 列（如 claude-opus-4.8）"),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Available,
                "created_at（ISO8601，事件完成时刻）",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Unavailable("request_multiplier=27.0 是 premium 付费倍率、total_nano_aiu 是 nano AIU 计量：非 USD 不映射".into()),
                "不映射",
            ),
        );
        fields.insert(
            "latency".into(),
            field(
                Availability::Available,
                "duration_ms + time_to_first_token_ms",
            ),
        );
        CapabilityTable {
            adapter_id: "copilot".to_string(),
            product: "GitHub Copilot CLI".to_string(),
            surfaces: vec!["cli".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["~/.copilot/session-store.db"],
                "env_override": null,
                "manual_roots": ".copilot 目录或 session-store.db 文件",
                "bounded": true,
                "pattern": "单库 assistant_usage_events（WAL；只读 + busy 暂存副本）；events.jsonl 无逐次 token 不采集",
                "profile": "无",
            }),
            detection: serde_json::json!({
                "magic": "assistant_usage_events 表 + 必需列集",
                "version_field": "schema_version 表（8=KnownVersion；其他/缺失 LatestFallback）",
                "registry": "adapters/copilot/versions 注册表（唯一条目）",
                "fail_closed": true,
                "unknown_version": "未知 schema_version 走 latest 兼容尝试（列集探测层校验）",
            }),
            fields,
            lifecycle: serde_json::json!({
                "per_turn": "append-only 事件表；premium 倍率与 nano AIU 不入 token 统计",
                "storage_migration": "2026-09-25 曾全树消失（疑升级迁移）：来源消失时保留旧结果并告警，不回退双计",
            }),
            incremental: serde_json::json!({
                "cursor": "已处理最大 id（append-only）；单轮 50k 行",
            }),
            dedup: serde_json::json!({
                "primary": "copilot:usage:<id>",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "VS Code 侧 copilot-chat 库无 usage 表（本机实测）：VS Code 用量需遥测启用路径",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::usage_events_v8::COPILOT_PARSER_VERSION,
                "format_evidence": "M0 m0-agent-fixtures.md（1.0.73 取证）+ 本机 schema_version=8 真实数据复核（tests/fixtures/copilot）",
                "evidence_level": "real-data（36 行核对，语义 0 违例）",
                "upgrade_policy": "schema_version 变化走 latest 兼容尝试，真实样本后锚定新版本",
            }),
            scheduling: serde_json::json!({ "entry": "统一 run_adapter_scan" }),
            limitations: vec![
                "OTel 路径（events.jsonl/OTLP 导出）为需启用补充载体：未启用前不可见".into(),
                "reasoning 与 output 包含关系未证：并列报告不并入派生总量".into(),
                "premium 倍率/nano AIU 是计量单位：不入 token 统计（额度类另行展示）".into(),
            ],
        }
    }
}
