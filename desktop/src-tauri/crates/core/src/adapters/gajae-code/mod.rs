//! gajae-code, command gjc, independent adapter; shares Pi v3 ancestry,
//! but official usage normalization uses exclusive buckets. Reads <agentDir>/sessions/<scope>/*.jsonl.
//! Child files nest under the parent-name directory, depth ≤3; deduplicate by file and entry ID.

pub mod detect;
pub mod versions;

pub use detect::GJC_FORMAT;
pub use versions::session_v3like;
pub use versions::{GJC_FORMAT_VERSION, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// Stateless gajae-code adapter.
pub struct GajaeCodeAdapter;

impl Default for GajaeCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl GajaeCodeAdapter {
    pub fn new() -> Self {
        GajaeCodeAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for GajaeCodeAdapter {
    fn adapter_id(&self) -> &'static str {
        "gajae-code"
    }

    fn agent(&self) -> &'static str {
        "gajae-code"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        // Follow official dirs.ts order; product prevents .env contamination, collector uses process env.
        for env_name in ["GJC_CODING_AGENT_DIR", "PI_CODING_AGENT_DIR"] {
            if let Some(dir) = ctx.env.get(env_name).filter(|v| !v.trim().is_empty()) {
                roots.push((
                    std::path::PathBuf::from(dir.trim()).join("sessions"),
                    RootBasis::EnvOverride(env_name.to_string()),
                ));
                break;
            }
        }
        let config_name = ["GJC_CONFIG_DIR", "PI_CONFIG_DIR"]
            .iter()
            .find_map(|env| {
                ctx.env
                    .get(*env)
                    .filter(|v| !v.trim().is_empty())
                    .map(|v| (v.trim().to_string(), *env))
            })
            .map(|(v, env)| (std::path::PathBuf::from(v), env.to_string()));
        // config_dir override is absolute and home-independent; do not require home_dir.
        if let Some((dir, env)) = config_name {
            roots.push((
                dir.join("agent").join("sessions"),
                RootBasis::EnvOverride(env),
            ));
        } else if let Some(home_dir) = ctx.home_dir.as_ref() {
            roots.push((
                home_dir.join(".gjc").join("agent").join("sessions"),
                RootBasis::DefaultHome,
            ));
            // XDG applies only without override; official layout flattens the agent/ segment.
            if !cfg!(windows) {
                let xdg = ctx
                    .env
                    .get("XDG_DATA_HOME")
                    .filter(|v| !v.trim().is_empty())
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| home_dir.join(".local").join("share"));
                roots.push((xdg.join("gjc").join("sessions"), RootBasis::DefaultHome));
            }
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out = Vec::new();
        let mut seen: std::collections::BTreeSet<std::path::PathBuf> =
            std::collections::BTreeSet::new();
        for (root, basis) in roots {
            // Sessions are depth 1 at scope/<file>; child agents may nest to depth 3.
            let files = crate::adapters::framework::enumerate_files_bounded(&root, 3, &|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("jsonl"))
            });
            if !files.is_empty() && seen.insert(root.clone()) {
                out.push(DiscoveredRoot { root, basis, files });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "gjc@{}",
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
        versions::session_v3like::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let note =
            "官方源码及 gajae-code 0.18.7 单轮真实本地模型；其他 API/版本与场景待核验".to_string();
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(
                Availability::Partial(note.clone()),
                "归一化输入/输出；OpenAI-completions 零缓存与零用量保持未知，无法确认缓存桶时不展示未缓存输入",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Partial(note.clone()),
                "cacheRead；OpenAI-completions 零值可能是缺字段回退，保持未知",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial(note.clone()),
                "cacheWrite；OpenAI-completions 零值可能是缺字段回退，保持未知",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Partial(note.clone()),
                "每 assistant 消息一次（五桶齐全才统计，官方 parser 同口径）",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial(note.clone()),
                "message.model + provider（逐请求字段）",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial(note.clone()),
                "message.timestamp 毫秒优先；已核验 OpenAI-completions 为请求开始，缺则条目 ISO",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Partial(note.clone()),
                "cost.total（USD，费率自算 ⇒ Estimated micro-USD；OpenAI 历史路径曾写 0）",
            ),
        );
        fields.insert(
            "latency".into(),
            field(Availability::Unavailable("无延迟字段".into()), "无"),
        );
        CapabilityTable {
            adapter_id: "gajae-code".to_string(),
            product: "gajae-code（gjc，pi 血统）".to_string(),
            surfaces: vec!["cli".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": [
                    "$GJC_CODING_AGENT_DIR|$PI_CODING_AGENT_DIR/sessions",
                    "$GJC_CONFIG_DIR|$PI_CONFIG_DIR/agent/sessions",
                    "~/.gjc/agent/sessions（默认）",
                    "$XDG_DATA_HOME/gjc/sessions（非 Windows，官方拍平 agent 段）",
                ],
                "env_override": "GJC_CODING_AGENT_DIR/PI_CODING_AGENT_DIR/GJC_CONFIG_DIR/PI_CONFIG_DIR",
                "manual_roots": "sessions 目录或 scope 目录",
                "bounded": true,
                "pattern": "scope/<ts>_<uuid7>.jsonl（深度 1）+ 子代理嵌套（父文件去 .jsonl 为 artifacts 目录，深度 ≤3）",
                "profile": "无",
            }),
            detection: serde_json::json!({
                "magic": "首行 {type:\"session\",version,id,timestamp} 头",
                "version_field": "session.version（5 = KnownVersion；其他/缺失 LatestFallback 兼容尝试）",
                "registry": "adapters/gajae-code/versions 注册表（唯一条目）",
                "fail_closed": true,
                "unknown_version": "未文档化条目类型整文件拒绝（V17）",
            }),
            fields,
            lifecycle: serde_json::json!({
                "five_bucket_gate": "五桶齐全且 model/provider 非空才统计（官方 stats parser 同口径）；缺桶跳过不补零",
                "subagent": "子代理文件独立嵌套；会话文件+条目 id 去重（官方 UNIQUE(session_file, entry_id) 同键）",
            }),
            incremental: serde_json::json!({
                "cursor": "JSONL 字节偏移；事件键 gjc:<session id>:<entry id>",
            }),
            dedup: serde_json::json!({
                "primary": "gjc:<session id>:<entry id>",
                "replay": "子代理重放/跨文件复制按同键 upsert 幂等",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "缺桶 assistant 行不入账（官方同样跳过）",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::session_v3like::GJC_PARSER_VERSION,
                "format_evidence": "Yeachan-Heo/gajae-code 7e54f9c（docs/session.md + packages/ai types.ts + packages/utils dirs.ts + packages/stats）",
                "evidence_level": "real-local（0.18.7/session v5 单次 OpenAI-completions：API/CLI/载体 412/2/414；未知缓存、未缓存、推理保留；其他 API/场景未验收）",
                "upgrade_policy": "完整旧 gjc-session-1 摘要仅纠正零回退/未缓存与时间依据；其余字段继续仲裁，旧游标自动重评",
            }),
            scheduling: serde_json::json!({ "entry": "统一 run_adapter_scan" }),
            limitations: vec![
                "pi 家族复用但逐项验证：map_pi_family 的互斥口径以 gjc 官方归一化证据为准（现行 pi 已分叉 v4 头）".into(),
                "message.timestamp 毫秒与条目 ISO 并存：实现以 message.timestamp 优先".into(),
                "仅核验 0.18.7 一次本地调用；云端、分支/重绕、子 Agent、多模型和其他版本未验收".into(),
            ],
        }
    }
}
