//! Copilot CLI 版本注册表（V30 目录约定）。
//!
//! 锚点：session-store.db `schema_version` 表（本机实测 8）+ M0 核验 1.0.73
//! 逐 turn 字段。schema_version=8 → KnownVersion；其他/缺失 → LatestFallback
//! 兼容尝试（列集在探测层校验）。

pub mod usage_events_v8;

/// 当前格式实现标识。
pub const LATEST_IMPL_ID: &str = "usage_events_v8";

/// 已验证的格式版本（schema_version=8，真实数据核对 2026-09-29）。
pub const COPILOT_FORMAT_VERSION: &str = "assistant-usage-events-v8";

/// 已验证支持的格式版本 → 格式实现。
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] =
    &[("assistant-usage-events-v8", "usage_events_v8")];

/// 版本分派结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// 按格式版本选择实现；探测与扫描共用本函数（V30）。
pub fn select(found: Option<&str>) -> Selection {
    match found {
        Some(version) => {
            let known = VERIFIED_VERSION_IMPLS
                .iter()
                .find(|(v, _)| *v == version)
                .map(|(_, impl_id)| *impl_id);
            match known {
                Some(impl_id) => Selection {
                    impl_id,
                    basis: crate::domain::VersionBasis::KnownVersion,
                },
                None => Selection {
                    impl_id: LATEST_IMPL_ID,
                    basis: crate::domain::VersionBasis::LatestFallback,
                },
            }
        }
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
    fn v8_dispatches_known() {
        assert_eq!(
            select(Some("assistant-usage-events-v8")),
            Selection {
                impl_id: "usage_events_v8",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unknown_or_missing_falls_back() {
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
        assert_eq!(select(Some("v9")).basis, VersionBasis::LatestFallback);
    }
}
