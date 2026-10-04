//! MiMo Code 版本注册表：`session.version`（TEXT）→ 格式实现映射与未知版本
//! 回退选择（architecture.md#adapter-layout / #unknown-version）。
//!
//! 已验证版本须有真实脱敏 fixture 与期望值核验结果。当前**没有**任何本机真实
//! 样本（2026-09-25 盘点 not_found），仅有固定源码
//! （456678b6a5afb0eef3fe2754575637218cfb3c84）文档或源码依据，因此注册表为空：
//! 一切版本走 `LatestFallback`（带兼容标记，数据照常入库，compat=unverified）。
//! 注册表与 OpenCode 目录独立（adapters.md A14：不因内核同名合并锚点）。

pub mod step_finish_parts_v1;

/// 当前格式实现标识（"最新内置解析器"由本常量明确指定，不联网获取）。
pub const LATEST_IMPL_ID: &str = "step_finish_parts_v1";

/// 已验证支持的 session.version → 格式实现。
/// 空集：尚未用真实样本核验（见模块头），核验完成前不登记任何版本。
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[];

/// 版本分派结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// 按来源原始版本选择格式实现；探测与扫描共用本函数保证同一策略（V30）。
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
    fn registry_is_empty_so_everything_is_latest_fallback() {
        assert!(VERIFIED_VERSION_IMPLS.is_empty());
        assert_eq!(select(Some("0.1.0")).basis, VersionBasis::LatestFallback);
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
    }
}
