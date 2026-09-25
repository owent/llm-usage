//! kimi-work wire JSONL 格式实现（`wire_v14`，protocol_version 1.4）。
//!
//! 格式证据（本机 Kimi Work 内嵌 kimi-code home 实读 2026-09-25；
//! 真实脱敏 fixture：tests/fixtures/kimi-work/{conv-main,agent-44-subagent}）：
//! - 路径：`…/Kimi/share/daimon-share/daimon/runtime/kimi-code/home/sessions/
//!   <wd_hash>/<conv-<hexid>|ctitle-<uuid>>/agents/<agent>/wire.jsonl`
//!   （宿主 daimon，state.json createdBy=daimon-kernel-adapter 佐证产品身份）；
//! - `usage.record{model, usage{inputOther,output,inputCacheRead,
//!   inputCacheCreation}, usageScope: turn|session, time}`——与 1.5 的差异：
//!   **无 agentId 字段**（代理身份来自 agents/<id>/ 目录）、model 为裸 id；
//! - 其余家族合同（回声去重、subagent 快照、毫秒时间、四互斥字段）与 1.5
//!   实读一致 ⇒ 解析逻辑在 [`crate::adapters::kimi_wire`] 共享；
//!   1.4 特有记录类型（tools.register_user_tool、permission.
//!   record_approval_result、micro_compaction.apply 等）在共享忽略清单内。
//!
//! 本模块只绑定产品身份常量（kimi-work 命名空间与 Agent 名，统计分列）。

use crate::adapters::framework::{ScanLimits, ScanOutcome, ScanTarget, StoredScanState};
use crate::adapters::kimi_wire::{scan_wire, WireProduct};
use crate::error::CoreError;

pub const PARSER_VERSION: &str = "kimi-wire-14-1";

/// 本实现的产品绑定（kimi-work 命名空间与 Agent 名）。
pub(crate) const PRODUCT: WireProduct = WireProduct {
    ns: "kimi-work",
    agent: "kimi-work",
    parser_version: PARSER_VERSION,
};

/// 增量扫描一个 wire.jsonl（统一入口 `KimiWorkAdapter::scan` 分派到本实现；
/// 版本注册表分派由 [`super::super::versions`] 与探测共用）。
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    scan_wire(target, stored, limits, now_ms, &PRODUCT, &|found| {
        super::select(found).basis
    })
}
