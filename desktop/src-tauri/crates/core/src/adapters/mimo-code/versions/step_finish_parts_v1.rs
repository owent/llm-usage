//! mimocode.db `part` 表逐 step usage 格式实现（`step_finish_parts_v1`）。
//!
//! 格式依据（A14 固定源码 456678b6a5afb0eef3fe2754575637218cfb3c84，
//! 按文档或源码实现，待真实样本核验；本机 not_found）：
//! - 逐次 usage 载体：`part` 行 `data.type="step-finish"`（message-v2.ts
//!   `StepFinishPart` zod：tokens{total?, input, output, reasoning,
//!   cache{read, write}} + cost）；
//! - assistant `message.data.tokens` 是 turn 级聚合（不读，防与 step 双计，
//!   adapters.md A14）；message 仅取 `modelID`/`providerID` 归属
//!   （Assistant zod 必需字段）；
//! - MiMo `session` 表无 tokens_* 累计列（OpenCode 才有）⇒ 无会话级对账目标。
//!
//! 解析核心在家族共享模块 [`crate::adapters::opencode_family`]（与 OpenCode
//! 经两产品 pinned 源码证实同形的部分）；本模块只绑定产品身份常量。

use crate::adapters::framework::{ScanLimits, ScanOutcome, ScanTarget, StoredScanState};
use crate::adapters::mimo_code::common::{
    open_source_db, schema_fingerprint, short_probe, StagingLimits,
};
use crate::adapters::opencode_family::{scan_step_finish_parts, PartProduct};
use crate::error::CoreError;

pub const PARSER_VERSION: &str = "mimo-code-step-finish-parts-2";

/// 本实现的产品绑定（事件键命名空间 / Agent 名 / 对账开关）。
pub(crate) const PRODUCT: PartProduct = PartProduct {
    ns: "mimo-code",
    agent: "mimo-code",
    parser_version: PARSER_VERSION,
    // MiMo session 表无 tokens_* 累计列（session.sql.ts）：无对账目标。
    reconcile_session_counters: false,
    default_zero_unknown: true,
};

/// 增量扫描一个 mimocode.db（统一入口 `MimoCodeAdapter::scan` 分派到本实现）。
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let source = open_source_db(&target.path, short_probe, &StagingLimits::default())?;
    let conn = source.conn();
    let _sql = crate::adapters::run_policy::SqliteScope::new(conn)?;
    let fingerprint = schema_fingerprint(conn)?;
    // 探测层保证不会走到这里；游标期间库被换掉时按未知格式拒绝，保留旧结果。
    let Some(fingerprint) = fingerprint else {
        return Err(CoreError::Validation(
            "mimocode.db schema fingerprint no longer matches; fail closed".to_string(),
        ));
    };
    let db_version = crate::adapters::opencode_family::max_session_version(conn)?;
    let selection = super::select(db_version.as_deref());
    scan_step_finish_parts(
        conn,
        target,
        stored,
        now_ms,
        crate::adapters::opencode_family::PartScanContext {
            product: PRODUCT,
            fingerprint,
            db_version,
            basis: selection.basis,
            record_basis: |version| super::select(version).basis,
            row_limit: limits
                .jsonl
                .max_lines
                .unwrap_or(crate::adapters::opencode_family::MAX_ROWS_PER_ROUND as u64)
                .min(crate::adapters::opencode_family::MAX_ROWS_PER_ROUND as u64)
                as i64,
        },
    )
}
