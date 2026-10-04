//! Crush 适配器（Charm；独立目录约定）。载体：全局 projects.json 注册表 →
//! 每项目 `.crush/crush.db` 的根会话 cost 累计（token 列是上下文快照不采）。

pub mod common;
pub mod detect;
pub mod versions;

pub use detect::CRUSH_FORMAT;
pub use versions::sessions_cost_v1;
pub use versions::{CRUSH_FORMAT_VERSION, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// Crush 适配器（无状态）。
pub struct CrushAdapter;

impl Default for CrushAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CrushAdapter {
    pub fn new() -> Self {
        CrushAdapter
    }
}

/// 解析 projects.json：{projects:[{path, data_dir, ...}]} → crush.db 候选列表。
/// data_dir 为绝对路径或相对项目 path（官方 config.load.go）。
fn project_dbs(registry_path: &std::path::Path) -> Vec<std::path::PathBuf> {
    let Ok(text) = std::fs::read_to_string(registry_path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    let Some(projects) = value.get("projects").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for project in projects {
        let path = project.get("path").and_then(|v| v.as_str());
        let data_dir = project.get("data_dir").and_then(|v| v.as_str());
        let (Some(path), Some(data_dir)) = (path, data_dir) else {
            continue;
        };
        let data = if std::path::Path::new(data_dir).is_absolute() {
            std::path::PathBuf::from(data_dir)
        } else {
            std::path::Path::new(path).join(data_dir)
        };
        let db = data.join("crush.db");
        if db.is_file() {
            out.push(db);
        }
    }
    out
}

impl crate::adapters::framework::SourceAdapter for CrushAdapter {
    fn adapter_id(&self) -> &'static str {
        "crush"
    }

    fn agent(&self) -> &'static str {
        "crush"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        // 注册表目录候选：CRUSH_GLOBAL_DATA → XDG → Windows LOCALAPPDATA。
        let mut registry_dirs: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        if let Some(dir) = ctx.env.get("CRUSH_GLOBAL_DATA").filter(|v| {
            // 相对路径只能相对宿主进程 cwd 解析，采集侧无法复现：只接受绝对路径
            // （与 GOOSE_PATH_ROOT 规则相同）。
            let t = v.trim();
            !t.is_empty() && std::path::Path::new(t).is_absolute()
        }) {
            registry_dirs.push((
                std::path::PathBuf::from(dir.trim()),
                RootBasis::EnvOverride("CRUSH_GLOBAL_DATA".to_string()),
            ));
        }
        if let Some(home) = &ctx.home_dir {
            let xdg = ctx
                .env
                .get("XDG_DATA_HOME")
                .filter(|v| !v.trim().is_empty())
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| home.join(".local").join("share"));
            registry_dirs.push((xdg.join("crush"), RootBasis::DefaultHome));
            if let Some(appdata) = ctx.env.get("LOCALAPPDATA") {
                registry_dirs.push((
                    std::path::PathBuf::from(appdata).join("crush"),
                    RootBasis::DefaultHome,
                ));
            }
        }
        let mut out = Vec::new();
        let mut seen: std::collections::BTreeSet<std::path::PathBuf> =
            std::collections::BTreeSet::new();
        for (dir, basis) in registry_dirs {
            let registry = dir.join("projects.json");
            if !registry.is_file() {
                continue;
            }
            for db in project_dbs(&registry) {
                // 实例身份用项目库路径（同注册表下多项目去重）。
                if seen.insert(db.clone()) {
                    out.push(DiscoveredRoot {
                        root: db
                            .parent()
                            .map(|p| p.to_path_buf())
                            .unwrap_or_else(|| db.clone()),
                        basis: basis.clone(),
                        files: vec![db],
                    });
                }
            }
        }
        // 手工根：crush.db 文件、其 data_dir 或 projects.json 所在目录。
        for manual in &ctx.manual_roots {
            let candidates = if manual.is_file() {
                if manual.file_name().and_then(|n| n.to_str()) == Some("projects.json") {
                    project_dbs(manual)
                } else {
                    vec![manual.clone()]
                }
            } else {
                let direct = manual.join("crush.db");
                if direct.is_file() {
                    vec![direct]
                } else if manual.join("projects.json").is_file() {
                    project_dbs(&manual.join("projects.json"))
                } else {
                    Vec::new()
                }
            };
            for db in candidates {
                if seen.insert(db.clone()) {
                    out.push(DiscoveredRoot {
                        root: db
                            .parent()
                            .map(|p| p.to_path_buf())
                            .unwrap_or_else(|| db.clone()),
                        basis: RootBasis::Manual,
                        files: vec![db],
                    });
                }
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "crush@{}",
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
        versions::sessions_cost_v1::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let note =
            "官方源码证据（charmbracelet/crush 1f3827b）；本机未安装，待真实样本核验".to_string();
        let token_unavailable = "sessions.prompt_tokens/completion_tokens 是最近 step 上下文规模快照（SET 覆盖、摘要后重置、标题请求另加）——不是用量，一律不采（官方源码 agent.go:2060-2086）";
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(
                Availability::Unavailable(token_unavailable.to_string()),
                "无用量载体",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Unavailable(token_unavailable.to_string()),
                "无",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Unavailable(token_unavailable.to_string()),
                "无",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Unavailable(
                    "messages 表无 token 列；message_count 是触发器维护的行数".into(),
                ),
                "无",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Unavailable(
                    "会话行无模型列（模型在 messages 表且与 cost 无逐行对应）".into(),
                ),
                "无",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial(note.clone()),
                "会话 created_at..updated_at（Unix 秒，cost 观测区间）",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Partial(note.clone()),
                "根会话 cost 累计（parent_session_id IS NULL 防子会话回卷双计；费率自算 ⇒ Estimated micro-USD）",
            ),
        );
        fields.insert(
            "latency".into(),
            field(Availability::Unavailable("无".into()), "无"),
        );
        CapabilityTable {
            adapter_id: "crush".to_string(),
            product: "Crush（Charm）".to_string(),
            surfaces: vec!["cli".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": [
                    "$CRUSH_GLOBAL_DATA/projects.json（env）",
                    "~/.local/share/crush/projects.json（XDG）",
                    "Windows %LOCALAPPDATA%/crush/projects.json",
                ],
                "env_override": "CRUSH_GLOBAL_DATA",
                "manual_roots": "crush.db 文件、项目 data_dir 或 projects.json 目录",
                "bounded": true,
                "pattern": "projects.json {projects:[{path,data_dir}]} → 每项目 <data_dir>/crush.db（默认 <project>/.crush/crush.db）",
                "profile": "无",
            }),
            detection: serde_json::json!({
                "magic": "SQLite sessions 表 + id/parent_session_id/cost/created_at/updated_at",
                "version_field": "无；文档级锚点 crush-sessions-cost-1",
                "registry": "adapters/crush/versions 注册表（唯一条目）",
                "fail_closed": true,
                "unknown_version": "缺列即 fail closed",
            }),
            fields,
            lifecycle: serde_json::json!({
                "cost_only": "每根会话一条 UsageObservation（cost-only；token 全 Unknown）",
                "parent_rollup": "子会话 cost 回卷父行：只取根行防双计（官方统计同口径）",
            }),
            incremental: serde_json::json!({
                "cursor": "offset 恒 0（WAL）；事件键内容哈希幂等，cost 增长按请求更新推进",
            }),
            dedup: serde_json::json!({
                "primary": "crush:session:<session id>",
                "cross_project": "实例按项目库路径分列",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "token 维度完全不可用（快照语义）；官方 SUM 口径的近似值不沿用",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::sessions_cost_v1::CRUSH_PARSER_VERSION,
                "format_evidence": "charmbracelet/crush 1f3827b（agent.go/coordinator.go/internal/db sql/config/load.go）",
                "evidence_level": "official-source（无本机样本）",
                "upgrade_policy": "上游若新增逐请求用量表再扩展",
            }),
            scheduling: serde_json::json!({ "entry": "统一 run_adapter_scan" }),
            limitations: vec![
                "token 统计不可用：sessions token 列是上下文规模快照（官方证据），不采不推算"
                    .into(),
                "仅 cost：费率自算 Estimated；子会话回卷已过滤".into(),
                "本机未安装：文档级实现".into(),
            ],
        }
    }
}
