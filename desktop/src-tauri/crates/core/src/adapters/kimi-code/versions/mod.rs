//! kimi-code 版本注册表：wire protocol_version → 格式实现的映射与未知版本回退
//! （architecture.md#adapter-layout / #unknown-version，V30）。
//!
//! 版本选择依据（本机实读 + fixture，非官方协议文档）：
//! - 已验证：仅 `1.5`（本机 Kimi Code desktop 1.0.3，M0 fixture +
//!   tests/fixtures/kimi-code 真实脱敏样本）→ `wire_v15`，KnownVersion；
//! - 未收录/缺失 protocol_version：尚未确认不兼容 ⇒ 默认回退最新内置解析器
//!   （LatestFallback，带兼容标记），不因版本号未收录直接拒绝；
//!   注意 1.4 是 Kimi Work 侧的已验证锚点，在**本产品**注册表中同样走
//!   latest_fallback（两产品注册表独立，A12/A13）。
//!
//! 选择规则由 [`select`] 单一事实来源承载，探测（detect）与扫描（scan）共用。

pub mod wire_v15;

/// 当前格式实现标识（"最新内置解析器"由本常量明确指定，不联网获取）。
pub const LATEST_IMPL_ID: &str = "wire_v15";

/// 已验证 wire protocol_version → 格式实现（逐版本 fixture 登记后再收录）。
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[
    // 本机 desktop 1.0.3 实读（M0 + M4 fixture：session-main / subagent-agent-0）。
    ("1.5", "wire_v15"),
    ("1.4", "wire_v15"), // 同形 wire 格式：kimi-work 侧 1.4 已验证，本产品本机 active_compat 文件证实同构
];

/// 版本分派结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// 按来源 wire protocol_version 选择格式实现；探测与扫描共用（V30）。
/// kimi-code 尚无已确认不兼容的版本：未收录/缺失一律 latest_fallback。
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
            select(Some("1.5")),
            Selection {
                impl_id: "wire_v15",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unrecorded_and_missing_versions_fall_back_to_latest() {
        // 尚未确认不兼容的未收录/缺失版本：兼容尝试（V30），不直接拒绝。
        // 1.4 现已注册（本机 active_compat 文件证实同构 wire_v15）。
        for found in [Some("9.9"), None] {
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
