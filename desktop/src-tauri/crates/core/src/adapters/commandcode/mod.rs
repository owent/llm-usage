//! Command Code 适配器（独立目录合同）。载体：
//! `~/.commandcode/projects/<slug>/*.jsonl` v3 树形（当前路径口径；fork 复制
//! 按 entry id+timestamp 跨文件去重）。证据来自官方 npm 分发物 1.69.0。

pub mod detect;
pub mod versions;

pub use detect::COMMANDCODE_FORMAT;
pub use versions::tree_v3;
pub use versions::{COMMANDCODE_FORMAT_VERSION, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// Command Code 适配器（无状态）。
pub struct CommandCodeAdapter;

impl Default for CommandCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandCodeAdapter {
    pub fn new() -> Self {
        CommandCodeAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for CommandCodeAdapter {
    fn adapter_id(&self) -> &'static str {
        "commandcode"
    }

    fn agent(&self) -> &'static str {
        "command-code"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        if let Some(home) = &ctx.home_dir {
            roots.push((
                home.join(".commandcode").join("projects"),
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
            // projects/<slug>/<session>.jsonl（深度 2；排除 checkpoints/prompts/v2.bak）。
            let files = crate::adapters::framework::enumerate_files_bounded(&root, 2, &|p| {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                name.ends_with(".jsonl")
                    && !name.contains(".checkpoints.")
                    && !name.contains(".prompts.")
                    && !name.ends_with(".v2.bak")
            });
            if !files.is_empty() && seen.insert(root.clone()) {
                out.push(DiscoveredRoot { root, basis, files });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "commandcode@{}",
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
        versions::tree_v3::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let note = "官方 npm 分发物证据（command-code@1.69.0 dist/cli.mjs 逐行核对）；闭源，本机未安装，待真实样本核验".to_string();
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(
                Availability::Partial(note.clone()),
                "usage 四桶（完成请求必写、?? 0 归一化 ⇒ 全零=已报告零；缺字段=未知）；inputTokens 含 cache（官方成本公式证实）",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Partial(note.clone()),
                "cacheReadTokens（inputTokens 子集）",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial(note.clone()),
                "cacheWriteTokens（inputTokens 子集；1h 桶 ⊆ 5m 桶不另计）",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Partial(note.clone()),
                "链上 assistant entry 一次（当前路径口径）；子代理 usage 不写盘",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial(note.clone()),
                "entry.model（provider/model）或链上最近 model_change",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial(note.clone()),
                "entry.timestamp（ISO）",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Partial(note.clone()),
                "costUsd（本地费率估算 ⇒ Estimated micro-USD；费率缺失时无字段）",
            ),
        );
        fields.insert(
            "latency".into(),
            field(Availability::Unavailable("无延迟字段".into()), "无"),
        );
        CapabilityTable {
            adapter_id: "commandcode".to_string(),
            product: "Command Code（CommandCodeAI，闭源）".to_string(),
            surfaces: vec!["cli".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["~/.commandcode/projects"],
                "env_override": null,
                "manual_roots": "projects 目录或 slug 目录",
                "bounded": true,
                "pattern": "projects/<slugify(cwd)>/<sessionId>.jsonl（深度 2；排除 checkpoints/prompts/v2.bak）；注意上游 HOME 优先于 USERPROFILE",
                "profile": "无（config.json 是偏好配置不参与解析）",
            }),
            detection: serde_json::json!({
                "magic": "首行 {type:\"session\",version,id} 头",
                "version_field": "header.version：3=KnownVersion；>3 上游拒开；<3 产品打开时自动迁移（fail closed 不读）",
                "registry": "adapters/commandcode/versions 注册表（唯一条目）",
                "fail_closed": true,
                "unknown_version": "v1/v2 旧形状按 UnsupportedVersion 处理",
            }),
            fields,
            lifecycle: serde_json::json!({
                "effective_path": "文件最后一条 entry 回溯 parentId 链（官方 buildSessionPath）；rewind 孤儿分支不计",
                "no_retraction": "rewind 后已入账事件保持（append-only 无墓碑；与 cline 删除语义一致）",
                "fork": "复制条目保留原 id+timestamp ⇒ 跨文件去重键 cmd:<entry id>:<timestamp>",
                "compaction": "链上压缩前条目仍计（真实调用）；firstKeptEntryId 只影响上下文不影响计费",
            }),
            incremental: serde_json::json!({
                "cursor": "整文件树重建（32 MiB 上限）：字节游标 + 无变化短路",
            }),
            dedup: serde_json::json!({
                "primary": "cmd:<entry id>:<timestamp>（8hex id 跨文件可碰撞，加时间戳）",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "中断/最终错误请求无 usage 字段（官方 ?? 0 只在完成事件写）；corrupted 行计入诊断不中断",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::tree_v3::COMMANDCODE_PARSER_VERSION,
                "format_evidence": "官方 npm 包 command-code@1.69.0 dist/cli.mjs（toSessionUsage/estimateSessionCostUsd/buildSessionPath/createBranchedSession/detectSessionFileVersion）",
                "evidence_level": "official-distribution（npm bundle；无本机样本）",
                "upgrade_policy": "按 npm 版本浮动：真实样本后随版本锚定",
            }),
            scheduling: serde_json::json!({ "entry": "统一 run_adapter_scan" }),
            limitations: vec![
                "闭源 + 本机未安装：文档级实现（npm 分发物证据）".into(),
                "rewind 不回溯撤账：孤儿分支首扫即不计，已入账事件保持".into(),
                "v1/v2 旧文件不读（产品打开时自动迁移后可读）".into(),
            ],
        }
    }
}
