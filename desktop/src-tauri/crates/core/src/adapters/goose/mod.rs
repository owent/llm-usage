//! Goose 适配器（Block→aaif-goose；独立目录合同）。载体：sessions.db 的
//! usage_ledger 逐请求表（迁移 15+）；旧库按 sessions.accumulated_* 聚合兜底。

pub mod common;
pub mod detect;
pub mod versions;

pub use detect::GOOSE_FORMAT;
pub use versions::usage_ledger_v1;
pub use versions::{GOOSE_FORMAT_VERSION, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// Goose 适配器（无状态）。
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
        // GOOSE_PATH_ROOT（必须绝对路径，官方 paths.rs:41-50）。
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
                // etcetera 0.11 Windows 策略：Roaming/<author>/<app>/data。
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
                // 旧 Block 时代 macOS 根（官方注释保留兼容）。
                roots.push((
                    home.join("Library")
                        .join("Application Support")
                        .join("Block")
                        .join("goose")
                        .join("sessions"),
                    RootBasis::DefaultHome,
                ));
            }
            // 旧 Block 时代 unix 根。
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
            // 文件形手工根：root 取文件本身——同目录下两个手工 db 文件的
            // root 规范化后相同会被框架去重吞并其一（该库永不扫描）。
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
            "官方源码证据（aaif-goose/goose a701bb1）；本机未安装，待真实样本核验".to_string();
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
                "format_evidence": "aaif-goose/goose a701bb1（session_manager.rs 迁移/建表/record_usage_metrics/token_usage.rs）",
                "evidence_level": "official-source（无本机样本）",
                "upgrade_policy": "真实样本后核验列形状与 cost_source 取值",
            }),
            scheduling: serde_json::json!({ "entry": "统一 run_adapter_scan" }),
            limitations: vec![
                "本机未安装：文档级实现".into(),
                "estimated/carried_forward 成本不映射；reasoning 差额不推算".into(),
                "旧库兜底只到会话级（无逐请求）".into(),
            ],
        }
    }
}
