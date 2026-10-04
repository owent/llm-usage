//! Zed 内置 Agent 适配器（独立目录约定 architecture.md#adapter-layout）：
//! - 本模块是该 Agent 的稳定入口（统一接口实现与再导出）；
//! - [`detect`]：threads.db 表/列指纹；
//! - [`versions`]：格式注册表（唯一条目：文档级 zed-threads-db-1）；
//! - 产品特有映射与源库只读访问在 [`common`]。
//!
//! 格式依据（Zed 官方源码 bd747337d7be138834e20972b9e203c7b239cc47，A38；
//! 本机 2026-09-29 只读核验 `%LOCALAPPDATA%/Zed/threads/threads.db` threads 表
//! schema 一致、0 行）：
//! - 库布局 `<data_dir>/threads/threads.db`；data_dir 三平台默认见 common；
//!   主程序无 ZED_DATA_DIR 环境变量（覆盖是 CLI --user-data-dir，采集不可见）。
//! - data blob（json/zstd）DbThread：model{provider,model}、
//!   cumulative_token_usage（线程总量，权威）、request_token_usage
//!   （turn 级桶，turn 内多请求后写覆盖前写，仅作对账）。
//! - 仅 provider=="zed.dev" 的 hosted 调用计入；分享导入线程（version "1.0.0"）
//!   用量置零跳过；外部 ACP Agent 会话不进 threads 表（官方 thread_import 写
//!   sidebar_threads），按底层来源适配器计量，不计为 Zed 内置支持。

pub mod common;
pub mod detect;
pub mod versions;

pub use common::{map_zed, ZedUsage};
pub use detect::ZED_FORMAT;
pub use versions::threads_db_v1;
pub use versions::{LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS, ZED_FORMAT_VERSION};

/// Zed 适配器（无状态）。
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
                // 官方 paths.rs:161-165：dirs::data_local_dir() + "Zed"。
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
                // 官方 paths.rs:155-160：XDG_DATA_HOME（默认 ~/.local/share）+ "zed"
                //（Linux 用小写）；FLATPAK_XDG_DATA_HOME 特例。
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
            // 手工根宽松：threads 目录、threads.db 本身或 Zed data 目录均可。
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
        let note = "官方源码证据（bd74733）+ 本机 schema 核验（2026-09-29，0 行）；真实用量样本待用户使用 Zed hosted agent 后核验".to_string();
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(
                Availability::Partial(note.clone()),
                "cumulative_token_usage 四桶（input/output/cache_read/cache_creation，线程级累计；缺省=已报告 0，官方 skip_serializing_if 语义）",
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
                "cross_source": "非 zed.dev provider 线程跳过（外部 Agent 由底层来源适配器计量）",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "仅 zed.dev hosted 调用；本地模型/自有 key 调用 provider 非 zed.dev 不计入（如实标注）",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::threads_db_v1::ZED_PARSER_VERSION,
                "format_evidence": "Zed 官方源码 bd74733（crates/agent/src/db.rs、crates/paths/src/paths.rs、language_model_core TokenUsage）+ 本机 schema 只读核验",
                "evidence_level": "official-source + local schema（0 行，无真实用量样本）",
                "upgrade_policy": "真实样本出现后按 DbThread 实际字段升级验证；zstd/json 双格式均有界",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan",
                "incremental_cost": "整表读（≤50k 行）+ 幂等聚合 upsert",
            }),
            limitations: vec![
                "本机 threads.db 为空（2026-09-29 核验 0 行）：无真实用量样本，实现按官方源码文档级交付".into(),
                "线程级聚合：无逐次请求时间戳/延迟；模型维度仅线程级单模型".into(),
                "request_token_usage 覆盖语义：桶数是 turn 数下界，桶合计可能小于累计值（对账不 matched 时如实展示）".into(),
                "ZED_STATELESS 模式不落盘；--user-data-dir 自定义路径需手工添加根".into(),
            ],
        }
    }
}
