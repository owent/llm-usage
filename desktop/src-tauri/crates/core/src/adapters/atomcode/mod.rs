//! AtomCode 适配器（AtomGit 生态 CLI；独立目录约定）。载体：
//! `$ATOMCODE_HOME`（默认 ~/.atomcode）/sessions/<project_hash>/<id>.meta 的
//! turn_stats 按模型累计（TurnStat 无时间戳 ⇒ 会话区间聚合；round_count 合计
//! 作调用数）。旧版单文件 <id>.json 同构。

pub mod detect;
pub mod versions;

pub use detect::ATOMCODE_FORMAT;
pub use versions::meta_turns_v1;
pub use versions::{ATOMCODE_FORMAT_VERSION, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// AtomCode 适配器（无状态）。
pub struct AtomCodeAdapter;

impl Default for AtomCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl AtomCodeAdapter {
    pub fn new() -> Self {
        AtomCodeAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for AtomCodeAdapter {
    fn adapter_id(&self) -> &'static str {
        "atomcode"
    }

    fn agent(&self) -> &'static str {
        "atomcode"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        if let Some(home) = ctx
            .env
            .get("ATOMCODE_HOME")
            .filter(|v| !v.trim().is_empty())
        {
            roots.push((
                std::path::PathBuf::from(home.trim()).join("sessions"),
                RootBasis::EnvOverride("ATOMCODE_HOME".to_string()),
            ));
        }
        if let Some(home) = &ctx.home_dir {
            roots.push((
                home.join(".atomcode").join("sessions"),
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
            // .meta（当前）与旧版单文件 .json（含 turn_stats）同构读取；
            // .jsonl/.snapshot 等不读。
            let files = crate::adapters::framework::enumerate_files_bounded(&root, 2, &|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e == "meta" || e == "json")
            });
            if !files.is_empty() && seen.insert(root.clone()) {
                out.push(DiscoveredRoot { root, basis, files });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "atomcode@{}",
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
        versions::meta_turns_v1::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let note =
            "官方源码证据（atomgit e4215f7，GitHub 镜像同 SHA 核验）；本机未安装，待真实样本核验"
                .to_string();
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(
                Availability::Partial(note.clone()),
                "TokenBreakdown 三桶（input=prompt−cached 非缓存、cached_input=缓存命中、output）；按模型跨 turn 累计",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(Availability::Partial(note.clone()), "cached_input"),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Unavailable("kernel 三列无 cache 写桶".into()),
                "无",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Partial(note.clone()),
                "turn_stats[].round_count 合计（LLM 往返数；TurnStat 无时间戳不展开逐次）",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial(note.clone()),
                "model_usage[].provider_id/model_id（含 detached_model_usage）",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial(note.clone()),
                "会话 created_at..updated_at（epoch 毫秒）",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Unavailable("无成本字段（源码未见）".into()),
                "无",
            ),
        );
        fields.insert(
            "latency".into(),
            field(
                Availability::Unavailable(
                    "turn duration_ms 不在 .meta 主载体逐 turn 读取范围".into(),
                ),
                "无",
            ),
        );
        CapabilityTable {
            adapter_id: "atomcode".to_string(),
            product: "AtomCode（AtomGit 生态，联合华为 InsCode AI IDE；本适配器为 CLI 形态）"
                .to_string(),
            surfaces: vec!["cli".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["~/.atomcode/sessions"],
                "env_override": "ATOMCODE_HOME",
                "manual_roots": "sessions 目录或 project_hash 目录",
                "bounded": true,
                "pattern": "sessions/<stable_project_hash>/<id>.meta（深度 2）+ 旧版单文件 <id>.json；.jsonl/.snapshot/.todos/.rewind 不读",
                "profile": "无",
            }),
            detection: serde_json::json!({
                "magic": "JSON 含 id + turn_stats",
                "version_field": "meta.v schema 版本整数不逐值锚定；文档级锚点 atomcode-meta-turns-1",
                "registry": "adapters/atomcode/versions 注册表（唯一条目）",
                "fail_closed": true,
                "unknown_version": "schema 偏离 fail closed",
            }),
            fields,
            lifecycle: serde_json::json!({
                "session_aggregate": "TurnStat 无时间戳 ⇒ 每 (session, model) 一条区间聚合；round_count 合计作 reported_call_count",
                "detached": "detached_model_usage（不归属 turn 的用量）并入同模型累计；detached_unattributed_tokens（无模型）不归任何模型，如实丢弃记限制",
            }),
            incremental: serde_json::json!({
                "cursor": "整写 JSON 字节游标；scope_key=atomcode:session:<id>:<provider/model>，revision=updated_at",
            }),
            dedup: serde_json::json!({
                "primary": "atomcode:session:<id>:<provider>/<model>",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "turn_stats 只存每 turn 末次请求的 total_tokens（快照）；模型桶累计是产品自记口径",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::meta_turns_v1::ATOMCODE_PARSER_VERSION,
                "format_evidence": "atomgit.com/atomgit_atomcode/atomcode e4215f7（crates/atomcode-capabilities session/manager.rs + usage_provider.rs + atomcode-config distribution.rs）",
                "evidence_level": "official-source（无本机样本）",
                "upgrade_policy": "真实样本后核验 meta.v 各版本形状",
            }),
            scheduling: serde_json::json!({ "entry": "统一 run_adapter_scan" }),
            limitations: vec![
                "会话级聚合：turn 无时间戳，不虚构逐次/延迟".into(),
                "无 cache 写桶（kernel 三列）".into(),
                "detached_unattributed_tokens（无模型归属）不计入任何模型行".into(),
                "本机未安装：文档级实现；InsCode IDE 归 F1".into(),
            ],
        }
    }
}
