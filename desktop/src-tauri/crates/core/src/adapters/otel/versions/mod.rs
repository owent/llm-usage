//! OTel span JSONL version registry (V30 layout), using otel-spans-doc-1.
//! Initially based on VS Code agent_monitoring.md at bdc5ebe and CodeBuddy agentlens attributes;
//! official documents checked on 2026-09-29, before local native samples were available.

pub mod qwen_sdk_025;
pub mod spans_doc1;

/// Identifier of the current format implementation.
pub const LATEST_IMPL_ID: &str = "spans_doc1";

/// Format version established by documentation/source inspection.
pub const OTEL_FORMAT_VERSION: &str = "otel-spans-doc-1";

/// Supported format versions mapped to implementations.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[
    ("otel-spans-doc-1", "spans_doc1"),
    ("qwen-code-sdk-file-0.25.0", "qwen_sdk_025"),
];

/// Result of selecting a version implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// Select by format version; detection and scanning share this function (V30).
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
