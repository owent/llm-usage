//! Junie 版本注册表（V30 目录约定）。
//!
//! events.jsonl 无格式版本字段：注册表锚点是文档级格式版本
//! junie-events-doc-1（第三方解析器 tokscale 固定提交 1d9a939 的字段依据，
//! 闭源产品官方源码不可得），格式偏离在扫描层 fail closed。

pub mod events_doc1;

/// 当前格式实现标识。
pub const LATEST_IMPL_ID: &str = "events_doc1";

/// 文档级格式版本（依据第三方解析器实现）。
pub const JUNIE_FORMAT_VERSION: &str = "junie-events-doc-1";

/// 已验证支持的格式版本 → 格式实现。
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[("junie-events-doc-1", "events_doc1")];

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
            select(Some("junie-events-doc-1")),
            Selection {
                impl_id: "events_doc1",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unknown_or_missing_falls_back() {
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
    }
}
