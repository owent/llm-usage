//! opencode.db `part` 表逐 step usage 格式实现（`step_finish_parts_v1`）。
//!
//! 格式依据（A17 固定源码 0027387dc5c59793c12dfc531abc78f825ed6868，
//! 与 1.18.34（aec0b9a6）真实脱敏 fixture 核验）：
//! - 逐次 usage 载体：`part` 行 `data.type="step-finish"` 且 `cost`+`tokens`
//!   在场（projector.ts `usage()` 提取规则），`tokens{input, output,
//!   reasoning, cache{read, write}}`（vendored client 类型含可选 `total`）；
//! - `session.tokens_*` 五列 = Σ 当前 step-finish 部件（`applyUsage` 增量
//!   维护，删行补偿；迁移 20260510033149 从 message.data 逐字段回填）：
//!   仅作对账，不逐次相加入账（A17）；
//! - assistant `message.data.tokens` 是 turn 级聚合（不读，防与 step 双计）；
//!   message 仅取 `modelID`/`providerID` 归属。
//!
//! 解析核心在家族共享模块 [`crate::adapters::opencode_family`]（与 MiMo Code
//! 经两产品 pinned 源码证实同形的部分）；本模块只绑定产品身份常量与
//! 只读打开路径。

use crate::adapters::framework::{ScanLimits, ScanOutcome, ScanTarget, StoredScanState};
use crate::adapters::opencode::common::{
    open_source_db, schema_fingerprint, short_probe, StagingLimits,
};
use crate::adapters::opencode_family::{scan_step_finish_parts, PartProduct};
use crate::error::CoreError;

pub const PARSER_VERSION: &str = "opencode-step-finish-parts-2";

/// 本实现的产品绑定（事件键命名空间 / Agent 名 / 对账开关）。
pub(crate) const PRODUCT: PartProduct = PartProduct {
    ns: "opencode",
    agent: "opencode",
    parser_version: PARSER_VERSION,
    // session.tokens_* 五列在 pinned sql.ts 存在：逐次合计可对账。
    reconcile_session_counters: true,
    default_zero_unknown: false,
};

/// 增量扫描一个 opencode.db（统一入口 `OpenCodeAdapter::scan` 分派到本实现）。
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
            "opencode.db schema fingerprint no longer matches; fail closed".to_string(),
        ));
    };
    let (db_version, basis) = crate::adapters::opencode::detect::usage_version_summary(conn)?
        .unwrap_or((None, crate::domain::VersionBasis::LatestFallback));
    scan_step_finish_parts(
        conn,
        target,
        stored,
        now_ms,
        crate::adapters::opencode_family::PartScanContext {
            product: PRODUCT,
            fingerprint,
            db_version,
            basis,
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
