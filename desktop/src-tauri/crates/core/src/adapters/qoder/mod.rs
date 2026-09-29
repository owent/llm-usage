//! Qoder 适配器（阿里，前通义灵码；独立目录合同）。**前置取证阶段**：
//! 路径已证（`~/.qoder/projects/...`，QODER_CONFIG_DIR 重定向），用量落盘
//! 字段仍缺证（bundle schema 线索不作数；计量是云端 Credits）⇒ 只发现与
//! 识别，不解析（fail closed）。JetBrains 插件归 F1。

pub mod detect;
pub mod versions;

pub use detect::QODER_FORMAT;
pub use versions::probe_only;
pub use versions::{LATEST_IMPL_ID, QODER_FORMAT_VERSION, VERIFIED_VERSION_IMPLS};

/// Qoder 适配器（无状态）。
pub struct QoderAdapter;

impl Default for QoderAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl QoderAdapter {
    pub fn new() -> Self {
        QoderAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for QoderAdapter {
    fn adapter_id(&self) -> &'static str {
        "qoder"
    }

    fn agent(&self) -> &'static str {
        "qoder"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        if let Some(dir) = ctx
            .env
            .get("QODER_CONFIG_DIR")
            .filter(|v| !v.trim().is_empty())
        {
            roots.push((
                std::path::PathBuf::from(dir.trim()).join("projects"),
                RootBasis::EnvOverride("QODER_CONFIG_DIR".to_string()),
            ));
        }
        if let Some(home) = &ctx.home_dir {
            roots.push((home.join(".qoder").join("projects"), RootBasis::DefaultHome));
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out = Vec::new();
        let mut seen: std::collections::BTreeSet<std::path::PathBuf> =
            std::collections::BTreeSet::new();
        for (root, basis) in roots {
            // 会话 jsonl（深度 2）+ state.json（sessions/<id>/state.json 形态）。
            let files = crate::adapters::framework::enumerate_files_bounded(&root, 3, &|p| {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                name == "state.json"
                    || (name.ends_with(".jsonl")
                        && !name.contains("subagents")
                        && !name.contains("segments"))
            });
            if !files.is_empty() && seen.insert(root.clone()) {
                out.push(DiscoveredRoot { root, basis, files });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "qoder@{}",
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
        versions::probe_only::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let unverified = Availability::Unavailable(
            "用量落盘字段未证实（bundle schema 线索不作数；计量为云端 Credits）；待本机 fixture"
                .to_string(),
        );
        let mut fields = serde_json::Map::new();
        for key in [
            "tokens",
            "cache_read",
            "cache_write",
            "per_request_calls",
            "model",
            "time",
            "cost",
            "latency",
        ] {
            fields.insert(
                key.to_string(),
                serde_json::json!({ "availability": unverified.clone(), "note": "不解析" }),
            );
        }
        CapabilityTable {
            adapter_id: "qoder".to_string(),
            product: "Qoder CLI（阿里，前通义灵码；探针级接入）".to_string(),
            surfaces: vec!["cli".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["~/.qoder/projects"],
                "env_override": "QODER_CONFIG_DIR",
                "manual_roots": "projects 目录",
                "bounded": true,
                "pattern": "projects/<processed-project-path-name>/<session-id>.jsonl + <session-id>/state.json（官方文档已证路径）",
                "profile": "无",
            }),
            detection: serde_json::json!({
                "magic": "sessionId/session_id 或 modelRequests/compact_token_usage_json 宽指纹",
                "version_field": "无（LatestFallback：字段未验证）",
                "registry": "adapters/qoder/versions 注册表（探针条目）",
                "fail_closed": true,
                "unknown_version": "扫描层 fail closed：不解析任何用量字段",
            }),
            fields,
            lifecycle: serde_json::json!({
                "probe_only": "发现/识别/登记来源；用量解析待本机 fixture 证实 state.json 与会话 jsonl 实际字段",
                "credits": "云端 Credits 计量：即使证实也不属本机 token 载体",
            }),
            incremental: serde_json::json!({
                "cursor": "探针无解析增量",
            }),
            dedup: serde_json::json!({ "primary": "qoder:<file_identity>" }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "全部调用暂不可见（字段未证）",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::probe_only::QODER_PARSER_VERSION,
                "format_evidence": "docs.qoder.com/cli/sessions.md（路径）+ npm 1.1.64 bundle 解包（schema 线索，不作数）",
                "evidence_level": "path-verified / fields-unverified",
                "upgrade_policy": "本机安装 Qoder 后提取 <session-id>.jsonl 与 state.json 脱敏 fixture，逐字段证实后实现解析",
            }),
            scheduling: serde_json::json!({ "entry": "统一 run_adapter_scan" }),
            limitations: vec![
                "探针级接入：只发现与登记来源，不产出用量数据（禁止猜测）".into(),
                "取证途径：本机跑一轮会话后实测两文件（唯一可靠途径）".into(),
                "IDE Electron 载体（%APPDATA%/Qoder）仍缺证：连同 JetBrains 插件归 F1".into(),
            ],
        }
    }
}
