//! DSH 版本注册表：格式版本 → 格式实现的映射与回退选择
//! （architecture.md#adapter-layout / #unknown-version，V30 目录约定）。
//!
//! rc.2 原生格式 4 已有非空真实样本；旧 session-log-doc-1 仍为独立文档锚点。
//! KnownVersion 只认证载体格式，不认证全部客户端版本或继承历史。
//! select 保留统一回退形状；实际 detect 拒绝未知明示格式版本。

pub mod session_log_doc1;
pub mod session_v4;

/// 当前格式实现标识（"最新内置解析器"由本常量明确指定，不联网获取）。
pub const LATEST_IMPL_ID: &str = "session_v4";

/// 文档级格式版本（非产品版本）：按固定 README 46a7f68 的事件与替换语义实现，
/// 待真实样本核验。
pub const DSH_FORMAT_VERSION: &str = "session-log-doc-1";

/// 已验证支持的格式版本 → 格式实现。
/// 无逐产品版本登记；原生格式 4 与旧文档锚点分别保存。
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[
    ("session-log-doc-1", "session_log_doc1"),
    ("4", "session_v4"),
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
                // 实际 detect 已拒绝未知明示格式；保留统一选择接口。
                None => Selection {
                    impl_id: LATEST_IMPL_ID,
                    basis: crate::domain::VersionBasis::LatestFallback,
                },
            }
        }
        // 实际 detect 要求原生 header 或旧指纹，缺失分支保持统一形状。
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
            select(Some("session-log-doc-1")),
            Selection {
                impl_id: "session_log_doc1",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unreachable_branches_keep_unified_shape() {
        // None/其他值分支对 dsh 实际不可达（detect 不读版本号）；
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
