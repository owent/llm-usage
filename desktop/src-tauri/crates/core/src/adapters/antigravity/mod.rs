//! Antigravity 适配器（Google，闭源；独立目录合同）。载体：
//! `~/.gemini/antigravity[-cli]/conversations/<uuid>.db` 的 gen_metadata
//! protobuf（逆向证据；无 #9.#4 时间戳的行 fail closed 不推造时间）。
//! IDE 主体用量走 language server：无本地载体证据，不实施。

pub mod detect;
pub mod versions;

pub use detect::ANTIGRAVITY_FORMAT;
pub use versions::gen_metadata_v1;
pub use versions::{ANTIGRAVITY_FORMAT_VERSION, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// Antigravity 适配器（无状态）。
pub struct AntigravityAdapter;

impl Default for AntigravityAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl AntigravityAdapter {
    pub fn new() -> Self {
        AntigravityAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for AntigravityAdapter {
    fn adapter_id(&self) -> &'static str {
        "antigravity"
    }

    fn agent(&self) -> &'static str {
        "antigravity"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        // gemini 根：GEMINI_CLI_HOME 覆盖（tokscale clients.rs:744-756）。
        let gemini_root = ctx
            .env
            .get("GEMINI_CLI_HOME")
            .filter(|v| !v.trim().is_empty())
            .map(|v| std::path::PathBuf::from(v.trim()))
            .or_else(|| ctx.home_dir.as_ref().map(|h| h.join(".gemini")));
        if let Some(root) = gemini_root {
            roots.push((
                root.join("antigravity-cli").join("conversations"),
                RootBasis::DefaultHome,
            ));
            roots.push((
                root.join("antigravity").join("conversations"),
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
                p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("db"))
            });
            if !files.is_empty() && seen.insert(root.clone()) {
                out.push(DiscoveredRoot { root, basis, files });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "antigravity@{}",
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
        versions::gen_metadata_v1::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let note = "第三方逆向证据（tokscale 1d9a939 antigravity_cli.rs）；闭源 + 本机未安装，protobuf 布局须逐版本锚定".to_string();
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(
                Availability::Partial(note.clone()),
                "gen_metadata #4 usage：input = #1 固定 system prompt + #2 新输入（逆向证据：固定提示计入计费输入）+ cacheRead/output/thinking",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(Availability::Partial(note.clone()), "usage #5"),
        );
        fields.insert(
            "cache_write".into(),
            field(Availability::Unavailable("逆向证据无该字段".into()), "无"),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Partial(note.clone()),
                "每 generation 一行（responseId 去重）",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial(note.clone()),
                "chatModel #19 responseModel（机器 id）；#21 显示名仅 join 不采；gemini-default 路由标签不采",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial(note.clone()),
                "agy 1.1.17 及更早的 #9.#4 Timestamp；1.1.18+ 布局变更的行无可靠时间戳，跳过不推造",
            ),
        );
        fields.insert(
            "cost".into(),
            field(Availability::Unavailable("无".into()), "无"),
        );
        fields.insert(
            "latency".into(),
            field(Availability::Unavailable("无".into()), "无"),
        );
        CapabilityTable {
            adapter_id: "antigravity".to_string(),
            product: "Antigravity（Google，CLI/扩展 conversations 库）".to_string(),
            surfaces: vec!["cli".into(), "vscode".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": [
                    "~/.gemini/antigravity-cli/conversations",
                    "~/.gemini/antigravity/conversations",
                ],
                "env_override": "GEMINI_CLI_HOME",
                "manual_roots": "conversations 目录",
                "bounded": true,
                "pattern": "conversations/<uuid>.db（深度 1；与 Gemini CLI 同根不同子目录，互不推断）",
                "profile": "无",
            }),
            detection: serde_json::json!({
                "magic": "SQLite + gen_metadata 表",
                "version_field": "无；文档级锚点 antigravity-gen-metadata-1（逆向布局）",
                "registry": "adapters/antigravity/versions 注册表（唯一条目）",
                "fail_closed": true,
                "unknown_version": "1.1.18+ 时间戳布局变更行跳过；protobuf 畸形行跳过；真实样本后逐版本锚定",
            }),
            fields,
            lifecycle: serde_json::json!({
                "per_generation": "每 generation 一行 usage；responseId 文件内去重",
                "input_semantics": "input = 固定 system prompt + 新输入（逆向证据）；cache_write 未知",
            }),
            incremental: serde_json::json!({
                "cursor": "offset 恒 0（≤50k 行整读）；事件键 antigravity:<file_identity>:<responseId|idx>",
            }),
            dedup: serde_json::json!({
                "primary": "antigravity:<file_identity>:<responseId>",
                "cross_surface": "CLI/扩展库分实例；与 Gemini CLI 目录互不推断",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "IDE language server 主体用量无本地载体证据：不实施，如实标注",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::gen_metadata_v1::ANTIGRAVITY_PARSER_VERSION,
                "format_evidence": "tokscale 1d9a939 sessions/antigravity_cli.rs（protobuf 字段号逆向 + 1.1.18 变更注记）",
                "evidence_level": "third-party-reverse-engineered（无本机样本；风险最高档）",
                "upgrade_policy": "官方发行产物/固定版本反推核对后逐版本锚定；布局漂移即 fail closed",
            }),
            scheduling: serde_json::json!({ "entry": "统一 run_adapter_scan" }),
            limitations: vec![
                "protobuf 布局为逆向结论：证据等级最低，真实 fixture 前按文档级交付".into(),
                "1.1.18+ 无 #9.#4 时间戳的行跳过（8 字节编码推断与 steps 回退不采纳）".into(),
                "IDE language server 用量载体未见证据：不实施".into(),
            ],
        }
    }
}
