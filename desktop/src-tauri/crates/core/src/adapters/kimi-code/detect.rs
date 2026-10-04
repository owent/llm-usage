//! kimi-code 探测与版本分派：有界读取首行 metadata 头（家族共享指纹），
//! 按 [`super::versions`] 注册表选择格式实现。
//!
//! 约定（architecture.md#unknown-version，V17/V30）：
//! - 首行不是 JSON / 不是 metadata 头 ⇒ 未知格式，fail closed；
//! - protocol_version 已收录（"1.5"）⇒ KnownVersion；
//! - 未收录/缺失 ⇒ LatestFallback（兼容尝试带标记，不直接拒绝）；
//!   kimi-code 尚无已确认不兼容的版本（与 pi 的 v1/v2 拒绝分支不同）。

use crate::adapters::framework::DetectOutcome;
use crate::adapters::kimi_wire::{read_metadata_head, HeadProbe};
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const KIMI_CODE_FORMAT: &str = "kimi-wire-jsonl";

/// 探测一个 wire.jsonl 并按注册表分派（首行 metadata 头是家族共享指纹；
/// 产品身份来自发现根，同一 wire 格式在 kimi-work 侧有独立注册表）。
pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    match read_metadata_head(path)? {
        HeadProbe::Pending => Ok(DetectOutcome::Pending),
        HeadProbe::NotMetadata(reason) => Ok(DetectOutcome::UnknownFormat { reason }),
        HeadProbe::Metadata(head) => {
            let selection = versions::select(head.protocol_version.as_deref());
            Ok(DetectOutcome::Supported {
                format: KIMI_CODE_FORMAT.to_string(),
                format_version: head.protocol_version,
                basis: selection.basis,
            })
        }
    }
}
