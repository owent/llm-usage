//! OpenCode adapter; see architecture.md#adapter-layout.
//! - Stable entry point implements the shared interface and re-exports modules.
//! - detect checks product-specific opencode.db schema and the session.version registry.
//! - versions selects a verified implementation or latest_fallback per record.
//! - common checks product-specific schema and read-only/staged source access.
//! - Wire parsing lives in the shared crate::adapters::opencode_family module.
//!
//! Reference A17: commit 0027387dc5c59793c12dfc531abc78f825ed6868.
//! Official CLI/local-model 1.18.34 (aec0b9a6) samples verified main-loop usage and cache reads.
//! These checks do not verify other versions in the same database or every call path.
//! - xdg-basedir data lives at $XDG_DATA_HOME/opencode, defaulting to
//!   ~/.local/share/opencode; Windows layout has no real-sample verification.
//!   WAL databases use opencode.db or the channel variant opencode-<channel>.db.
//! - Per-step usage comes from part rows of type step-finish; see family module references.
//! - The earlier research note that exact per-request mapping was unimplemented was
//!   corrected after source inspection: step-finish carries usage per step and drives upstream projector counts.
//!   Map each part row; exact call timestamps and model-switch order require further event-level data.
//!   Timestamp basis remains observed_at.

pub mod common;
pub mod detect;
pub mod versions;

