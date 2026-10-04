//! Droid 版本注册表（V30 目录约定）。settings.json 无版本字段；
//! 锚点为文档级 droid-settings-doc-1（第三方解析器 tokscale 1d9a939）。

pub mod settings_doc1;

/// 当前格式实现标识。
pub const LATEST_IMPL_ID: &str = "settings_doc1";

/// 文档级格式版本。
pub const DROID_FORMAT_VERSION: &str = "droid-settings-doc-1";

/// 已验证支持的格式版本 → 格式实现。
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[("droid-settings-doc-1", "settings_doc1")];

/// 版本分派结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// 按格式版本选择实现（V30 注册表标准形）。settings.json 当前无版本字段，
/// 探测层固定文档锚点 KnownVersion；本函数为版本字段出现时的分派入口预留。
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
            select(Some("droid-settings-doc-1")).basis,
            VersionBasis::KnownVersion
        );
    }
}
