//! Built-in Zed Agent adapter; see architecture.md#adapter-layout.
//! - Stable entry point implements the shared interface and re-exports modules.
//! - detect checks threads.db tables and columns.
//! - versions registers document formats zed-threads-db-1 / 2.
//! - common owns product-specific mapping and read-only source access.
//!
//! Nonempty external-provider samples from 1.22.0 / 76659a55 were checked on 2026-10-07.
//! Only llm-usage-zhipu with DbThread 0.3.0 uses the separately verified default-zero mapping.
//! Hosted-format references: official Zed commit bd747337d7be138834e20972b9e203c7b239cc47, A38,
//! plus read-only local inspection on 2026-09-29 of %LOCALAPPDATA%/Zed/threads/threads.db:
//! matching threads schema with zero rows, without usage-format acceptance.
//! - Layout is <data_dir>/threads/threads.db; common documents platform data directories.
//!   Zed has no ZED_DATA_DIR override; --user-data-dir is not visible to collection.
//! - JSON/zstd DbThread data has model{provider,model},
//!   cumulative_token_usage (thread total) and request_token_usage
//!   (turn buckets overwritten by later requests in that turn, used only for reconciliation).
//! - The hosted path requires provider=="zed.dev"; shared imports with version "1.0.0"
//!   have initialized zero usage and are skipped. External ACP threads are written to
//!   sidebar_threads, not threads; collect through their underlying source adapters.

pub mod common;
pub mod detect;
pub mod versions;

pub use common::{map_zed, ZedUsage};
pub use detect::ZED_FORMAT;
pub use versions::threads_db_v1;
pub use versions::{LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS, ZED_FORMAT_VERSION};

/// Stateless Zed adapter.
pub struct ZedAdapter;

