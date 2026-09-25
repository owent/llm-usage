//! Codex 版本注册表：发布版本 → 格式实现的映射与未知版本回退选择
//! （architecture.md#adapter-layout / #unknown-version）。
//!
//! 已验证版本须有真实脱敏 fixture 与期望值证据（m2a 验证记录；本机历史版本
//! 逐版本补验后登记）；注册表扩展只增加条目，不删除历史实现。
//! 当前仅一个格式实现 `rollout_v1`（0.155.0-alpha.16.3 真实 fixture 全链路核验）。
//!
//! 选择规则：
//! - 已收录版本 → `KnownVersion`，按映射分派；
//! - 未收录/缺失版本 → `LatestFallback`，先尝试最新内置解析器，
//!   通过校验的数据带兼容标记入库，不因版本号未收录直接拒绝；
//! - Codex 无已证实不兼容的版本（无 `known_incompatible` 条目）；
//!   结构不兼容在扫描层按 V30 判定并保留旧结果。

pub mod rollout_v1;

/// 当前格式实现标识（"最新内置解析器"由本常量明确指定，不联网获取）。
pub const LATEST_IMPL_ID: &str = "rollout_v1";

/// 已验证支持的发布版本 → 格式实现。
/// 每个版本都有固定格式样本证据；同形版本共用实现，分派仍逐版本登记。
/// 0.139–0.151 系列本机实测无 token_usage_record（仅 token_count 累计快照），
/// 与 rollout_v1 的逐次载体不同，待专用实现取证后登记；在此之前按
/// LatestFallback 尝试并在扫描层判不兼容（可见诊断，不伪造数据）。
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[
    // M2-A 真实 fixture（3 会话，49 调用全链路期望）。
    ("0.155.0-alpha.16.3", "rollout_v1"),
    // M2-D 逐版本脱敏 fixture（rollout-v*.sanitized.json + _expectations.md
    // jq 独立核算；token_usage_record 逐次载体与 0.155 同形，共用 rollout_v1）。
    ("0.154.0-alpha.6.2", "rollout_v1"),
    ("0.154.0-alpha.6.1", "rollout_v1"),
    ("0.153.0", "rollout_v1"),
];

/// 兼容公开路径的旧常量名（tests/examples 引用）。
pub const SUPPORTED_CLI_VERSIONS: &[&str] = &["0.155.0-alpha.16.3"];

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
    fn verified_versions_dispatch_known() {
        assert_eq!(
            select(Some("0.155.0-alpha.16.3")),
            Selection {
                impl_id: "rollout_v1",
                basis: VersionBasis::KnownVersion
            }
        );
        // M2-D 逐版本 fixture 核验的同形版本。
        for v in ["0.153.0", "0.154.0-alpha.6.1", "0.154.0-alpha.6.2"] {
            assert_eq!(
                select(Some(v)),
                Selection {
                    impl_id: "rollout_v1",
                    basis: VersionBasis::KnownVersion
                }
            );
        }
        // 0.139–0.151 系列无逐次载体证据，仍按未知版本回退，不预登已验证。
        assert_eq!(select(Some("0.142.5")).basis, VersionBasis::LatestFallback);
    }

    #[test]
    fn unrecorded_version_falls_back_to_latest() {
        assert_eq!(
            select(Some("0.199.0-alpha.1")),
            Selection {
                impl_id: LATEST_IMPL_ID,
                basis: VersionBasis::LatestFallback
            }
        );
        assert_eq!(
            select(None),
            Selection {
                impl_id: LATEST_IMPL_ID,
                basis: VersionBasis::LatestFallback
            }
        );
    }
}
