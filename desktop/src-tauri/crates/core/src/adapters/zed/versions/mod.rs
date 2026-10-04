//! Zed 版本注册表：格式版本 → 格式实现的映射与回退选择
//! （architecture.md#adapter-layout / #unknown-version，V30 目录约定）。
//!
//! threads.db 无产品版本字段：注册表锚点是文档级格式版本
//! zed-threads-db-1（官方源码 bd74733 建表/迁移 SQL 与 data blob 结构），
//! schema 偏离在探测/扫描层 fail closed，不走版本回退。

pub mod threads_db_v1;

/// 当前格式实现标识（"最新内置解析器"由本常量明确指定）。
pub const LATEST_IMPL_ID: &str = "threads_db_v1";

/// 文档级格式版本：按官方源码 bd74733 的 threads 表 + DbThread data blob 定义实现。
pub const ZED_FORMAT_VERSION: &str = "zed-threads-db-1";

/// 已验证支持的格式版本 → 格式实现。
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[("zed-threads-db-1", "threads_db_v1")];

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
            select(Some("zed-threads-db-1")),
            Selection {
                impl_id: "threads_db_v1",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unknown_or_missing_falls_back_to_latest() {
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
        assert_eq!(select(Some("other")).basis, VersionBasis::LatestFallback);
    }
}
