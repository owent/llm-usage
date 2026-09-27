//! ZCode 适配器（独立目录合同 architecture.md#adapter-layout）：
//! - 本模块是该 Agent 的稳定入口（统一接口实现与再导出）；
//! - [`detect`]：产品/格式探测与版本分派；
//! - [`versions`]：已验证格式实现的注册与映射（版本锚点
//!   `request.headers["x-zcode-app-version"]`；已验证 3.14.3，未知版本
//!   latest_fallback 兼容尝试）；
//! - [`common`]：双口径 usage 映射（AI SDK 主口径 + anthropic 对照口径，
//!   从根级 usage_map.rs 下沉）；
//! - [`db_backfill`]：数据库逐次记录为主来源，按来源/日原子替换；
//! - [`db_reconciliation`]：轮级累计只用于对账，不与逐次记录相加。
//!
//! 发现依据（m345-inventory-2026-09-25 路径证据，本机实读）：
//! 数据根 `<home>/.zcode/cli`；用量逐次记录在 `rollout/model-io-*.jsonl`；
//! `db/db.sqlite` 的 model_usage 用于采集，turn_usage 用于对照；
//! `agents/*/transcript.jsonl` 为正文类（不计量）；`~/.zcode/v2` 无 rollout；
//! `%APPDATA%/zcode` 为桌面端 session 小存储（未接入）。无文档化环境覆盖。

pub mod common;
pub mod db_backfill;
pub mod detect;
pub mod versions;

