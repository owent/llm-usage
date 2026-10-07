//! Claude 版本注册表：格式版本 → 格式实现的映射与回退选择
//! （architecture.md#adapter-layout / #unknown-version，V30 目录约定）。
//!
//! 原生 2.1.197 每条 assistant 自带 version；逐条绑定，不以文件首行
//! 或安装版本认证历史。无版本的旧文档锚点单独保留，其他版本兼容读取。

pub mod transcript_doc1;

/// 当前格式实现标识（"最新内置解析器"由本常量明确指定，不联网获取）。
pub const LATEST_IMPL_ID: &str = "transcript_doc1";

/// 文档级格式版本（非 CLI 版本）：transcript 条目格式官方明示不稳定，
/// 旧实现按 A01 文档定义；该锚点不等于 CLI 版本核验。
pub const CLAUDE_FORMAT_VERSION: &str = "transcript-doc-1";

/// 已验证支持的格式版本 → 格式实现。
/// 保留旧文档锚点，原生版本只登记已核验的 2.1.197。
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[
    ("transcript-doc-1", "transcript_doc1"),
    ("2.1.197", "transcript_doc1"),
];

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
                // 未核验原生版本仅兼容读取。
                None => Selection {
                    impl_id: LATEST_IMPL_ID,
                    basis: crate::domain::VersionBasis::LatestFallback,
                },
            }
        }
        // 未给定选择依据时保持兼容标记。
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
    fn unknown_branches_keep_unified_shape() {
        // 未登记原生版本不升级为真实支持。
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
