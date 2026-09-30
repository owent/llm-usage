//! gajae-code（命令 `gjc`）适配器（独立目录合同）。pi 血统（共同祖先 v3），
//! 但 usage 已官方归一化为互斥桶。载体：`<agentDir>/sessions/<scope>/*.jsonl`；
//! 子代理文件嵌套在父会话同名目录下（深度 ≤3 枚举，会话文件+条目 id 去重）。

pub mod detect;
pub mod versions;

pub use detect::GJC_FORMAT;
pub use versions::session_v3like;
pub use versions::{GJC_FORMAT_VERSION, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// gajae-code 适配器（无状态）。
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
        // 官方 dirs.ts 解析顺序（.env 污染防护由产品侧负责，采集器取环境一级）。
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
        // config_dir 覆盖是绝对路径、不依赖 home_dir：不能嵌在 home 判断里丢失。
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
            // XDG（仅非 override 时；官方拍平 agent/ 段）。
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
            // 会话文件深度 1（scope/<file>）；子代理嵌套至深度 3。
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
        let note = "官方源码证据（Yeachan-Heo/gajae-code 7e54f9c + docs/session.md）；本机未安装，待真实样本核验".to_string();
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(
                Availability::Partial(note.clone()),
                "usage 已归一化互斥桶（input 非缓存/output 含 thinking/cacheRead/cacheWrite/totalTokens=四桶和）；与 pi 家族同口径",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(Availability::Partial(note.clone()), "cacheRead"),
        );
        fields.insert(
            "cache_write".into(),
            field(Availability::Partial(note.clone()), "cacheWrite"),
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
                "message.timestamp（Unix 毫秒）优先；缺则条目 ISO（两种单位并存）",
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
                "evidence_level": "official-source（无本机样本）",
                "upgrade_policy": "真实样本后核验五桶/双时间戳单位",
            }),
            scheduling: serde_json::json!({ "entry": "统一 run_adapter_scan" }),
            limitations: vec![
                "pi 家族复用但逐项验证：map_pi_family 的互斥口径以 gjc 官方归一化证据为准（现行 pi 已分叉 v4 头）".into(),
                "message.timestamp 毫秒与条目 ISO 并存：实现以 message.timestamp 优先".into(),
                "本机未安装：文档级实现".into(),
            ],
        }
    }
}
