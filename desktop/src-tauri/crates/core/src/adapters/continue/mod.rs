//! Continue 适配器（continuedev/continue；独立目录约定）。载体：
//! `~/.continue/sessions/<uuid>.json` 顶层 `usage`（**仅 CLI 写入**的会话累计
//! 真实 API 值；GUI 会话无此字段；devdata.sqlite 是估算不采纳）。

pub mod detect;
pub mod versions;

pub use detect::CONTINUE_FORMAT;
pub use versions::session_usage_v1;
pub use versions::{CONTINUE_FORMAT_VERSION, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// Continue 适配器（无状态）。
pub struct ContinueAdapter;

impl Default for ContinueAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ContinueAdapter {
    pub fn new() -> Self {
        ContinueAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for ContinueAdapter {
    fn adapter_id(&self) -> &'static str {
        "continue"
    }

    fn agent(&self) -> &'static str {
        "continue"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        if let Some(dir) = ctx
            .env
            .get("CONTINUE_GLOBAL_DIR")
            .filter(|v| !v.trim().is_empty())
        {
            roots.push((
                std::path::PathBuf::from(dir.trim()).join("sessions"),
                RootBasis::EnvOverride("CONTINUE_GLOBAL_DIR".to_string()),
            ));
        }
        if let Some(home) = &ctx.home_dir {
            roots.push((
                home.join(".continue").join("sessions"),
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
            // <uuid>.json（深度 1）；sessions.json 是索引不读。
            let files = crate::adapters::framework::enumerate_files_bounded(&root, 1, &|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("json"))
                    && p.file_name().and_then(|n| n.to_str()) != Some("sessions.json")
            });
            if !files.is_empty() && seen.insert(root.clone()) {
                out.push(DiscoveredRoot { root, basis, files });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "continue@{}",
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
        versions::session_usage_v1::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let note = "官方源码（5522c6f）与官方 CLI 1.5.47 的真实流式本地模型会话；其他场景仍待核验"
            .to_string();
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(
                Availability::Partial(note.clone()),
                "顶层 usage（promptTokens/completionTokens + promptTokensDetails.cachedTokens/cacheWriteTokens）：CLI 会话累计真实 API 值",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Partial(note.clone()),
                "promptTokensDetails.cachedTokens；产品默认零不能证明已报告，零保留 unknown，正数采信",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial(note.clone()),
                "promptTokensDetails.cacheWriteTokens；产品默认零不能证明已报告，零保留 unknown，正数采信",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Unavailable(
                    "会话累计，无逐 turn 明细（API 逐请求值只在内存）".into(),
                ),
                "无",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Unavailable("会话文件无逐请求模型（chatModelTitle 是显示名）".into()),
                "无",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial(note.clone()),
                "区间端点=文件 mtime（Uncertain）；sessions.json 索引时间格式随端而变不读",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Unavailable(
                    "totalCost 来源未核验（CLI 累计）且会话级聚合无 cost 载体：不映射".into(),
                ),
                "不映射",
            ),
        );
        fields.insert(
            "latency".into(),
            field(Availability::Unavailable("无".into()), "无"),
        );
        CapabilityTable {
            adapter_id: "continue".to_string(),
            product: "Continue（CLI/VS Code/JetBrains；usage 仅 CLI 载体）".to_string(),
            surfaces: vec!["cli".into(), "vscode".into(), "jetbrains".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["~/.continue/sessions"],
                "env_override": "CONTINUE_GLOBAL_DIR（JetBrains 端硬编码 ~/.continue 不读该变量）",
                "manual_roots": "sessions 目录",
                "bounded": true,
                "pattern": "sessions/<uuidv4>.json（深度 1；sessions.json 索引/logs/index/dev_data 均不读）",
                "profile": "无",
            }),
            detection: serde_json::json!({
                "magic": "JSON 对象含 sessionId + history",
                "version_field": "无；文档级锚点 continue-session-usage-1",
                "registry": "adapters/continue/versions 注册表（唯一条目）",
                "fail_closed": true,
                "unknown_version": "格式偏离 fail closed",
            }),
            fields,
            lifecycle: serde_json::json!({
                "cli_only_usage": "GUI（VS Code/JetBrains）会话无 usage 字段 = 无数据，不补零；devdata.sqlite tokenizer 估算不采纳",
                "cumulative": "每会话一条 session 级聚合，upsert 幂等（revision=mtime）",
            }),
            incremental: serde_json::json!({
                "cursor": "整写 JSON 字节游标 + 无变化短路",
            }),
            dedup: serde_json::json!({
                "primary": "continue:session:<sessionId>",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "GUI 会话逐次用量只在内存；promptLogs 无 token 字段",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::session_usage_v1::CONTINUE_PARSER_VERSION,
                "format_evidence": "continuedev/continue 5522c6f + @continuedev/cli 1.5.47 固定官方包；真实本地模型/API/会话输入 1471、输出 2 一致",
                "evidence_level": "real-local（1.5.47 CLI readonly、本地流式响应；产品初始化缓存零保留 unknown）",
                "upgrade_policy": "产品版本不在会话文件，仍用格式锚点；完整旧摘要仅允许缓存默认零改未知，保留源修订与其他字段仲裁",
            }),
            scheduling: serde_json::json!({ "entry": "统一 run_adapter_scan" }),
            limitations: vec![
                "仅 CLI 会话有用量：GUI 会话不采集（无字段，非缺 bug）".into(),
                "会话级累计：无逐次/模型/延迟；totalCost 不映射".into(),
                "Hub/远程会话已移除（官方 'Hub integration removed'）：本地文件即全集".into(),
            ],
        }
    }

    fn prior_aggregate_hashes(
        &self,
        input: &crate::aggregates::SourceAggregateInput,
    ) -> Vec<String> {
        versions::session_usage_v1::prior_cache_hashes(input)
    }
}
