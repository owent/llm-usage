//! MiMo Code 适配器（独立目录约定 architecture.md#adapter-layout）：
//! - 本模块是该 Agent 的稳定入口（统一接口实现与再导出）；
//! - [`detect`]：mimocode.db schema 指纹（message.agent_id 产品互斥锚点，
//!   不从 OpenCode 派生关系推兼容）+ session.version 注册表；
//! - [`versions`]：格式注册表（当前空：文档或源码依据，全部 latest_fallback）；
//! - [`common`]：产品互斥 schema 指纹 + 源库只读/暂存副本约定；
//! - wire 解析核心在家族共享模块 [`crate::adapters::opencode_family`]。
//!
//! 格式依据（A14 固定源码 456678b6a5afb0eef3fe2754575637218cfb3c84，
//! 按文档或源码实现，待真实样本核验；本机 2026-09-25 盘点 not_found）：
//! - 路径：`resolveMimocodeHome()`（MIMOCODE_HOME 绝对路径 → `<home>/data`；
//!   否则 XDG `$XDG_DATA_HOME/mimocode`，缺省 ~/.local/share/mimocode）下
//!   `mimocode.db`（通道变体 `mimocode-<channel>.db`），WAL；
//! - 逐次 usage：`part` 表 step-finish 部件（family 模块头证据链）；
//!   step 与 message 汇总不双计（只读 step 侧）。

pub mod common;
pub mod detect;
pub mod versions;