pub use common::{map_zcode_ai_sdk, map_zcode_anthropic, ZcodeAiSdkUsage, ZcodeAnthropicUsage};
pub use detect::ZCODE_FORMAT;
pub use versions::{modelio_v1, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

use rusqlite::Connection;

/// ZCode 适配器（无状态）。
pub struct ZcodeAdapter;

impl Default for ZcodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ZcodeAdapter {
    pub fn new() -> Self {
        ZcodeAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for ZcodeAdapter {
    fn adapter_id(&self) -> &'static str {
        "zcode"
    }

    fn agent(&self) -> &'static str {
        "zcode"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        // 本机归属默认根：<home>/.zcode/cli（inventory 实读证实；不硬编码盘符）。
        if let Some(home) = &ctx.home_dir {
            roots.push((home.join(".zcode").join("cli"), RootBasis::DefaultHome));
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out = Vec::new();
        for (root, basis) in roots {
            let rollout = root.join("rollout");
            let db = root.join("db").join("db.sqlite");
            if !rollout.is_dir() && !db.is_file() {
                continue;
            }
            // rollout/model-io-<sessionId>.jsonl：深度 1，有界枚举。
            let mut files =
                crate::adapters::framework::enumerate_files_bounded(&rollout, 1, &|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with("model-io-") && n.ends_with(".jsonl"))
                        .unwrap_or(false)
                });
            if db.is_file() {
                files.push(db);
            }
            if !files.is_empty() {
                out.push(DiscoveredRoot { root, basis, files });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "zcode@{}",
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
        // 当前所有已验证版本共用 modelio_v1；注册表扩展多实现后在此按选择分派。
        versions::modelio_v1::scan(target, stored, limits, now_ms)
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
        fields.insert("tokens".into(), field(Availability::Available, "AI SDK response.usage 五键直报（inputTokens 含缓存读）；input_uncached/total 派生；anthropic 对照口径互斥校验"));
        fields.insert(
            "cache_read".into(),
            field(Availability::Available, "cacheReadTokens reported"),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial(
                    "真实样本全 0；>0 场景由合成 fixture 覆盖（cacheWriteTokens reported）".into(),
                ),
                "cacheWriteTokens reported",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Available,
                "数据库按 model_usage.id 识别已完成调用；没有数据库的来源才使用 JSONL requestId+attempt",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Available,
                "记录自带 model.modelId / model.providerId；缺失 unknown",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Available,
                "completedAt ISO8601 UTC 毫秒（source_completion）；epoch 数值防御折算",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Unavailable("本地无费用字段；远端账单/账号不接入".into()),
                "无",
            ),
        );
        fields.insert(
            "latency".into(),
            field(Availability::Available, "durationMs 逐次直报"),
        );
        CapabilityTable {
            adapter_id: "zcode".to_string(),
            product: "ZCode CLI".to_string(),
            surfaces: vec!["cli".into()],
            supported_versions: VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["<home>/.zcode/cli"],
                "env_override": null,
                "manual_roots": true,
                "bounded": true,
                "pattern": "rollout/model-io-<sessionId>.jsonl",
                "profile": "无 profile 概念（~/.zcode/v2 为另一布局，无 rollout）",
            }),
            detection: serde_json::json!({
                "magic": "首行 JSONL type=model_io 且 sessionId 必填",
                "version_field": "request.headers[\"x-zcode-app-version\"]",
                "registry": "adapters/zcode/versions 注册表分派",
                "fail_closed": true,
                "unknown_version": "未收录/缺失版本先尝试最新内置解析器（latest_fallback），通过校验的数据带兼容标记统计（active_compat）",
            }),
            fields,
            lifecycle: serde_json::json!({
                "model_call": "model_usage 已完成记录（native id 身份）；JSONL 仅作从未使用过 DB 的来源回退",
                "in_flight_tail": "finishReason=null 且无 usage/providerMetadata：正常形状，不产事件、不失败",
                "retries": "attempt 字段入身份（zcode:{requestId}:{attempt}）；本机实读 attempt 恒 1，>1 未观测",
                "subagent": "querySource=subagent ⇒ sub_agent（独立 model-io 文件）",
                "auxiliary": "querySource=session_title ⇒ auxiliary（db.model_usage 实读观测到该来源），照常计入",
                "category_unknown": "querySource 超出已证值域 ⇒ unknown + 一次性诊断，不猜",
            }),
            incremental: serde_json::json!({
                "cursor": "DB 使用来源/日快照摘要，JSONL 使用文件身份 + generation + 完整行偏移",
                "rewrite_detection": ["截断", "同长替换", "改名重探测", "重建（创建时间变化）"],
                "budget": "单源每轮 30s 初值；单行 8 MiB；单块 4 MiB",
                "half_line": "半行不前移游标",
            }),
            dedup: serde_json::json!({
                "primary": "zcodedb:{model_usage.id}（实例命名空间）；JSONL 回退为 zcode:{requestId}:{attempt}",
                "fallback": "seq:{sessionId}:{行号}（缺 requestId，已验证替代）",
                "dual_caliber": "AI SDK 与 anthropic 双口径互斥取一，绝不相加；矛盾进诊断、主口径保留",
                "cross_source": "同一来源优先 model_usage；整来源/日替换 JSONL 贡献，不靠相近时间和 token 猜测关联。使用过 DB 的来源在 DB 缺失时保留统计并报错，不退回 JSONL",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "未知；model-io JSONL 只含已完成模型调用记录",
                "sampling": "未观测到采样；坏行/坏记录逐条隔离记诊断",
                "source_retention": "本机安装包 recordModelUsage 按 started_at 清理 30 天前的记录；保留已采集旧日，边界日不以部分快照替换",
                "prompt_content": "只读白名单字段（type/attempt/sessionId/requestId/turnId/traceId/querySource/model/时间/durationMs/x-zcode-app-version 头/response.usage 与 providerMetadata.anthropic 的数值），正文/请求体/响应文本不提取",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::modelio_v1::ZCODE_PARSER_VERSION,
                "format_evidence": "M0 本机 fixture（3.14.3，main-session + subagent 双文件双口径人工核算）",
                "upgrade_policy": "未收录版本 latest_fallback 兼容尝试；逐版本真实 fixture 核验后升为已验证",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan；手动/间隔/监听触发按源合并",
                "incremental_cost": "DB 读取有界快照，未变化日不重写；JSONL 无变化文件探测短路",
                "pause_cancel": "文件间可停；单轮预算有界",
            }),
            limitations: vec![
                "跨版本仅 3.14.3 已验证；未收录版本数据带 latest_fallback 标记统计".into(),
                "真实样本 cacheWriteTokens 全 0；cache_write⊆input 以合成样本与 AI SDK 语义为据"
                    .into(),
                "双口径矛盾（dual_caliber_mismatch）保留 AI SDK 主口径，不自动择值".into(),
                "DB 没有逐行产品版本，结构校验后按 latest_fallback 记录兼容状态；"
                    .into(),
                "model-io JSONL 为滚动窗口：子代理会话文件被 ZCode 即时删除、主文件反复压实丢旧记录；"
                    .into(),
                "JSONL 与 DB 均会清理历史；来源删除且从未采集的调用无法恢复"
                    .into(),
                "子 Agent 无父会话字段：parent_session_id 不猜测（保持 None）".into(),
                "缺 requestId 记录用 sessionId+行号身份，文件同位替换后可能形成新键".into(),
                "符号链接/junction 不跟随；Windows 无稳定文件索引号，身份靠创建时间+首采样".into(),
            ],
        }
    }

    fn scan_archive(
        &self,
        storage: &crate::storage::Storage,
        root: &crate::adapters::framework::DiscoveredRoot,
        config: &crate::adapters::framework::RunConfig,
    ) -> Result<Option<crate::ingest::BatchOutcome>, crate::error::CoreError> {
        let instance = self.instance_id(root);
        let path = root.root.join("db").join("db.sqlite");
        if !path.is_file() {
            let authoritative: bool = storage.conn().query_row(
                "SELECT EXISTS(SELECT 1 FROM settings WHERE key=?1)",
                [db_backfill::authority_key(&instance)],
                |r| r.get(0),
            )?;
            if authoritative {
                return Err(crate::error::CoreError::Validation(
                    "ZCode usage database is unavailable; previous statistics are preserved. JSONL fallback is disabled to prevent duplicate counting.".into()));
            }
            return Ok(None);
        }
        let out = db_backfill::zcode_db_backfill(
            storage,
            &path,
            &instance,
            &config.timezone,
            config.now_ms,
        )?;
        Ok(Some(crate::ingest::BatchOutcome {
            added: out.added as i64,
            updated: out.updated as i64,
            unchanged: out.matched_existing as i64,
            skipped: 0,
            errors: 0,
            conflicts: 0,
            data_revision: storage.data_revision()?,
            affected_days: vec![],
        }))
    }
}

