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
//!   ~/.local/share/opencode; Windows 2.0.32 native projections were checked separately.
//!   WAL databases use opencode.db or the channel variant opencode-<channel>.db.
//! - Per-step usage comes from part rows of type step-finish; see family module references.
//! - The earlier research note that exact per-request mapping was unimplemented was
//!   corrected after source inspection: step-finish carries usage per step and drives upstream projector counts.
//!   Map each part row; exact call timestamps and model-switch order require further event-level data.
//!   Timestamp basis remains observed_at.
//! - v2 reads session_message with session_v2 ownership, native completion times,
//!   unknown default zeros and compaction observations; inherited fork rows are excluded.

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
        let source = common::open_source_db(
            &target.path,
            common::short_probe,
            &common::StagingLimits::default(),
        )?;
        if versions::session_messages_v2::matches_schema(source.conn())?
            && source
                .conn()
                .query_row("SELECT EXISTS(SELECT 1 FROM session_message)", [], |r| {
                    r.get::<_, bool>(0)
                })?
        {
            if common::schema_fingerprint(source.conn())?.is_some()
                && source
                    .conn()
                    .query_row("SELECT EXISTS(SELECT 1 FROM part)", [], |r| {
                        r.get::<_, bool>(0)
                    })?
            {
                return Err(crate::error::CoreError::Validation(
                    "OpenCode legacy/v2 projection overlap is unverified".into(),
                ));
            }
            return versions::session_messages_v2::scan(target, stored, limits, now_ms);
        }
        versions::step_finish_parts_v1::scan(target, stored, limits, now_ms)
    }

    fn scan_format(
        &self,
        outcome: &crate::adapters::framework::ScanOutcome,
    ) -> Option<crate::adapters::framework::DetectOutcome> {
        let context = outcome.parse_context.as_ref()?;
        if context["policy"] == versions::session_messages_v2::PARSER_VERSION {
            return Some(crate::adapters::framework::DetectOutcome::Supported {
                format: versions::session_messages_v2::FORMAT.into(),
                format_version: None,
                basis: crate::domain::VersionBasis::LatestFallback,
            });
        }
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

    fn has_validated_usage(&self, outcome: &crate::adapters::framework::ScanOutcome) -> bool {
        !outcome.events.is_empty()
            || outcome
                .parse_context
                .as_ref()
                .is_some_and(|value| value["validated_usage"] == true)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let awaiting = "1.18.34 官方 CLI / 本地模型主循环及缓存读已核对；2.0.32 Windows 原生投影已核对，未独立核验其 Provider/API；其他版本及未落盘调用另验".to_string();
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
                "v1 part.data.tokens；v2 session_message.data.tokens：input 为未缓存输入，output 为正文输出，reasoning 为独立输出。v2 正值正文输出可与 reasoning 相加还原输出总量；默认零明细仍未知。输入缺少缓存明细时只显示已知下界，不表示调用未识别。原始 usage 未必保存在本地，重新扫描不能补回未保存字段",
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
                "v1 每条 step-finish、v2 每条已完成 assistant 各计一次已观测 step；v2 compaction 可合并多次尝试，仅计 usage_observation。失败调用无 usage 仍计次数；未落盘调用不补造",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "v1 join message.data.modelID/providerID；v2 读取当前消息 data.model.id/providerID；均为 request_field，缺失不从会话当前模型推断",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "v1 part.time_created 是投影写入时刻（observed_at）；v2 原生 data.time.completed（source_completion），无完成时间的 compaction 使用 data.time.created（source_start）",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "原生 cost 为客户端估算（estimated micro-USD）；v2 仅正值，默认零未知，不代表账单费用",
            ),
        );
        fields.insert(
            "latency".into(),
            field(
                Availability::Partial("仅 v2 原生起止时间差".into()),
                "v2 completed-created 正值为 duration；TTFT 未读取；v1 不提供延迟",
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
                "windows_note": "2.0.32 Windows 默认 ~/.local/share/opencode 的真实 SQLite/WAL 投影已只读核对",
                "profile": "无 profile 概念（固定源码未见）",
            }),
            detection: serde_json::json!({
                "magic": "SQLite：v1 part/session/message 关键列与累计列；v2 session_v2/session_message 关键列及 fork 归属；两个非空布局并存时拒绝猜测重叠关系",
                "version_field": "v1 所属 session.version；v2 所属 session_v2.version，均逐记录保留，空会话不认证 usage",
                "registry": "adapters/opencode/versions 仅登记 1.18.34；按 step-finish 所属会话选择，混合版本/空会话/旧游标升级已回归，其他版本继续 latest_fallback",
                "fail_closed": true,
                "unknown_version": "未收录/缺失版本保留 latest_fallback 依据；结构完整且读取正常时不显示兼容警告，真实字段/归属错误仍显示",
            }),
            fields,
            lifecycle: serde_json::json!({
                "model_call": "v1 step-finish、v2 完成的 assistant；稳定记录 ID 与 time_updated 修订；v2 compaction 是用量观测",
                "no_double_count": "v1 只读 part；v2 只读 session_message 并排除 fork 继承前缀；不叠加 session 累计或事件溯源表",
                "cumulative_snapshot": "session.tokens_* 五列 = Σ 当前部件（projector applyUsage 含删行补偿），仅作会话对账",
                "subagent": "session.parent_id 非空 ⇒ sub_agent；子会话是独立 session 行（sql.ts parent_id 列）",
                "part_removal": "上游 PartRemoved/MessageRemoved 会扣减 session 计数（-sign 路径）；被删部件行消失后不再入账，靠对账 mismatch 可见",
            }),
            incremental: serde_json::json!({
                "cursor": "v1 time_updated/part.id 元组与 60s 重叠窗；v2 按消息 ID 分页循环复查完整投影，早期 ID 的修改在后续循环重读",
                "row_cap": "单轮最多 50,000 行并遵守 max_lines；v2 错误状态仅在整轮完成后替换，分页不掩盖旧错误",
                "schema_evolution": "指纹变化 ⇒ 处理位置重置全量重读（id 键 upsert 幂等）",
                "no_change_detection": "WAL 下主库文件长度不变不代表内容未变：游标 offset 恒 0，每轮执行处理位置查询而非字节短路",
                "rescan_note": "in-place 页重写触发框架 Rescan 标记；处理位置不重置（同一逻辑库）",
            }),
            dedup: serde_json::json!({
                "primary": "v1 opencode:part:{part.id}；v2 opencode:v2:{message.id}（实例命名空间）",
                "cross_source": "事件溯源表不叠加；其他派生产品根与注册表独立，同名/同形库仍需产品来源归属；v1/v2 同时非空的迁移库待单独核验",
                "rescan": "重扫/重叠窗重复行按同键同修订幂等（unchanged），不双计",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "1.18.34 默认标题生成未进入 step-finish 与会话累计，matched 不证明全部 API 调用被覆盖；被删部件/未完成 step 缺证据不计、不补零",
                "sampling": "未观测到采样；坏 data 行逐条隔离记诊断",
                "source_retention": "源端保留未知；可回填范围以现存行时间为准",
            }),
            maintenance: serde_json::json!({
                "parser_version": "opencode-dispatch-4",
                "format_evidence": "固定源码 0027387 + 1.18.34/aec0b9a6 的 sql/projector/processor/prompt 与真实脱敏 fixtures；API usage/CLI/原生库/应用独立核对",
                "evidence_level": "real-local：1.18.34 主循环/缓存读；2.0.32 Windows 原生投影 7 次 assistant 与统计库备份核对。v2 Provider/API 和实际 fork/compaction 另验",
                "upgrade_policy": "逐记录按 session.version 认证；支持或规则升级重评未变化的旧处理位置，完整有效重评标记完成；空会话及库内最高版本不认证其他记录",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan；手动/间隔/监听触发按源合并",
                "incremental_cost": "v1 time_updated 窗口；v2 有界完整投影循环，每页读取最多 50,000 行",
                "pause_cancel": "行级游标可停；busy 源转暂存副本或保留旧结果下轮重试",
            }),
            limitations: vec![
                "1.18.34 Linux CLI 有真实 API/原生库对照；2.0.32 Windows 仅核验原生投影到统计库，不认证所有 Provider/API。未登记版本保留 latest_fallback 依据，正常读取不警告".into(),
                "默认标题生成未进入 step-finish：真实 API 2 次/848 token，原生载体及应用仅 1 次/299 token；明确标题的对照为 1 次/299 token，不能替代默认覆盖结论".into(),
                "tokens 五互斥口径已在 1.18.34 本地模型核对（未缓存 295 + 缓存读 3 = API 输入 298）；其他 provider/版本、缓存写及非零 reasoning 尚待真实核验".into(),
                "逐调用精确时间与模型切换序待上游 event 层（A17）：part 行时间标 observed_at，模型归属经 message join".into(),
                "v2 fork 继承依据缺失时不猜测归属；v1/v2 两套投影同时非空的迁移库不自动叠加".into(),
                "v2 默认零未知，compaction 可合并尝试而不产生调用次数；真实 fork/compaction、其他平台与通道布局未在本轮验收".into(),
                "被删除的源记录不可恢复；历史已采集事件按保留策略保留，不因源投影删除而抹去".into(),
            ],
        }
    }
}
