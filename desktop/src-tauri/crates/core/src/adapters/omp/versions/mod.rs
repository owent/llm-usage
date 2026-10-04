//! omp 版本注册表：session 数值版本 → 格式实现的映射与未知版本回退选择
//! （architecture.md#adapter-layout / #unknown-version）。
//!
//! 已验证版本须经真实样本核验：本机 58/58 会话头实测全部 `version`=3（18.2.7 scoop，
//! 2026-08-21 至 2026-09-24）+ 固定源码 oh-my-pi 62bc57b 佐证。注册表扩展只增加
//! 条目，不删除历史实现。当前仅一个格式实现 `session_v3`。
//!
//! 选择规则（与 pi 不同，已核验范围不同）：
//! - 已收录版本 → `KnownVersion`，按映射分派；
//! - 未收录数值版本 / `version` 字段缺失 → `LatestFallback`，先尝试最新内置
//!   解析器，通过校验的数据带兼容标记入库，不因版本号未收录直接拒绝；
//! - 依据：pi 的 v1/v2 已按固定源码确认不兼容（pi 注册表有 evidenced-incompatible
//!   分支）；omp 旧版落盘格式尚未核验（尚未确认格式不同），故无 `known_incompatible`
//!   条目，未收录与缺失都回退 `session_v3` 尝试；结构不兼容在扫描层按 V30
//!   判定（读到记录、零事件且带结构诊断 ⇒ 保留旧结果）。

pub mod session_v3;

/// 当前格式实现标识（"最新内置解析器"由本常量明确指定，不联网获取）。
pub const LATEST_IMPL_ID: &str = "session_v3";

/// 已验证支持的 session 数值版本 → 格式实现。
/// 每个版本都经真实样本核验；同形版本共用实现，分派仍逐版本登记。
pub const VERIFIED_VERSION_IMPLS: &[(i64, &str)] = &[
    // 本机 58/58 会话头实测 version=3 + oh-my-pi 62bc57b 固定源码。
    (3, "session_v3"),
];

/// 版本分派结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// 按来源原始 session 数值版本选择格式实现；探测与扫描共用本函数保证同一策略（V30）。
pub fn select(found: Option<i64>) -> Selection {
    let known = found.and_then(|v| {
        VERIFIED_VERSION_IMPLS
            .iter()
            .find(|(recorded, _)| *recorded == v)
            .map(|(_, impl_id)| *impl_id)
    });
    match known {
        Some(impl_id) => Selection {
            impl_id,
            basis: crate::domain::VersionBasis::KnownVersion,
        },
        // 未收录数值版本或 version 字段缺失：Agent 身份/输入类型已确认，默认回退
        // 最新实现（omp 旧版格式尚未核验，尚未确认不兼容，不直接拒绝）。
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
    fn verified_versions_dispatch_known() {
        assert_eq!(
            select(Some(3)),
            Selection {
                impl_id: "session_v3",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unrecorded_version_falls_back_to_latest() {
        assert_eq!(
            select(Some(4)),
            Selection {
                impl_id: LATEST_IMPL_ID,
                basis: VersionBasis::LatestFallback
            }
        );
        assert_eq!(
            select(Some(2)),
            Selection {
                impl_id: LATEST_IMPL_ID,
                basis: VersionBasis::LatestFallback
            }
        );
        // version 字段缺失（legacy 形状）同样回退尝试，不直接拒绝（与 pi 不同：
        // pi v1 无 version 字段已按固定源码确认不兼容，omp 尚未核验）。
        assert_eq!(
            select(None),
            Selection {
                impl_id: LATEST_IMPL_ID,
                basis: VersionBasis::LatestFallback
            }
        );
    }
}