/// cli/db/db.sqlite 只读对账结果（白名单数值，无 ID/路径/正文）。
///
/// 证据（本机 3.14.3 只读探测）：`model_usage` 逐次行（4169 行）、
/// `turn_usage` 逐轮聚合（438 行），computed_total_tokens 可对账；
/// M0 样本 16/16 轮相等；活库快照上少量在途/取消轮存在差异，
/// 对账只报告 matched/mismatch，不做修正、不入库计量。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbReconciliation {
    pub model_rows: i64,
    pub turn_rows: i64,
    pub turns_total: i64,
    /// Σmodel_usage.computed_total_tokens == turn_usage.computed_total_tokens 的轮数。
    pub turns_matched: i64,
    /// 有 model_usage 行但合计不等的轮数（活库快照下含在途/取消轮）。
    pub turns_mismatched: i64,
    /// 无任何 model_usage 行的轮数。
    pub turns_without_model_rows: i64,
    pub model_computed_total_sum: i64,
    pub turn_computed_total_sum: i64,
}

/// 只读打开 cli/db/db.sqlite 并做 Σmodel_usage == turn_usage 对账
///（M0 结论 turn_usage==Σmodel_usage 的持续核验）。失败返回错误，不写库。
pub fn db_reconciliation(
    db_path: &std::path::Path,
) -> Result<DbReconciliation, crate::error::CoreError> {
    let flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
        | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX
        | rusqlite::OpenFlags::SQLITE_OPEN_URI;
    let conn = Connection::open_with_flags(db_path, flags)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    let snapshot = conn.unchecked_transaction()?;
    let (model_rows, model_computed_total_sum): (i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(computed_total_tokens), 0) FROM model_usage",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let (turn_rows, turn_computed_total_sum): (i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(computed_total_tokens), 0) FROM turn_usage",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let (turns_total, turns_without, turns_mismatched): (i64, i64, i64) = conn.query_row(
        "SELECT COUNT(*),
                COALESCE(SUM(CASE WHEN m.s IS NULL THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN m.s IS NOT NULL
                          AND t.computed_total_tokens != m.s THEN 1 ELSE 0 END), 0)
         FROM turn_usage t
         LEFT JOIN (
           SELECT session_id, turn_id, SUM(computed_total_tokens) AS s
           FROM model_usage GROUP BY session_id, turn_id
         ) m ON m.session_id = t.session_id AND m.turn_id = t.turn_id",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    snapshot.commit()?;
    Ok(DbReconciliation {
        model_rows,
        turn_rows,
        turns_total,
        turns_matched: turns_total - turns_without - turns_mismatched,
        turns_mismatched,
        turns_without_model_rows: turns_without,
        model_computed_total_sum,
        turn_computed_total_sum,
    })
}
