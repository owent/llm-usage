//! pi 版本注册表：session version → 格式实现的映射与未知版本回退选择
//! （architecture.md#adapter-layout / #unknown-version）。
//!
//! 版本选择依据（固定源码 pi-mono b45597504eeaba1f11a9920a1d1048c361ed4b8e +
//! 本机真实 fixture，原始格式依据见 [`session_v3`] 文件头）：
//! - 已验证：仅 session version 3（固定源码 `CURRENT_SESSION_VERSION=3` +
//!   本机真实 fixture 已核验）→ `session_v3`，KnownVersion；
//! - 已确认不兼容：version 1、2（固定源码证实 v1/v2 落盘格式不同——条目无
//!   id/parentId 字段），取得逐版本 fixture 前不尝试，返回不兼容原因；
//! - version 字段缺失：固定源码表明 v1/v2 时代不写 version 字段，按 legacy
//!   形态处理，同样已确认不兼容（found: None）；
//! - 其余未收录数值（如 4）：默认回退最新内置解析器（LatestFallback），带兼容
//!   标记，不因版本号未收录直接拒绝。
//!
//! 选择规则由 [`select`] 单一事实来源承载，探测（detect）与扫描（scan）共用，
//! 保证两侧策略一致（V30）；注册表扩展只增加条目，不删除历史实现。

pub mod session_v3;

/// 当前格式实现标识（"最新内置解析器"由本常量明确指定，不联网获取）。
pub const LATEST_IMPL_ID: &str = "session_v3";

/// 已验证支持的 session version → 格式实现。
/// 每个版本都有固定源码与 fixture 核验结果；同形版本共用实现，分派仍逐版本登记。
pub const VERIFIED_VERSION_IMPLS: &[(i64, &str)] = &[
    // 固定源码 CURRENT_SESSION_VERSION=3 + 本机真实 fixture（session-error-zero-usage）。
    (3, "session_v3"),
];

/// 已按固定源码确认不兼容的 session version（v1/v2 落盘格式不同：无 id/parentId）。
pub const EVIDENCED_INCOMPATIBLE_VERSIONS: &[i64] = &[1, 2];

/// v1/v2 不兼容原因（取得逐版本 fixture 前不尝试回退）。
pub const REASON_V1_V2_INCOMPATIBLE: &str =
    "pi-mono fixed source: v1/v2 format differs (no id/parentId); per-version fixture 前不尝试";

/// version 字段缺失的原因（v1/v2 时代不写该字段，按 legacy 形态拒绝）。
pub const REASON_LEGACY_MISSING: &str = "pi-mono fixed source: v1/v2 era session headers carry no version field; legacy shape treated as evidenced incompatible";

/// 兼容旧公开常量名（唯一已验证 session version）。
pub const SUPPORTED_SESSION_VERSION: i64 = 3;

/// 版本分派结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// 按来源原始 session version 选择格式实现；探测与扫描共用本函数保证同一策略（V30）。
/// `Err` 为已确认的不兼容原因（调用方 fail closed 并记诊断）。
pub fn select(found: Option<i64>) -> Result<Selection, &'static str> {
    match found {
        Some(version) => {
            if let Some((_, impl_id)) = VERIFIED_VERSION_IMPLS.iter().find(|(v, _)| *v == version) {
                return Ok(Selection {
                    impl_id,
                    basis: crate::domain::VersionBasis::KnownVersion,
                });
            }
            if EVIDENCED_INCOMPATIBLE_VERSIONS.contains(&version) {
                return Err(REASON_V1_V2_INCOMPATIBLE);
            }
            // 未收录数值：默认回退最新内置解析器，带兼容标记，不直接拒绝（V30）。
            Ok(Selection {
                impl_id: LATEST_IMPL_ID,
                basis: crate::domain::VersionBasis::LatestFallback,
            })
        }
        // version 字段缺失：v1/v2 时代不写该字段，按 legacy 形态处理，不尝试回退。
        None => Err(REASON_LEGACY_MISSING),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::VersionBasis;

    #[test]
    fn verified_version_dispatches_known() {
        assert_eq!(
            select(Some(3)),
            Ok(Selection {
                impl_id: "session_v3",
                basis: VersionBasis::KnownVersion
            })
        );
    }

    #[test]
    fn evidenced_incompatible_versions_rejected() {
        for version in [1, 2] {
            assert_eq!(select(Some(version)), Err(REASON_V1_V2_INCOMPATIBLE));
        }
        // version 字段缺失：v1/v2 时代 legacy 形态，同样已确认不兼容。
        assert_eq!(select(None), Err(REASON_LEGACY_MISSING));
    }

    #[test]
    fn unrecorded_version_falls_back_to_latest() {
        assert_eq!(
            select(Some(4)),
            Ok(Selection {
                impl_id: LATEST_IMPL_ID,
                basis: VersionBasis::LatestFallback
            })
        );
    }
}
