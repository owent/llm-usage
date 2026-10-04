//! Gemini 格式注册表：文档级格式版本 → 实现的映射与选择
//! （architecture.md#adapter-layout / #unknown-version）。
//!
//! gemini 会话 JSON 无版本字段，格式版本为文档级 [`GEMINI_FORMAT_VERSION`]
//! （按官方文档 A10 定义，待真实样本），因此：
//! - 不存在"未知版本"状态，detect 不按版本分派、不做 latest_fallback 回退；
//! - 本注册表为与其他 Agent 统一的目录/调用形状而设，条目键是文档级格式版本
//!   （非 CLI 版本）；新文档级格式（如 session-doc-2）核验后在此登记并分派。
//!
//! 选择规则（与 codex 同形）：已收录 → `KnownVersion`；未收录/缺失 →
//! `LatestFallback`。gemini 现有调用方只以固定文档级版本进入（恒为 KnownVersion），
//! 不依赖未知版本回退语义。

pub mod session_doc1;

/// 文档级格式版本（非 CLI 版本）：会话 JSON 结构按 A10 文档定义实现，待真实样本。
pub const GEMINI_FORMAT_VERSION: &str = "session-doc-1";

/// 当前格式实现标识（"最新内置解析器"由本常量明确指定，不联网获取）。
pub const LATEST_IMPL_ID: &str = "session_doc1";

/// 已验证支持的文档级格式版本 → 格式实现（当前唯一条目）。
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[
    // 依据 A10：顶层结构与消息 tokens 分类按官方文档定义实现。
    (GEMINI_FORMAT_VERSION, "session_doc1"),
];

/// 版本分派结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// 按版本选择格式实现；与 codex 保持同一函数形状，便于注册表统一消费。
/// gemini 无版本字段，调用方只以固定文档级版本进入（恒为 KnownVersion）。
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
        // 版本缺失：gemini 调用方不进入该分支（无版本字段即固定文档级版本）；
        // 保留回退分支仅为与 codex 同形。
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
    fn documented_format_version_dispatches_known() {
        assert_eq!(
            select(Some(GEMINI_FORMAT_VERSION)),
            Selection {
                impl_id: "session_doc1",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unregistered_version_keeps_codex_shaped_fallback() {
        // 与 codex 同形的回退分支：未收录/缺失 → 最新内置实现 + LatestFallback。
        // gemini 无版本字段，实际调用不会走到；仅锁定注册表统一形状。
        assert_eq!(
            select(Some("session-doc-9")),
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
