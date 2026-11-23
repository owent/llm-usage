//! Local Antigravity generation databases under ~/.gemini/antigravity[-cli]/conversations.
//! Desktop 2.34.0 pairs generation usage with completed planner responses in SQLite.

pub mod detect;
pub mod versions;

pub use detect::ANTIGRAVITY_FORMAT;
pub use versions::gen_metadata_v1;
pub use versions::{ANTIGRAVITY_FORMAT_VERSION, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// Stateless Antigravity adapter.
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
        // GEMINI_CLI_HOME overrides the Gemini root (tokscale clients.rs:744–756).
        // Record EnvOverride for an environment root and DefaultHome for the default directory.
        let env_hit = ctx
            .env
            .get("GEMINI_CLI_HOME")
            .filter(|v| !v.trim().is_empty());
        let gemini = match env_hit {
            Some(v) => Some((
                std::path::PathBuf::from(v.trim()),
                RootBasis::EnvOverride("GEMINI_CLI_HOME".to_string()),
            )),
            None => ctx
                .home_dir
                .as_ref()
                .map(|h| (h.join(".gemini"), RootBasis::DefaultHome)),
        };
        if let Some((root, basis)) = gemini {
            roots.push((
                root.join("antigravity-cli").join("conversations"),
                basis.clone(),
            ));
            roots.push((root.join("antigravity").join("conversations"), basis));
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
        let note = "桌面版 2.34.0 使用本机样本核对，旧版 CLI 布局依据第三方字段检查".to_string();
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(
                Availability::Partial(note.clone()),
                "桌面版 gen_metadata #1.#4 提供未缓存输入、缓存读取、总输出和推理；旧格式使用顶层 #4",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(Availability::Partial(note.clone()), "usage #5"),
        );
        fields.insert(
            "cache_write".into(),
            field(Availability::Partial(note.clone()), "仅桌面布局的正值"),
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
                "chatModel #19 为原始模型 ID，省略 gemini-default 路由标签",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial(note.clone()),
                "桌面版按 generation index 与 response ID 匹配已完成的规划器响应；旧布局使用 #9.#4",
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
            product: "Antigravity（Google，桌面/CLI conversations 库）".to_string(),
            surfaces: vec!["desktop".into(), "cli".into(), "vscode".into()],
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
                "version_field": "按行识别 protobuf 布局",
                "registry": "adapters/antigravity/versions",
                "fail_closed": true,
                "unknown_version": "布局不符、缺少可核对时间或 protobuf 畸形的记录保留诊断",
            }),
            fields,
            lifecycle: serde_json::json!({
                "per_generation": "每 generation 一行 usage；responseId 文件内去重",
                "input_semantics": "桌面版输入桶独立，缺失字段保持未知；旧版映射按原规则处理",
            }),
            incremental: serde_json::json!({
                "cursor": "按 generation index 分批，升级后重读旧位置；未匹配到完成记录的行再检查",
            }),
            dedup: serde_json::json!({
                "primary": "antigravity:<file_identity>:<responseId>",
                "cross_surface": "CLI/扩展库分实例；与 Gemini CLI 目录互不推断",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "覆盖本地 SQLite 中有记录的调用",
            }),
            maintenance: serde_json::json!({
                "format_evidence": "Windows 桌面 2.34.0 本机 SQLite；旧版参考 tokscale 1d9a939",
            }),
            scheduling: serde_json::json!({ "entry": "统一 run_adapter_scan" }),
            limitations: vec![
                "旧 CLI 布局要求 #9.#4 时间戳；桌面版用量要求关联一条已完成规划器响应。".into(),
            ],
        }
    }
}
