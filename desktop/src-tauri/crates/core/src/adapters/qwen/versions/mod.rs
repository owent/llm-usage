//! Qwen 版本注册表：格式版本 → 格式实现的映射与选择
//! （architecture.md#adapter-layout / #unknown-version）。
//!
//! Qwen 的格式锚点是固定源码 commit（085e98c0 前缀），不是发布版本白名单：
//! `record.version`（CLI 版本）逐条存 schema_version，不参与分派；本注册表为
//! 与其他 Agent 统一的结构而设，当前仅一个格式实现 `chatrecord_085e98c0`
//! （A18 固定源码依据）。重固定新 commit 后增加条目并扩展枚举。
//!
//! 选择规则（与其他 Agent 同形）：
//! - 已收录格式版本 → `KnownVersion`，按映射分派；
//! - 未收录/缺失 → `LatestFallback`，回退最新内置解析器；
//!   Qwen 探测恒以固定锚点查询（已收录），该分支仅为保持统一函数形状。

pub mod chatrecord_085e98c0;

/// 当前格式实现标识（"最新内置解析器"由本常量明确指定，不联网获取）。
pub const LATEST_IMPL_ID: &str = "chatrecord_085e98c0";

/// 格式版本 = 固定源码 commit 前缀（schema 以该 commit 为准）。
pub const QWEN_FORMAT_VERSION: &str = "chatrecord-085e98c0";

/// 已验证格式版本 → 格式实现。
/// 键是固定源码 commit 锚点，不是 CLI 版本白名单（`record.version` 逐条存
/// schema_version，不做白名单）；注册表为统一结构而设。
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[
    // A18 固定源码依据（qwen-code commit 085e98c00cac2f8dd29eb39c760409bc6da889a9）。
    ("chatrecord-085e98c0", "chatrecord_085e98c0"),
];

/// 版本分派结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// 按格式版本选择格式实现；探测与扫描共用本函数保证同一策略（V30）。
/// Qwen 格式版本恒为固定锚点（已收录 ⇒ KnownVersion）；未收录/缺失分支
/// 仅为与其他 Agent 统一的函数形状保留，当前不会被走到。
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
    fn pinned_format_anchor_dispatches_known() {
        // 固定源码 commit 锚点已收录：探测/扫描据此给出 known_version。
        assert_eq!(
            select(Some(QWEN_FORMAT_VERSION)),
            Selection {
                impl_id: LATEST_IMPL_ID,
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unified_select_shape_keeps_fallback_branch() {
        // 与 codex 同形的回退分支：仅保持统一结构，Qwen 恒以固定锚点查询。
        assert_eq!(
            select(Some("chatrecord-ffffffff")).basis,
            VersionBasis::LatestFallback
        );
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
    }
}
