//! kimi-work 版本注册表：wire protocol_version → 格式实现的映射与未知版本回退
//! （architecture.md#adapter-layout / #unknown-version，V30）。
//!
//! 版本选择依据（本机实读 + fixture，非官方协议文档）：
//! - 已验证：仅 `1.4`（本机 Kimi Work 内嵌 kimi-code home，2026-09-25 盘点 +
//!   tests/fixtures/kimi-work 真实脱敏样本）→ `wire_v14`，KnownVersion；
//! - 未收录/缺失 protocol_version：尚未确认不兼容 ⇒ LatestFallback 兼容尝试；
//!   注意 `1.5` 是 Kimi Code 侧的已验证锚点，在**本产品**注册表同样走
//!   latest_fallback（两产品注册表独立，A12/A13；同一 wire 家族不共享验证态）。
//!
//! 选择规则由 [`select`] 单一事实来源承载，探测（detect）与扫描（scan）共用。

pub mod wire_v14;

/// 当前格式实现标识（"最新内置解析器"由本常量明确指定，不联网获取）。
pub const LATEST_IMPL_ID: &str = "wire_v14";

/// 已验证 wire protocol_version → 格式实现（逐版本 fixture 登记后再收录）。
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[
    // 本机 Kimi Work（daimon 内嵌 kimi-code home）69 文件实读全为 1.4；
    // 真实脱敏 fixture：conv-main / agent-44-subagent。
    ("1.4", "wire_v14"),
];

/// 版本分派结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// 按来源 wire protocol_version 选择格式实现；探测与扫描共用（V30）。
/// kimi-work 尚无已确认不兼容的版本：未收录/缺失一律 latest_fallback。
pub fn select(found: Option<&str>) -> Selection {
    match found.and_then(|v| VERIFIED_VERSION_IMPLS.iter().find(|(known, _)| *known == v)) {
        Some((_, impl_id)) => Selection {
            impl_id,
            basis: crate::domain::VersionBasis::KnownVersion,
        },
        None => Selection {
            impl_id: LATEST_IMPL_ID,
            basis: crate::domain::VersionBasis::LatestFallback,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::VersionBasis;

    #[test]
    fn verified_version_dispatches_known() {
        assert_eq!(
            select(Some("1.4")),
            Selection {
                impl_id: "wire_v14",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unrecorded_and_missing_versions_fall_back_to_latest() {
        // 1.5 在 kimi-code 已验证、在本产品注册表未收录 ⇒ latest_fallback：
        // 同一 wire 家族的两产品验证态独立（A12/A13）。
        for found in [Some("1.5"), Some("9.9"), None] {
            assert_eq!(
                select(found),
                Selection {
                    impl_id: LATEST_IMPL_ID,
                    basis: VersionBasis::LatestFallback
                }
            );
        }
    }
}
