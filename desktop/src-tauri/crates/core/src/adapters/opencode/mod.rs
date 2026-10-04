//! OpenCode 适配器（独立目录约定 architecture.md#adapter-layout）：
//! - 本模块是该 Agent 的稳定入口（统一接口实现与再导出）；
//! - [`detect`]：opencode.db schema 指纹（产品互斥）+ session.version 注册表；
//! - [`versions`]：格式注册表（当前空：文档或源码依据，全部 latest_fallback）；
//! - [`common`]：产品互斥 schema 指纹 + 源库只读/暂存副本约定；
//! - wire 解析核心在家族共享模块 [`crate::adapters::opencode_family`]。
//!
//! 格式依据（A17 固定源码 0027387dc5c59793c12dfc531abc78f825ed6868，
//! 按文档或源码实现，待真实样本核验；本机 2026-09-25 盘点 not_found）：
//! - 路径：xdg-basedir 的 opencode 数据目录（`$XDG_DATA_HOME/opencode`，
//!   缺省 `~/.local/share/opencode`；Windows 布局未经真实样本核验）下
//!   `opencode.db`（安装通道变体 `opencode-<channel>.db`），WAL；
//! - 逐次 usage：`part` 表 step-finish 部件（family 模块头证据链）；
//! - 调研备注"精确逐请求映射尚未实现"经 pinned 源码核验修正为：
//!   step-finish 部件携带逐 step usage（projector 计数即由其推导），
//!   按部件行逐次映射；逐调用的精确时间/模型切换序仍待 event 层，
//!   时间依据如实标 observed_at。

pub mod common;
pub mod detect;
pub mod versions;

pub use detect::OPENCODE_FORMAT;
pub use versions::step_finish_parts_v1;
pub use versions::{LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// OpenCode 适配器（无状态）。
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
        // xdg-basedir 数据目录（固定源码 global.ts：data = xdgData/opencode）。
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
        // 发现范围按**数据目录名**限定（不按文件名全盘递归）：opencode*.db 只在
        // 名为 opencode 的数据目录内接受。手工根兼容三种形状：数据目录本身、
        // <root>/opencode、<root>/.local/share/opencode——防止把 kilo 等同血统
        // 产品目录里的 opencode-rc.db（同名前缀、schema 同形）误认为本产品。
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
        // 当前所有可尝试版本共用 step_finish_parts_v1；注册表扩展后在此分派。
        versions::step_finish_parts_v1::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let awaiting =
            "文档级证据（A17 固定源码 0027387）；本机 not_found（2026-09-25 盘点），待真实样本核验"
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
                "每条 step-finish 部件 = 一次 step 的模型调用（part.id 稳定身份，time_updated 修订序号）；调研备注'精确逐请求映射尚未实现'经 pinned 源码核验修正（projector 计数即由部件推导）",
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
                "version_field": "session.version（库内数值最大者）",
                "registry": "adapters/opencode/versions 注册表分派（当前空：文档级证据）",
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
                "cursor": "schema 指纹 + part.id 稳定键 + time_updated 水位（+60s 有界重叠窗）",
                "row_cap": "单轮 50,000 行；触顶停在最后一个完整毫秒",
                "schema_evolution": "指纹变化 ⇒ 水位重置全量重读（id 键 upsert 幂等）",
                "no_change_detection": "WAL 下主库文件长度不变不代表内容未变：游标 offset 恒 0，每轮执行水位查询而非字节短路",
                "rescan_note": "in-place 页重写触发框架 Rescan 标记；水位不重置（同一逻辑库）",
            }),
            dedup: serde_json::json!({
                "primary": "opencode:part:{part.id}（实例命名空间）",
                "cross_source": "event 表（事件溯源层）与 session_message 投影不读；kilo 等其他 opencode 派生产品数据根不重叠、注册表独立——kilo 目录内的 opencode-rc.db 与本库 schema 同形，默认/父目录发现不触碰（数据目录名收敛），手工根直指该目录时仅靠来源归属区分",
                "rescan": "重扫/重叠窗重复行按同键同修订幂等（unchanged），不双计",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "被删部件/未完成 step 缺证据不计、不补零；session 计数与部件合计的残差经对账 mismatch 可见",
                "sampling": "未观测到采样；坏 data 行逐条隔离记诊断",
                "source_retention": "源端保留未知；可回填范围以现存行时间为准",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::step_finish_parts_v1::PARSER_VERSION,
                "format_evidence": "固定源码 0027387（session/sql.ts、projector.ts、迁移 20260510033149、vendored client StepFinishPart/AssistantMessage 类型）+ 家族共享模块证据链；合成 fixtures 标注待真实样本",
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
                "tokens 语义取自 pinned 源码（input=nonCachedInputTokens、inclusive inputTokens=in+cr+cw、total=in+out+reason+cr+cw）：直报 total 与派生不一致时记诊断，真实样本复验前数值口径未落地".into(),
                "逐调用精确时间与模型切换序待上游 event 层（A17）：part 行时间标 observed_at，模型归属经 message join".into(),
                "仅解析 part/session/message 三表；仅存新 core session_message 投影层的库 fail closed 待专用实现".into(),
                "Windows 默认目录（xdg-basedir 语义）与通道变体库文件名未经真实样本核验".into(),
                "step 与 message 汇总不双计：只读 part；被删部件在源表消失后不可恢复，残差靠对账可见".into(),
            ],
        }
    }
}
