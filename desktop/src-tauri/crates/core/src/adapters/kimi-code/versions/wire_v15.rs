//! kimi-code wire JSONL 格式实现（`wire_v15`，protocol_version 1.5）。
//!
//! 格式依据（本机实读 2026-09-24/25，Kimi Code desktop 1.0.3；
//! 真实脱敏 fixture：tests/fixtures/kimi-code/{session-main,subagent-agent-0}）：
//! - 路径：`KIMI_CODE_HOME/sessions/<wd_hash>/session_<uuid>/agents/<agent>/wire.jsonl`
//!   （默认 `~/.kimi-code`）；首行 `metadata{protocol_version:"1.5", created_at}`；
//! - `usage.record{agentId, model, usage{inputOther,output,inputCacheRead,
//!   inputCacheCreation}, usageScope: turn|session, time}`：逐次计账源；
//! - `context.append_loop_event.event(step.end).usage` 是回声（uuid/messageId/
//!   延迟字段只存在于回声侧，与记录侧无关联键）⇒ 不计账、仅对账；
//! - `subagent.completed.usage` = 子代理 wire 截至 completed.time 的 Σ 快照，
//!   不产事件（防双计）；
//! - usageScope=session 出现在 full_compaction 区间（压缩摘要调用）⇒ auxiliary。
//!
//! 解析逻辑全部在家族共享模块 [`crate::adapters::kimi_wire`]（kimi-code 与
//! kimi-work 经真实数据测试证明一致的部分）；本模块只绑定产品身份常量。

use crate::adapters::framework::{ScanLimits, ScanOutcome, ScanTarget, StoredScanState};
use crate::adapters::kimi_wire::{scan_wire, WireProduct};
use crate::error::CoreError;

pub const PARSER_VERSION: &str = "kimi-wire-15-1";

/// 本实现的产品绑定（kimi-code 命名空间与 Agent 名）。
pub(crate) const PRODUCT: WireProduct = WireProduct {
    ns: "kimi-code",
    agent: "kimi-code",
    parser_version: PARSER_VERSION,
};

/// 增量扫描一个 wire.jsonl（统一入口 `KimiCodeAdapter::scan` 分派到本实现；
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
