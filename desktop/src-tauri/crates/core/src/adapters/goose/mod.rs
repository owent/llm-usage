//! Goose adapter (Block→aaif-goose; independent directory). Reads sessions.db
//! per-request usage_ledger, migration 15+; old DBs fall back to sessions.accumulated_* aggregates.

pub mod common;
pub mod detect;
pub mod versions;

pub use detect::GOOSE_FORMAT;
pub use versions::usage_ledger_v1;
pub use versions::{GOOSE_FORMAT_VERSION, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// Stateless Goose adapter.
pub struct GooseAdapter;

impl Default for GooseAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl GooseAdapter {
    pub fn new() -> Self {
        GooseAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for GooseAdapter {
    fn adapter_id(&self) -> &'static str {
        "goose"
    }

    fn agent(&self) -> &'static str {
        "goose"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        // GOOSE_PATH_ROOT must be absolute (official paths.rs:41–50).
        if let Some(root) = ctx.env.get("GOOSE_PATH_ROOT").filter(|v| {
            let t = v.trim();
            !t.is_empty() && std::path::Path::new(t).is_absolute()
        }) {
            roots.push((
                std::path::PathBuf::from(root.trim())
                    .join("data")
                    .join("sessions"),
                RootBasis::EnvOverride("GOOSE_PATH_ROOT".to_string()),
            ));
        }
        if let Some(home) = &ctx.home_dir {
            if cfg!(windows) {
                // etcetera 0.11 Windows layout: Roaming/<author>/<app>/data.
                if let Some(appdata) = ctx.env.get("APPDATA") {
                    roots.push((
                        std::path::PathBuf::from(appdata)
                            .join("Block")
                            .join("goose")
                            .join("data")
                            .join("sessions"),
                        RootBasis::DefaultHome,
                    ));
                }
            } else {
                let xdg = ctx
                    .env
                    .get("XDG_DATA_HOME")
                    .filter(|v| !v.trim().is_empty())
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| home.join(".local").join("share"));
                roots.push((xdg.join("goose").join("sessions"), RootBasis::DefaultHome));
                // Legacy Block macOS root retained for compatibility by official comments.
                roots.push((
                    home.join("Library")
                        .join("Application Support")
                        .join("Block")
                        .join("goose")
                        .join("sessions"),
                    RootBasis::DefaultHome,
                ));
            }
            // Legacy Block Unix root.
            roots.push((
                home.join(".local")
                    .join("share")
                    .join("Block")
                    .join("goose")
                    .join("sessions"),
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
            // File manual roots use the file itself: two DBs in one directory must not
            // normalize to one directory and lose one forever through framework deduplication.
            let (db, root_key) = if root.join("sessions.db").is_file() {
                (root.join("sessions.db"), root.clone())
            } else if root.is_file() {
                (root.clone(), root.clone())
            } else {
                continue;
            };
            if seen.insert(db.clone()) {
                out.push(DiscoveredRoot {
                    root: root_key,
                    basis,
                    files: vec![db],
                });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "goose@{}",
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
        versions::usage_ledger_v1::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let note =
            "官方源码（a701bb1）与官方 1.53.0 CLI 本地模型真实 usage_ledger；更多场景仍待核验"
                .to_string();
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(
                Availability::Partial(note.clone()),
                "usage_ledger 逐请求五列（input 含 cache 读写，官方口径，uncached 派生）；旧库按 accumulated_* 会话聚合",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(Availability::Partial(note.clone()), "cache_read_tokens"),
        );
        fields.insert(
            "cache_write".into(),
            field(Availability::Partial(note.clone()), "cache_write_tokens"),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Partial(note.clone()),
                "每行一次 provider 响应（含 compaction 基线调用）",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial(note.clone()),
                "ledger.model（会话级 model_config_json 仅作背景不读）",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial(note.clone()),
                "created_timestamp（Unix 秒，事件时刻）",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Partial(note.clone()),
                "cost 列：provider_reported ⇒ Reported（micro-USD）；estimated/carried_forward 不映射（估算不采信）",
            ),
        );
        fields.insert(
            "latency".into(),
            field(Availability::Unavailable("无延迟列".into()), "无"),
        );
        CapabilityTable {
            adapter_id: "goose".to_string(),
            product: "Goose（Block → aaif-goose）".to_string(),
            surfaces: vec!["cli".into(), "desktop".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": [
                    "GOOSE_PATH_ROOT/data/sessions/sessions.db（env，绝对路径）",
                    "Windows %APPDATA%/Block/goose/data/sessions/sessions.db（etcetera 策略）",
                    "unix $XDG_DATA_HOME|~/.local/share/goose/sessions/sessions.db",
                    "旧 Block 根：~/Library/Application Support/Block/goose/sessions、~/.local/share/Block/goose/sessions",
                ],
                "env_override": "GOOSE_PATH_ROOT",
                "manual_roots": "sessions 目录或 sessions.db 文件",
                "bounded": true,
                "pattern": "单库 sessions.db（WAL；只读 + busy 暂存副本）",
                "profile": "无",
            }),
            detection: serde_json::json!({
                "magic": "usage_ledger 表（迁移 15+）或 sessions.accumulated_* 列（旧库兜底）",
                "version_field": "schema_version 列不直读；按表存在性分派（文档级锚点 goose-usage-ledger-1）",
                "registry": "adapters/goose/versions 注册表（唯一条目）",
                "fail_closed": true,
                "unknown_version": "schema 指纹变化重置全量重读；事件键 upsert 幂等",
            }),
            fields,
            lifecycle: serde_json::json!({
                "per_request": "每 provider 响应 INSERT 一行；is_compaction=1 ⇒ Auxiliary",
                "carried_forward": "累计补账行：token 计入（产品自己的对账），cost 不映射",
                "fork": "fork/copy 不复制用量；subagent 独立行全表求和不双计（官方口径）",
            }),
            incremental: serde_json::json!({
                "cursor": "append-only 表：已处理最大 ledger id；单轮 50k 行",
                "old_db": "旧库兜底模式按 sessions 聚合（与 ledger 互斥防双计）",
            }),
            dedup: serde_json::json!({
                "primary": "goose:ledger:<id> / goose:session:<id>（兜底）",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "sessions 单次列（最后快照）不读；reasoning 差额推算（tokscale）不采纳",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::usage_ledger_v1::GOOSE_PARSER_VERSION,
                "format_evidence": "aaif-goose/goose a701bb1 + v1.53.0 固定官方发布包；usage_ledger 1 次/322 token 与真实本地模型 API 及应用一致",
                "evidence_level": "real-local（1.53.0 CLI、无扩展本地模型；缓存写/cost 为 NULL，更多场景未验收）",
                "upgrade_policy": "保留 goose-usage-ledger-1 格式锚点；采样客户端版本不认证其他记录/版本，继续核验 schema 与 cost_source",
            }),
            scheduling: serde_json::json!({ "entry": "统一 run_adapter_scan" }),
            limitations: vec![
                "真实验收仅 1.53.0 CLI 一次本地模型响应；桌面、旧库、fork/子 Agent/compaction 和其他版本仍待核验".into(),
                "estimated/carried_forward 成本不映射；reasoning 差额不推算".into(),
                "旧库兜底只到会话级（无逐请求）".into(),
            ],
        }
    }
}
