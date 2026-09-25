//! OpenClaw 运行时库（openclaw-agent.sqlite）解析占位。
//!
//! A09 官方文档（store 参考）确认每 Agent 一个
//! `~/.openclaw/agents/<agentId>/agent/openclaw-agent.sqlite`，含会话行
//! （token counters 等可变运行态）与追加式 transcript（含 usage 测量）两个
//! 持久层；token-use 参考确认 assistant transcript 条目持久化规范化 usage
//! 形状（input/output 别名归一、total 缺失回退 input+output、usage.cost）。
//! **文档未给出表名/列名/时间字段名**（research.md A09：具体表和兼容版本
//! 待验），本机无真实样本（2026-09-25 盘点 not_found）。
//!
//! 因此本模块不解析任何表（禁止猜字段）：detect 对运行时库 fail closed
//! （结构可读、身份由文档路径形状确认，表级 schema 待真实样本取证后在此
//! 实现读取映射）。scan 入口防御性拒绝——框架只会把 Supported 文件交给
//! scan，当前探测层不产生 Supported。

use crate::adapters::framework::{ScanLimits, ScanOutcome, ScanTarget, StoredScanState};
use crate::error::CoreError;

pub const OPENCLAW_PARSER_VERSION: &str = "openclaw-runtime-store-awaiting-sample-0";

pub fn scan(
    _target: &ScanTarget,
    _stored: &StoredScanState,
    _limits: &ScanLimits,
    _now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    Err(CoreError::Validation(
        "openclaw scan reached without a supported format; detect must have failed closed \
         (table-level schema awaiting real sample)"
            .to_string(),
    ))
}
