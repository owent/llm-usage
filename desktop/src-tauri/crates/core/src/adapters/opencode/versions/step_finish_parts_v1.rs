//! opencode.db `part` 表逐 step usage 格式实现（`step_finish_parts_v1`）。
//!
//! 格式证据（A17 固定源码 0027387dc5c59793c12dfc531abc78f825ed6868，
//! 文档级证据待真实样本；本机 not_found）：
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

pub const PARSER_VERSION: &str = "opencode-step-finish-parts-1";

/// 本实现的产品绑定（事件键命名空间 / Agent 名 / 对账开关）。
pub(crate) const PRODUCT: PartProduct = PartProduct {
    ns: "opencode",
    agent: "opencode",
    parser_version: PARSER_VERSION,
    // session.tokens_* 五列在 pinned sql.ts 存在：逐次合计可对账。
    reconcile_session_counters: true,
};

/// 增量扫描一个 opencode.db（统一入口 `OpenCodeAdapter::scan` 分派到本实现）。
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    _limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let source = open_source_db(&target.path, short_probe, &StagingLimits::default())?;
    let conn = source.conn();
    let fingerprint = schema_fingerprint(conn)?;
    // 探测层保证不会走到这里；游标期间库被换掉时按未知格式拒绝，保留旧结果。
    let Some(fingerprint) = fingerprint else {
        return Err(CoreError::Validation(
            "opencode.db schema fingerprint no longer matches; fail closed".to_string(),
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
        },
    )
}
