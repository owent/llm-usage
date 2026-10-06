//! OpenClaw 版本注册表（形状约定与 kilo/hermes 一致）。
//!
//! Official 2026.9.8 native schema 24 and two real local CLI turns are verified.
//! Mutable schema_meta.app_version cannot certify historical client versions.

pub mod runtime_store;

pub const LATEST_IMPL_ID: &str = "runtime_store_schema24";

/// No immutable per-record client version is available.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[];

/// 版本分派结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// 按来源原始版本选择格式实现；探测与扫描共用（V30）。
/// 当前一律 LatestFallback（注册表为空）。
pub fn select(found: Option<&str>) -> Selection {
    let known = found.and_then(|version| {
        VERIFIED_VERSION_IMPLS
            .iter()
            .find(|(v, _)| *v == version)
            .map(|(_, impl_id)| *impl_id)
    });
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::VersionBasis;

    #[test]
    fn registry_is_empty_so_everything_is_latest_fallback() {
        assert!(VERIFIED_VERSION_IMPLS.is_empty());
        assert_eq!(select(Some("1")).basis, VersionBasis::LatestFallback);
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
    }
}
