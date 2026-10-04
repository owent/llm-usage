//! Hermes 版本注册表：state.db `schema_version`（INTEGER）→ 格式实现的映射
//! 与未知版本回退选择（architecture.md#adapter-layout / #unknown-version）。
//!
//! 版本标记是 state.db `schema_version` 表的单行整数（固定源码
//! hermes_state_common.py SCHEMA_VERSION=30；探测时另验真实列与主键形状，
//! 不只看整数）。
//!
//! 已验证版本须有真实脱敏 fixture 与期望值核验结果。当前**没有**任何本机真实
//! 样本（2026-09-25 盘点 not_found，m0-agent-fixtures.md），仅有固定源码/
//! 官方文档依据，因此注册表为空：一切版本走 `LatestFallback`
//! （带兼容标记，数据照常入库，compat=unverified）。取得真实 fixture 后
//! 逐版本升级为 KnownVersion。
//!
//! 选择规则：
//! - 已收录版本 → `KnownVersion`（当前为空集）；
//! - 未收录/缺失版本（含 schema_version 表缺失）→ `LatestFallback`，
//!   先尝试最新内置解析器（`session_model_usage_v1`）；
//! - 已确认不兼容的形状（缺表/缺列/pre-v22 主键）在探测层 fail closed，
//!   不进入回退。

pub mod session_model_usage_v1;

/// 当前格式实现标识（"最新内置解析器"由本常量明确指定，不联网获取）。
pub const LATEST_IMPL_ID: &str = "session_model_usage_v1";

/// 已验证支持的 schema_version → 格式实现。
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
        // 版本字段缺失但 Agent 身份/输入类型已确认：默认回退最新实现。
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
        // 尚未用真实样本核验：固定源码 schema_version=30 也未升为已验证。
        assert!(VERIFIED_VERSION_IMPLS.is_empty());
        assert_eq!(
            select(Some("30")),
            Selection {
                impl_id: "session_model_usage_v1",
                basis: VersionBasis::LatestFallback
            }
        );
        assert_eq!(select(Some("22")).basis, VersionBasis::LatestFallback);
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
    }
}
