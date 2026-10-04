//! OTel spans JSONL 版本注册表（V30 目录约定）。锚点 otel-spans-doc-1
//! （VS Code agent_monitoring.md bdc5ebe 文档格式 + CodeBuddy agentlens 属性，
//! 2026-09-29 官方文档核验；无本机真实样本）。

pub mod spans_doc1;

/// 当前格式实现标识。
pub const LATEST_IMPL_ID: &str = "spans_doc1";

/// 文档级格式版本。
pub const OTEL_FORMAT_VERSION: &str = "otel-spans-doc-1";

/// 已验证支持的格式版本 → 格式实现。
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[("otel-spans-doc-1", "spans_doc1")];

/// 版本分派结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// 按格式版本选择实现；探测与扫描共用本函数（V30）。
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
    fn doc_anchor_dispatches_known() {
        assert_eq!(
            select(Some("otel-spans-doc-1")).basis,
            VersionBasis::KnownVersion
        );
    }
}
