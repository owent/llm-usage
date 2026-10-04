//! Xum 版本注册表（V30 目录约定）。session-usage.json 的版本演化尚未核验
//! （version 字段为整数内容版本）；锚点为文档级 xum-session-usage-doc-1
//! （第三方解析器 tokscale 1d9a939 字段依据 + 官方开源仓库线索）。

pub mod usage_v1;

/// 当前格式实现标识。
pub const LATEST_IMPL_ID: &str = "usage_v1";

/// 文档级格式版本。
pub const XUM_FORMAT_VERSION: &str = "xum-session-usage-doc-1";

/// 已验证支持的格式版本 → 格式实现。
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[("xum-session-usage-doc-1", "usage_v1")];

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
    fn doc_anchor_dispatches_known() {
        assert_eq!(
            select(Some("xum-session-usage-doc-1")).basis,
            VersionBasis::KnownVersion
        );
    }
}
