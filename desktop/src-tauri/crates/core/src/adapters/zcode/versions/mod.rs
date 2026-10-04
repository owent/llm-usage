//! ZCode 版本注册表：发布版本 → 格式实现的映射与未知版本回退选择
//! （architecture.md#adapter-layout / #unknown-version）。
//!
//! 已验证版本须有真实脱敏 fixture 与期望值核验结果（M0/M4 验证记录）；注册表扩展
//! 只增加条目，不删除历史实现。当前仅一个格式实现 `modelio_v1`
//! （3.14.3 真实 fixture 核验读取、解析、入库与查询：main-session + subagent 双文件）。
//!
//! 版本锚点：每条 `model_io` 记录的 `request.headers["x-zcode-app-version"]`
//!（本机 3.14.3 实读证实；同机日日志另有 `context.schemaVersion=1`、
//! 协议客户端 0.16.9，非本适配器的版本锚点，不参与分派）。
//!
//! 选择规则：
//! - 已收录版本 → `KnownVersion`，按映射分派；
//! - 未收录/缺失版本 → `LatestFallback`，先尝试最新内置解析器，
//!   通过校验的数据带兼容标记入库（active_compat），不因版本号未收录直接拒绝；
//! - ZCode 无已证实不兼容的版本（无 `known_incompatible` 条目）；
//!   结构不兼容在扫描层按 V30 判定（fail closed / 保留旧结果）。

pub mod modelio_v1;

/// 当前格式实现标识（"最新内置解析器"由本常量明确指定，不联网获取）。
pub const LATEST_IMPL_ID: &str = "modelio_v1";

/// 已验证支持的发布版本 → 格式实现。
/// 每个版本都有固定格式样本并已核验；同形版本共用实现，分派仍逐版本登记。
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[
    // M0/M4 真实 fixture（2026-09-25 本机只读提取，8 条记录按两种字段语义人工核算；
    // adapters.md ZCode 行：本机 3.14.3 已核验、跨版本待核验）。
    ("3.14.3", "modelio_v1"),
];

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
        // 版本锚点缺失但 Agent 身份/输入类型已确认：默认回退最新实现。
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
    fn verified_version_dispatches_known() {
        assert_eq!(
            select(Some("3.14.3")),
            Selection {
                impl_id: "modelio_v1",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unrecorded_version_falls_back_to_latest() {
        // synthetic-future-version 场景（9.9.9 未收录）⇒ latest_fallback 兼容尝试。
        assert_eq!(
            select(Some("9.9.9")),
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