impl Default for ZedAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ZedAdapter {
    pub fn new() -> Self {
        ZedAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for ZedAdapter {
    fn adapter_id(&self) -> &'static str {
        "zed"
    }

    fn agent(&self) -> &'static str {
        "zed"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        if let Some(home) = &ctx.home_dir {
            if cfg!(windows) {
                // Official paths.rs:161-165 uses dirs::data_local_dir() + "Zed".
                let base = ctx
                    .env
                    .get("LOCALAPPDATA")
                    .filter(|v| !v.trim().is_empty())
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| home.join("AppData").join("Local"));
                roots.push((base.join("Zed").join("threads"), RootBasis::DefaultHome));
            } else if cfg!(target_os = "macos") {
                roots.push((
                    home.join("Library")
                        .join("Application Support")
                        .join("Zed")
                        .join("threads"),
                    RootBasis::DefaultHome,
                ));
            } else {
                // Official paths.rs:155-160 uses XDG_DATA_HOME, default ~/.local/share, plus "zed";
                // Linux uses lowercase and also has FLATPAK_XDG_DATA_HOME handling.
                let xdg = ctx
                    .env
                    .get("XDG_DATA_HOME")
                    .filter(|v| !v.trim().is_empty())
                    .map(std::path::PathBuf::from)
                    .or_else(|| {
                        ctx.env
                            .get("FLATPAK_XDG_DATA_HOME")
                            .filter(|v| !v.trim().is_empty())
                            .map(std::path::PathBuf::from)
                    })
                    .unwrap_or_else(|| home.join(".local").join("share"));
                roots.push((xdg.join("zed").join("threads"), RootBasis::DefaultHome));
            }
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out = Vec::new();
        let mut seen: std::collections::BTreeSet<std::path::PathBuf> =
            std::collections::BTreeSet::new();
        for (root, basis) in roots {
            // Accept a threads directory, an existing file, or a data directory named exactly "Zed".
            let db = if root.join("threads.db").is_file() {
                root.join("threads.db")
            } else if root.is_file() {
                root.clone()
            } else if root.file_name().and_then(|n| n.to_str()) == Some("Zed") {
                root.join("threads").join("threads.db")
            } else {
                continue;
            };
            if !db.is_file() {
                continue;
            }
            if seen.insert(db.clone()) {
                out.push(DiscoveredRoot {
                    root: db.parent().map(|p| p.to_path_buf()).unwrap_or(root),
                    basis,
                    files: vec![db],
                });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "zed@{}",
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
        versions::threads_db_v1::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let note = "Zed 1.22.0 固定修订 76659a55；2026-10-07 本机内置 Agent 使用 llm-usage-zhipu 的 GLM 两模型/缓存真实样本；hosted 仍文档级".to_string();
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(
                Availability::Partial(note.clone()),
                "线程 cumulative 四桶；已核验 OpenAI chat 的 input 为非缓存输入，默认零未知；hosted 映射保持原依据，不能互相认证",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Partial(note.clone()),
                "cache_read_input_tokens",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial(note.clone()),
                "cache_creation_input_tokens",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Unavailable("request_token_usage 是 turn 级桶且 turn 内多请求后写覆盖前写（官方 thread.rs:2893）；只作 reported_call_count（turn 数）与对账，不伪造逐次".into()),
                "turn 级（下界），非逐请求",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial(note.clone()),
                "线程级 DbThread.model（单模型）；模型中途切换未文档化",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial(note.clone()),
                "线程 created_at..updated_at（RFC3339）；逐次无时间戳（官方源码未见）",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Unavailable("threads.db 无 credits/成本字段（官方源码未见）".into()),
                "无",
            ),
        );
        fields.insert(
            "latency".into(),
            field(
                Availability::Unavailable("逐次无时间戳，无延迟证据".into()),
                "无",
            ),
        );
        CapabilityTable {
            adapter_id: "zed".to_string(),
            product: "Zed（内置 hosted Agent，zed.dev）".to_string(),
            surfaces: vec!["desktop".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": [
                    "Windows %LOCALAPPDATA%/Zed/threads/threads.db",
                    "macOS ~/Library/Application Support/Zed/threads/threads.db",
                    "Linux ~/.local/share/zed/threads/threads.db",
                ],
                "env_override": null,
                "manual_roots": "threads 目录、threads.db 文件或 Zed data 目录",
                "bounded": true,
                "pattern": "单库 threads 表；data blob json/zstd（zstd 有 64 MiB 解压上限）",
                "profile": "无",
            }),
            detection: serde_json::json!({
                "magic": "SQLite threads 表 + 必需列（id/summary/updated_at/data_type/data）",
                "version_field": "无产品版本；文档级锚点 zed-threads-db-1",
                "registry": "adapters/zed/versions 注册表（唯一条目）",
                "fail_closed": true,
                "unknown_version": "schema 偏离在探测/扫描层 fail closed，不走版本回退",
            }),
            fields,
            lifecycle: serde_json::json!({
                "aggregate_only": "每线程一条 session 级聚合（cumulative_token_usage 权威）；不展开伪造逐次事件",
                "overwrite_semantics": "request_token_usage turn 内后写覆盖前写：桶数=turn 数下界，桶合计仅作 Reconciliation 对照",
                "imported": "SharedThread（version 1.0.0）用量置零，跳过；外部 ACP Agent 会话不进 threads 表",
            }),
            incremental: serde_json::json!({
                "cursor": "offset 恒 0（WAL）；聚合按 scope_key upsert 幂等，source_revision=updated_at 毫秒（单调）",
                "bounded": "单轮 50,000 行；单 blob 解压 64 MiB 上限",
            }),
            dedup: serde_json::json!({
                "primary": "zed:thread:<id>",
                "cross_source": "zed.dev 或已核验 llm-usage-zhipu/DbThread 0.3.0；其他 provider 跳过，ACP 按底层来源计量",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "累计未保存逐次请求时间；turn 桶不认证工具循环/辅助请求数或逐模型归属",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::threads_db_v1::ZED_PARSER_VERSION,
                "format_evidence": "Zed 官方源码 bd74733（crates/agent/src/db.rs、crates/paths/src/paths.rs、language_model_core TokenUsage）+ 本机 schema 只读核验",
                "evidence_level": "official-source + native Zed 1.22.0 OpenAI chat usage（两线程三 turn）",
                "upgrade_policy": "真实样本出现后按 DbThread 实际字段升级验证；zstd/json 双格式均有界",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan",
                "incremental_cost": "整表读（≤50k 行）+ 幂等聚合 upsert",
            }),
            limitations: vec![
                "本机真实样本限内置 Agent 的 llm-usage-zhipu/DbThread 0.3.0；hosted/其他 provider 不因该样本取得真实认证".into(),
                "线程级聚合：无逐次请求时间戳/延迟；模型维度仅线程级单模型".into(),
                "request_token_usage 覆盖语义：桶数是 turn 数下界，桶合计可能小于累计值（对账不 matched 时如实展示）".into(),
                "ZED_STATELESS 模式不落盘；--user-data-dir 自定义路径需手工添加根".into(),
            ],
        }
    }
}
