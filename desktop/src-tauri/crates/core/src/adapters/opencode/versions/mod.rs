//! OpenCode 版本注册表：`session.version`（TEXT，写入会话的上游应用版本）→
//! 格式实现映射与未知版本回退选择（architecture.md#adapter-layout /
//! #unknown-version）。
//!
//! 已验证版本须有真实脱敏 fixture 与期望值核验结果。1.18.34 官方 CLI / 本地
//! 模型真实主循环及缓存读已核对；逐记录选择依据，空会话不认证其他记录。
//! 支持更新通过稳定部件键重评旧处理位置，不追加相同用量。
//!
//! 选择规则：
//! - 已收录版本 → `KnownVersion`；
//! - 未收录/缺失版本 → `LatestFallback`，先尝试最新内置解析器
//!   （`step_finish_parts_v1`）；
//! - 已确认不兼容的形状（三表缺失/关键列缺失/仅新 core 派生视图层）在探测层
//!   fail closed，不进入回退。

pub mod step_finish_parts_v1;

/// 当前格式实现标识（"最新内置解析器"由本常量明确指定，不联网获取）。
pub const LATEST_IMPL_ID: &str = "step_finish_parts_v1";

/// 已验证支持的 session.version → 格式实现。
/// 真实脱敏 fixture 与逐记录升级合同共同限定支持范围。
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[("1.18.34", LATEST_IMPL_ID)];

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
    fn registry_only_certifies_the_verified_version() {
        assert_eq!(select(Some("1.18.34")).basis, VersionBasis::KnownVersion);
        assert_eq!(select(Some("1.17.13")).basis, VersionBasis::LatestFallback);
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
    }
}
