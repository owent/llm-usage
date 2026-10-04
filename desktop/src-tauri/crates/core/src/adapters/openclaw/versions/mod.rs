//! OpenClaw 版本注册表（形状约定与 kilo/hermes 一致）。
//!
//! A09 官方文档只给出持久层概念（会话行 + 追加式 transcript）与磁盘路径，
//! 未给出任何表名/列名/版本标记；本机 2026-09-25 盘点 not_found，无真实样本。
//! 因此注册表为空、无内置解析器：探测层对全部输入 fail closed（标注待核验），
//! 取得真实脱敏 fixture 后在此登记版本 → 实现映射。

pub mod runtime_store;

/// 当前格式实现标识：占位（无内置解析器；真实样本前 detect 不返回 Supported）。
pub const LATEST_IMPL_ID: &str = "awaiting_real_sample";

/// 已验证支持的版本 → 格式实现。空集：尚未用真实样本核验（见模块头）。
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
