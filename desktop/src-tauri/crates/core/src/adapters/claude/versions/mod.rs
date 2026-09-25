//! Claude 版本注册表：格式版本 → 格式实现的映射与回退选择
//! （architecture.md#adapter-layout / #unknown-version，V30 目录合同）。
//!
//! claude transcript 无 CLI 版本字段：注册表锚点是文档级格式版本
//! transcript-doc-1（A01 文档口径），因此不存在"未知版本"状态——
//! detect 不读版本号，成功即 KnownVersion；格式偏离（未文档化记录 type、
//! 载体外 usage 字段）在扫描层 fail closed，不走版本回退。
//!
//! 选择规则（保持与 codex 相同的 select 函数形状，便于 V30 结构检查统一断言）：
//! - 文档级锚点 → `KnownVersion`，按映射分派；
//! - None / 其他值 → `LatestFallback` 形状返回；对 claude 实际不可达
//!   （detect 不读版本号，固定传文档级锚点），仅为注册表形状统一保留。

pub mod transcript_doc1;

/// 当前格式实现标识（"最新内置解析器"由本常量明确指定，不联网获取）。
pub const LATEST_IMPL_ID: &str = "transcript_doc1";

/// 文档级格式版本（非 CLI 版本）：transcript 条目格式官方明示不稳定，
/// 本适配器按 A01 文档口径实现，待真实样本核验。
pub const CLAUDE_FORMAT_VERSION: &str = "transcript-doc-1";

/// 已验证支持的格式版本 → 格式实现。
/// claude 无逐 CLI 版本登记：唯一锚点是文档级格式版本 transcript-doc-1。
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[("transcript-doc-1", "transcript_doc1")];

/// 版本分派结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// 按格式版本选择实现；探测与扫描共用本函数保证同一策略（V30）。
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
                // 对 claude 不可达：detect 固定传文档级锚点，无其他版本可查。
                None => Selection {
                    impl_id: LATEST_IMPL_ID,
                    basis: crate::domain::VersionBasis::LatestFallback,
                },
            }
        }
        // 对 claude 不可达：detect 不读版本号，缺失分支保持统一形状。
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
            select(Some("transcript-doc-1")),
            Selection {
                impl_id: "transcript_doc1",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unreachable_branches_keep_unified_shape() {
        // None/其他值分支对 claude 实际不可达（detect 不读版本号）；
        // 仅保持与其他 Agent 注册表相同的 LatestFallback 形状（V30 结构检查）。
        assert_eq!(
            select(None),
            Selection {
                impl_id: LATEST_IMPL_ID,
                basis: VersionBasis::LatestFallback
            }
        );
        assert_eq!(
            select(Some("other")),
            Selection {
                impl_id: LATEST_IMPL_ID,
                basis: VersionBasis::LatestFallback
            }
        );
    }
}