pub use detect::MIMO_CODE_FORMAT;
pub use versions::step_finish_parts_v1;
pub use versions::{LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// MiMo 官方环境覆盖（固定源码 resolveMimocodeHome：须绝对路径，非绝对上游
/// 抛错；发现层对非绝对值跳过并依赖其余候选）。
pub const MIMOCODE_ENV_HOME: &str = "MIMOCODE_HOME";

/// MiMo Code 适配器（无状态）。
pub struct MimoCodeAdapter;

impl Default for MimoCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl MimoCodeAdapter {
    pub fn new() -> Self {
        MimoCodeAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for MimoCodeAdapter {
    fn adapter_id(&self) -> &'static str {
        "mimo-code"
    }

    fn agent(&self) -> &'static str {
        "mimo-code"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        // MIMOCODE_HOME → <home>/data（resolveMimocodeHome mimocode_home 模式）。
        if let Some(home) = ctx.env.get(MIMOCODE_ENV_HOME) {
            if !home.trim().is_empty() {
                roots.push((
                    std::path::PathBuf::from(home.trim()).join("data"),
                    RootBasis::EnvOverride(MIMOCODE_ENV_HOME.to_string()),
                ));
            }
        }
        // XDG 默认（xdg 模式：$XDG_DATA_HOME/mimocode，缺省 ~/.local/share/mimocode）。
        if let Some(xdg) = ctx.env.get("XDG_DATA_HOME") {
            if !xdg.trim().is_empty() {
                roots.push((
                    std::path::PathBuf::from(xdg.trim()).join("mimocode"),
                    RootBasis::EnvOverride("XDG_DATA_HOME".to_string()),
                ));
            }
        }
        if let Some(home) = &ctx.home_dir {
            roots.push((
                home.join(".local").join("share").join("mimocode"),
                RootBasis::DefaultHome,
            ));
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        // 发现范围按**数据目录名**限定（不按文件名递归全盘）：mimocode*.db 只在
        // mimocode 数据目录或 MIMOCODE_HOME/data 内接受。手工根兼容四种形状：
        // 数据目录本身、<root>/data（MIMOCODE_HOME 形状）、<root>/mimocode、
        // <root>/.local/share/mimocode——防止把同血统产品目录里的同名前缀库
        // 误认为本产品。
        let is_db_name = |p: &std::path::Path| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("mimocode") && n.ends_with(".db"))
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
            for rel in [
                std::path::PathBuf::from("data"),
                std::path::PathBuf::from("mimocode"),
                std::path::PathBuf::from(".local")
                    .join("share")
                    .join("mimocode"),
            ] {
                let candidate = root.join(rel);
                if candidate.is_dir() && has_db(&candidate) {
                    data_dirs.push(candidate);
                }
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
            "mimo-code@{}",
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
        // 当前所有可尝试版本共用 step_finish_parts_v1；注册表扩展后在此分派。
        versions::step_finish_parts_v1::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let awaiting =
            "文档级证据（A14 固定源码 456678b）；本机 not_found（2026-09-25 盘点），待真实样本核验"
                .to_string();
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
                "part.data.tokens 五字段（input/output/reasoning/cache.read/cache.write；家族共享 pinned 语义：input=未缓存输入）：input_total=in+cr+cw、output_total=out+reasoning 派生、total=五字段之和（与直报 total 对照）",
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
                "每条 step-finish 部件 = 一次 step 的模型调用（part.id 稳定身份，time_updated 修订序号）；step 与 message 汇总不双计（只读 step 侧）",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "join message.data.$.modelID/providerID（request_field；message-v2.ts Assistant zod 必需字段）",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "part.time_created 是投影写入时刻（≈step 完成，observed_at），非上游精确逐调用时间",
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
            adapter_id: "mimo-code".to_string(),
            product: "MiMo Code（XiaomiMiMo）".to_string(),
            surfaces: vec!["cli".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["MIMOCODE_HOME/data", "$XDG_DATA_HOME/mimocode", "~/.local/share/mimocode"],
                "env_override": ["MIMOCODE_HOME（须绝对路径；非绝对上游抛错，发现层跳过）", "XDG_DATA_HOME"],
                "manual_roots": "数据目录本身 / <root>/data（MIMOCODE_HOME 形状）/ <root>/mimocode / <root>/.local/share/mimocode；不按文件名递归全盘",
                "bounded": true,
                "pattern": "<data>/mimocode*.db（通道变体 mimocode-<channel>.db；-wal/-shm 排除）",
                "profile": "无 profile 概念（固定源码未见）",
            }),
            detection: serde_json::json!({
                "magic": "SQLite + part/session/message 关键列（schema 指纹；message 须含 agent_id 列 ⇒ 与 OpenCode 库互斥，不从 fork 关系推兼容）",
                "version_field": "session.version（库内数值最大者）",
                "registry": "adapters/mimo-code/versions 注册表分派（当前空：文档级证据；与 opencode 注册表独立）",
                "fail_closed": true,
                "unknown_version": "未收录/缺失版本一律 latest_fallback（带兼容标记）",
            }),
            fields,
            lifecycle: serde_json::json!({
                "model_call": "step-finish 部件逐次入账（part.id 身份，time_updated 修订序号）",
                "no_double_count": "只读 part 表；assistant message.data.tokens 是 turn 级聚合不读（A14 step 与 message 汇总不双计）",
                "no_session_counters": "MiMo session 表无 tokens_* 累计列（session.sql.ts）：无会话级对账目标，残差不可见",
                "subagent": "session.parent_id 非空 ⇒ sub_agent；message.agent_id（默认 main）为产品特有列，语义未证实不用于分类",
            }),
            incremental: serde_json::json!({
                "cursor": "schema 指纹 + part.id 稳定键 + time_updated 水位（+60s 有界重叠窗）",
                "row_cap": "单轮 50,000 行；触顶停在最后一个完整毫秒",
                "schema_evolution": "指纹变化 ⇒ 水位重置全量重读（id 键 upsert 幂等）",
                "no_change_detection": "WAL 活库：游标 offset 恒 0，每轮执行水位查询而非字节短路",
                "rescan_note": "in-place 页重写触发框架 Rescan 标记；水位不重置（同一逻辑库）",
            }),
            dedup: serde_json::json!({
                "primary": "mimo-code:part:{part.id}（实例命名空间，与 opencode 分列）",
                "cross_source": "不读 event/同步层与外部导入表；与 OpenCode/kilo 数据根不重叠、注册表独立",
                "rescan": "重扫/重叠窗重复行按同键同修订幂等（unchanged），不双计",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "被删部件/未完成 step 缺证据不计、不补零；MiMo 无 session 累计列，对账缺口不可见（如实标注）",
                "sampling": "未观测到采样；坏 data 行逐条隔离记诊断",
                "source_retention": "源端保留未知；可回填范围以现存行时间为准",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::step_finish_parts_v1::PARSER_VERSION,
                "format_evidence": "固定源码 456678b（session/session.sql.ts、session/message-v2.ts StepFinishPart、shared/global.ts resolveMimocodeHome、storage/db.ts）+ 家族共享模块证据链；合成 fixtures 标注待真实样本",
                "evidence_level": "doc-level（无本机真实样本；2026-09-25 盘点 not_found）",
                "upgrade_policy": "取得真实脱敏 fixture 后逐 session.version 升为已验证；未收录版本 latest_fallback",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan；手动/间隔/监听触发按源合并",
                "incremental_cost": "time_updated 水位查询；每轮固定一次探测查询",
                "pause_cancel": "行级游标可停；busy 源转暂存副本或保留旧结果下轮重试",
            }),
            limitations: vec![
                "文档级证据实现：本机未安装（2026-09-25 盘点 not_found），全部版本 latest_fallback，待真实样本核验后升级".into(),
                "tokens 语义沿用家族 pinned 证据（input=nonCachedInputTokens 等）：OpenCode 侧源码证实，MiMo 侧仅证 shape 同形，数值口径待真实样本复验".into(),
                "session 表无累计列：无会话级对账，逐次合计与上游总量的残差不可见（不虚构对账目标）".into(),
                "message.agent_id 语义（'main' 之外取值）未经真实样本证实，不用于调用分类".into(),
                "MIMOCODE_HOME 非绝对路径时上游抛错：发现层跳过该候选，其余候选继续".into(),
                "step 与 message 汇总不双计：只读 part；turn 级聚合不重复入账".into(),
            ],
        }
    }
}
