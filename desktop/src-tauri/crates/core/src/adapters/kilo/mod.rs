//! Kilo Code CLI 适配器（独立目录合同 architecture.md#adapter-layout）：
//! - 本模块是该 Agent 的稳定入口（统一接口实现与再导出）；
//! - [`detect`]：kilo.db schema 指纹探测与版本分派；
//! - [`versions`]：已验证格式实现的注册与映射，未知版本默认回退最新内置解析器；
//! - [`common`]：产品特有 usage 映射（全互斥口径）+ 源库只读/暂存副本合同。
//!
//! 原始格式证据见各模块文件头（真实脱敏 fixture session-7.4.8-edges /
//! session-7.4.9-family + 2026-09-25 本机只读 SELECT 探查）；kilo 是 opencode
//! 派生（A11），仅按本目录证实的 message/session 两表解析，不共享 OpenCode 的
//! 目录、表名或累计假设。

pub mod common;
pub mod detect;
pub mod versions;

pub use common::{map_kilo, KiloUsage};
pub use detect::KILO_FORMAT;
pub use versions::{message_tokens_v1, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// Kilo Code 适配器（无状态）。
pub struct KiloAdapter;

impl Default for KiloAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl KiloAdapter {
    pub fn new() -> Self {
        KiloAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for KiloAdapter {
    fn adapter_id(&self) -> &'static str {
        "kilo"
    }

    fn agent(&self) -> &'static str {
        "kilo-code"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        // 默认根：Unix 布局 ~/.local/share/kilo（Windows 上同样是
        // %USERPROFILE%/.local/share/kilo，本机实测，用 home_dir 推导）。
        // kilo 无官方 env 覆盖（A11），不做 env 项。
        if let Some(home) = &ctx.home_dir {
            roots.push((
                home.join(".local").join("share").join("kilo"),
                RootBasis::DefaultHome,
            ));
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out: Vec<DiscoveredRoot> = Vec::new();
        let mut seen: std::collections::BTreeSet<std::path::PathBuf> =
            std::collections::BTreeSet::new();
        for (root, basis) in roots {
            if !root.is_dir() {
                continue;
            }
            // 手工根语义宽松：接受 kilo home 本身、其父目录或用户 home
            //（如直接传入 ~）。有界（深度 3）按文件名定位 kilo.db，
            // 不递归整盘。找到的每个 kilo.db 的父目录即一个实例根。
            let files = crate::adapters::framework::enumerate_files_bounded(&root, 3, &|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n == "kilo.db")
                    .unwrap_or(false)
            });
            for file in files {
                let Some(parent) = file.parent() else {
                    continue;
                };
                if !seen.insert(parent.to_path_buf()) {
                    continue;
                }
                out.push(DiscoveredRoot {
                    root: parent.to_path_buf(),
                    basis: basis.clone(),
                    files: vec![file],
                });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "kilo@{}",
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
        // 当前所有已验证版本共用 message_tokens_v1；注册表扩展多实现后在此按选择分派。
        versions::message_tokens_v1::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
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
                Availability::Available,
                "message.data.tokens 五字段全互斥（in+out+reason+cache.read+cache.write=total；fixture 与本机实读 13,342/13,342 成立）",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(Availability::Available, "tokens.cache.read reported"),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial(
                    "真实样本 cache.write 全为 0；字段存在且按互斥口径映射".into(),
                ),
                "tokens.cache.write reported",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Available,
                "每条 role=assistant 的 message 是一次模型调用；message.id 稳定身份，time_updated 为更新序号",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Available,
                "assistant 消息自带 modelID/providerID（request_field）；缺失 unknown",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Available,
                "data.time.completed 优先（source_completion）；未完成退 created（source_start）",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Unavailable(
                    "message.data.cost 全零且 session.cost 列存的是模型 JSON（实读）；不可用"
                        .into(),
                ),
                "无",
            ),
        );
        fields.insert(
            "latency".into(),
            field(
                Availability::Available,
                "time.completed - time.created 派生（毫秒）；缺 completed 为空",
            ),
        );
        CapabilityTable {
            adapter_id: "kilo".to_string(),
            product: "Kilo Code CLI / 桌面".to_string(),
            surfaces: vec!["cli".into(), "desktop".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["<home>/.local/share/kilo"],
                "env_override": null,
                "manual_roots": true,
                "bounded": true,
                "pattern": "<kilo home>/kilo.db（手工根深度 3 内按文件名定位）",
                "profile": "无 profile 概念",
            }),
            detection: serde_json::json!({
                "magic": "SQLite + message/session 两表关键列（schema 指纹）",
                "version_field": "session.version（库内数值最大者）",
                "registry": "adapters/kilo/versions 注册表分派",
                "fail_closed": true,
                "unknown_version": "未收录/缺失版本先尝试最新内置解析器（latest_fallback）；仅存新 core session_message 数据层时 fail closed 待取证",
            }),
            fields,
            lifecycle: serde_json::json!({
                "model_call": "assistant 消息 tokens（message.id 身份，time_updated 修订序号）",
                "cumulative_snapshot": "session 行 tokens_* 五列仅按五列合计对账，不映射为 request 事件（A11）；实读 276 会话 275 对上，7.4.8/7.4.9 期列名与值错位（列级语义随版本不稳定）",
                "unfinished": "无 finish/error/total 的消息记 partial；完成后同键替换为 final",
                "aborted": "data.error 或 finish=error ⇒ error_status；已返回部分按 tokens 记账（fixture 实证全零）",
                "retries": "格式内未观测到 transport 重试记录",
                "subagent": "session.parent_id 非空 ⇒ sub_agent；子会话是独立 session 行，与主会话同库无跨表双计",
            }),
            incremental: serde_json::json!({
                "cursor": "schema 指纹 + message.id 稳定键 + time_updated 水位（+60s 有界重叠窗）",
                "row_cap": "单轮 50,000 行；触顶停在最后一个完整毫秒",
                "schema_evolution": "指纹变化 ⇒ 水位重置全量重读（id 键 upsert 幂等）",
                "no_change_detection": "WAL 下主库文件长度不变不代表内容未变：游标 offset 恒 0，每轮执行水位查询而非字节短路",
                "rescan_note": "in-place 页重写改变文件头触发框架 Rescan 标记；水位不重置（同一逻辑库）",
            }),
            dedup: serde_json::json!({
                "primary": "kilo:msg:{message.id}（实例命名空间）",
                "cross_source": "opencode-rc.db / 新 core 数据层（session_message）不在本适配器范围；未读不相加",
                "rescan": "重扫/重叠窗重复行按同键同修订幂等（unchanged），不双计",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "error_calls": "data.error/finish=error 的 assistant 消息照常记账（fixture 实证 tokens 全零）",
                "hidden_calls": "删除/压缩的消息无法从本表恢复（缺证据不猜）；session_message 聚合层当前 0 行未接入",
                "sampling": "未观测到采样；坏 data 行逐条隔离记诊断",
                "source_retention": "源端保留未知；可回填范围以现存行时间为准（本机最早 2026-06）",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::message_tokens_v1::KILO_PARSER_VERSION,
                "format_evidence": "真实脱敏 fixture（7.4.8 edges / 7.4.9 family）+ 本机只读 SELECT（版本 7.3.42–7.7.12，13,342 assistant）",
                "upgrade_policy": "未收录版本 latest_fallback 兼容尝试；逐版本 fixture 核验后升为已验证",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan；手动/间隔/监听触发按源合并",
                "incremental_cost": "time_updated 水位查询（无索引时全表扫行头）；每轮固定一次探测查询",
                "pause_cancel": "行级游标可停；busy 源转暂存副本或保留旧结果下轮重试",
            }),
            limitations: vec![
                "session 行 tokens_* 列级语义随版本不稳定（7.4.8/7.4.9 错位、新版本一一对应）：仅五列合计参与对账".into(),
                "仅解析 fixture 证实的 message/session 两表；新 core 数据层（session_message/part/event）未接入，切换后需专用实现".into(),
                "缺证据的删除/压缩消息不推测：本表查不到的调用不计、不补零；实读有 1/276 会话快照与明细不吻合（mismatch 诊断可见）".into(),
                "WAL 活库：主库文件字节身份（首采样含 change counter）会因 checkpoint 变化，导致框架层文件身份重建与一次全量重读（幂等，不双计）".into(),
                "无 message 行索引 time_updated 时水位查询走全表行头扫描；超大库首轮全量读受单轮 50,000 行与 30s 预算约束".into(),
                "IDE 扩展变体缺独立证据（F1 后移），本适配器仅覆盖 CLI/桌面落盘的 kilo.db".into(),
            ],
        }
    }
}
