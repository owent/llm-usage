//! OpenCode 版本注册表：`session.version`（TEXT，写入会话的上游应用版本）→
//! 格式实现映射与未知版本回退选择（architecture.md#adapter-layout /
//! #unknown-version）。
//!
//! 已验证版本须有真实脱敏 fixture 与期望值证据。当前**没有**任何本机真实
//! 样本（2026-09-25 盘点 not_found，m0-agent-fixtures.md），仅有固定源码
//! （0027387dc5c59793c12dfc531abc78f825ed6868）文档级证据，因此注册表为空：
//! 一切版本走 `LatestFallback`（带兼容标记，数据照常入库，compat=unverified）。
//! 取得真实 fixture 后逐版本升级为 KnownVersion。
//!
//! 选择规则：
//! - 已收录版本 → `KnownVersion`（当前为空集）；
//! - 未收录/缺失版本 → `LatestFallback`，先尝试最新内置解析器
//!   （`step_finish_parts_v1`）；
//! - 有证据的不兼容形状（三表缺失/关键列缺失/仅新 core 投影层）在探测层
//!   fail closed，不进入回退。

pub mod step_finish_parts_v1;

/// 当前格式实现标识（"最新内置解析器"由本常量明确指定，不联网获取）。
pub const LATEST_IMPL_ID: &str = "step_finish_parts_v1";

/// 已验证支持的 session.version → 格式实现。
/// 空集：文档级证据阶段（见模块头），真实样本核验前不登记任何版本。
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
        // 版本字段缺失（空 session 表已由探测层 Pending 拦截；此分支防御
        // 会话行 version 异常空值）：默认回退最新实现。
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
        assert_eq!(select(Some("1.17.13")).basis, VersionBasis::LatestFallback);
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
    }
}