pub use detect::OPENCODE_FORMAT;
pub use versions::step_finish_parts_v1;
pub use versions::{LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// Stateless OpenCode adapter.
pub struct OpenCodeAdapter;

impl Default for OpenCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenCodeAdapter {
    pub fn new() -> Self {
        OpenCodeAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for OpenCodeAdapter {
    fn adapter_id(&self) -> &'static str {
        "opencode"
    }

    fn agent(&self) -> &'static str {
        "opencode"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        // xdg-basedir path from the referenced global.ts: data = xdgData/opencode.
        if let Some(xdg) = ctx.env.get("XDG_DATA_HOME") {
            if !xdg.trim().is_empty() {
                roots.push((
                    std::path::PathBuf::from(xdg.trim()).join("opencode"),
                    RootBasis::EnvOverride("XDG_DATA_HOME".to_string()),
                ));
            }
        }
        if let Some(home) = &ctx.home_dir {
            roots.push((
                home.join(".local").join("share").join("opencode"),
                RootBasis::DefaultHome,
            ));
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        // Inspect opencode*.db directly under each candidate data directory without recursive disk searches.
        // Manual roots may themselves contain databases, or provide <root>/opencode
        // or <root>/.local/share/opencode. Names alone cannot distinguish related products
        // such as Kilo with opencode-rc.db; detection must check the product-specific schema.
        let is_db_name = |p: &std::path::Path| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("opencode") && n.ends_with(".db"))
                .unwrap_or(false)
        };
        let has_db = |dir: &std::path::Path| {
            !crate::adapters::framework::enumerate_files_bounded(dir, 0, &is_db_name).is_empty()
        };
        let mut out: Vec<DiscoveredRoot> = Vec::new();
        let mut seen: std::collections::BTreeSet<std::path::PathBuf> =
            std::collections::BTreeSet::new();
        for (root, basis) in roots {
            if !root.is_dir() {
                continue;
            }
            let mut data_dirs: Vec<std::path::PathBuf> = Vec::new();
            if has_db(&root) {
                data_dirs.push(root.clone());
            }
            let direct = root.join("opencode");
            if direct.is_dir() && has_db(&direct) {
                data_dirs.push(direct);
            }
            let home_shape = root.join(".local").join("share").join("opencode");
            if home_shape.is_dir() && has_db(&home_shape) {
                data_dirs.push(home_shape);
            }
            for data_dir in data_dirs {
                if !seen.insert(data_dir.clone()) {
                    continue;
                }
                let files =
                    crate::adapters::framework::enumerate_files_bounded(&data_dir, 0, &is_db_name);
                if files.is_empty() {
                    continue;
                }
                out.push(DiscoveredRoot {
                    root: data_dir,
                    basis: basis.clone(),
                    files,
                });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "opencode@{}",
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
        // All currently attempted versions use step_finish_parts_v1; extend dispatch with the registry.
        versions::step_finish_parts_v1::scan(target, stored, limits, now_ms)
    }

    fn scan_format(
        &self,
        outcome: &crate::adapters::framework::ScanOutcome,
    ) -> Option<crate::adapters::framework::DetectOutcome> {
        let context = outcome.parse_context.as_ref()?;
        let basis = if context["has_unverified_records"].as_bool().unwrap_or(true) {
            crate::domain::VersionBasis::LatestFallback
        } else {
            serde_json::from_value(context["version_basis"].clone()).ok()?
        };
        Some(crate::adapters::framework::DetectOutcome::Supported {
            format: OPENCODE_FORMAT.into(),
            format_version: context["db_version"].as_str().map(str::to_string),
            basis,
        })
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let awaiting = "1.18.34 官方 CLI / 本地模型的真实主循环及缓存读已核对；逐记录认证，其他版本/云端及未落盘辅助调用另验".to_string();
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, note: &str| {
            serde_json::json!({
                "availability": availability,
                "note": note,
            })
        };
        fields.insert(
            "tokens".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "part.data.tokens 五字段（input=未缓存输入，pinned publish-llm-event tokens() 逐字映射）：input_total=in+cr+cw、output_total=out+reasoning 派生、total=五字段之和（与直报 total 对照）",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "tokens.cache.read 列",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "tokens.cache.write 列",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "每条 step-finish 部件 = 一次已观测主循环 step（part.id 稳定身份，time_updated 修订序号）；1.18.34 默认标题生成另有真实 API 调用，未进入该载体，不补造事件",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "join message.data.$.modelID/providerID（request_field；vendored client AssistantMessage 必需字段）；逐调用模型切换序待 event 层",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "part.time_created 是投影写入时刻（≈step 完成，observed_at），非上游精确逐调用时间；A17 逐调用时间继续追 event 层",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "step-finish 部件 cost（上游自算，estimated micro-USD）；口径未经真实样本核验",
            ),
        );
        fields.insert(
            "latency".into(),
            field(
                Availability::Unavailable("step-finish 部件无延迟/TTFT 字段".into()),
                "无",
            ),
        );
        CapabilityTable {
            adapter_id: "opencode".to_string(),
            product: "OpenCode".to_string(),
            surfaces: vec!["cli".into(), "desktop".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["$XDG_DATA_HOME/opencode", "~/.local/share/opencode"],
                "env_override": "XDG_DATA_HOME（xdg-basedir 数据目录，固定源码 global.ts）",
                "manual_roots": "数据目录本身 / <root>/opencode / <root>/.local/share/opencode 三种形状；不按文件名递归全盘",
                "bounded": true,
                "pattern": "<data>/opencode*.db（通道变体 opencode-<channel>.db；-wal/-shm 排除）",
                "scope_note": "发现按数据目录名收敛：kilo 等同血统产品目录内的 opencode-rc.db（同名前缀、schema 同形）不会被默认/父目录发现触碰；手工根直指该类目录时 schema 无法区分，见 dedup.cross_source",
                "windows_note": "Windows 布局（xdg-basedir 默认 ~/.local/share/opencode）未经真实样本核验",
                "profile": "无 profile 概念（固定源码未见）",
            }),
            detection: serde_json::json!({
                "magic": "SQLite + part/session/message 三表关键列（schema 指纹；session 须含 tokens_* 五累计列 ⇒ 与 MiMo 库互斥）",
                "version_field": "step-finish 所属 session.version（空会话不参与）",
                "registry": "adapters/opencode/versions 仅登记 1.18.34；按 step-finish 所属会话选择，混合版本/空会话/旧游标升级已回归，其他版本继续 latest_fallback",
                "fail_closed": true,
                "unknown_version": "未收录/缺失版本一律 latest_fallback（带兼容标记）；仅新 core session_message 投影层 fail closed 待取证",
            }),
            fields,
            lifecycle: serde_json::json!({
                "model_call": "step-finish 部件逐次入账（part.id 身份，time_updated 修订序号）",
                "no_double_count": "只读 part 表；assistant message.data.tokens 是 turn 级聚合不读，session.tokens_* 累计列不逐次相加（A17）",
                "cumulative_snapshot": "session.tokens_* 五列 = Σ 当前部件（projector applyUsage 含删行补偿），仅作会话对账",
                "subagent": "session.parent_id 非空 ⇒ sub_agent；子会话是独立 session 行（sql.ts parent_id 列）",
                "part_removal": "上游 PartRemoved/MessageRemoved 会扣减 session 计数（-sign 路径）；被删部件行消失后不再入账，靠对账 mismatch 可见",
            }),
            incremental: serde_json::json!({
                "cursor": "schema 指纹 + part.id 稳定键 + time_updated 处理位置（60s 有界重叠窗）；未完成分页保留原窗口起点",
                "row_cap": "单轮最多 50,000 行，并遵守 max_lines；以 time_updated/part.id 元组续扫，完成窗口后恢复重叠",
                "schema_evolution": "指纹变化 ⇒ 处理位置重置全量重读（id 键 upsert 幂等）",
                "no_change_detection": "WAL 下主库文件长度不变不代表内容未变：游标 offset 恒 0，每轮执行处理位置查询而非字节短路",
                "rescan_note": "in-place 页重写触发框架 Rescan 标记；处理位置不重置（同一逻辑库）",
            }),
            dedup: serde_json::json!({
                "primary": "opencode:part:{part.id}（实例命名空间）",
                "cross_source": "event 表（事件溯源层）与 session_message 投影不读；kilo 等其他 opencode 派生产品数据根不重叠、注册表独立——kilo 目录内的 opencode-rc.db 与本库 schema 同形，默认/父目录发现不触碰（数据目录名收敛），手工根直指该目录时仅靠来源归属区分",
                "rescan": "重扫/重叠窗重复行按同键同修订幂等（unchanged），不双计",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "1.18.34 默认标题生成未进入 step-finish 与会话累计，matched 不证明全部 API 调用被覆盖；被删部件/未完成 step 缺证据不计、不补零",
                "sampling": "未观测到采样；坏 data 行逐条隔离记诊断",
                "source_retention": "源端保留未知；可回填范围以现存行时间为准",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::step_finish_parts_v1::PARSER_VERSION,
                "format_evidence": "固定源码 0027387 + 1.18.34/aec0b9a6 的 sql/projector/processor/prompt 与真实脱敏 fixtures；API usage/CLI/原生库/应用独立核对",
                "evidence_level": "real-local（1.18.34 主循环/缓存读，2026-10-05；默认标题调用覆盖缺口保留）",
                "upgrade_policy": "逐记录按 session.version 认证；支持或规则升级重评未变化的旧处理位置，完整有效重评标记完成；空会话及库内最高版本不认证其他记录",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan；手动/间隔/监听触发按源合并",
                "incremental_cost": "time_updated 处理位置查询；每轮固定一次探测查询",
                "pause_cancel": "行级游标可停；busy 源转暂存副本或保留旧结果下轮重试",
            }),
            limitations: vec![
                "仅 1.18.34 Linux CLI 主循环及缓存读有真实样本；其他版本仍 latest_fallback，混合库按各自会话保留依据".into(),
                "默认标题生成未进入 step-finish：真实 API 2 次/848 token，原生载体及应用仅 1 次/299 token；明确标题的对照为 1 次/299 token，不能替代默认覆盖结论".into(),
                "tokens 五互斥口径已在 1.18.34 本地模型核对（未缓存 295 + 缓存读 3 = API 输入 298）；其他 provider/版本、缓存写及非零 reasoning 尚待真实核验".into(),
                "逐调用精确时间与模型切换序待上游 event 层（A17）：part 行时间标 observed_at，模型归属经 message join".into(),
                "仅解析 part/session/message 三表；仅存新 core session_message 投影层的库 fail closed 待专用实现".into(),
                "Windows 默认目录（xdg-basedir 语义）与通道变体库文件名未经真实样本核验".into(),
                "step 与 message 汇总不双计：只读 part；被删部件在源表消失后不可恢复，残差靠对账可见".into(),
            ],
        }
    }
}
