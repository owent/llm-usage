//! Aider version registry, V30 directory rules.
//!
//! Analytics JSONL lacks format versions; registry uses the documented format ID
//! aider-analytics-doc-1, analytics.py event shape from official source 5dc9490.
//! Scanner rejects different shapes without version fallback.

pub mod analytics_doc1;

/// Current implementation explicitly selects the latest built-in parser.
pub const LATEST_IMPL_ID: &str = "analytics_doc1";

/// Documented format implemented from official 5dc9490 analytics events.
pub const AIDER_FORMAT_VERSION: &str = "aider-analytics-doc-1";

/// Verified format versions mapped to implementations.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[("aider-analytics-doc-1", "analytics_doc1")];

/// Version-selection result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// Select by format version; shared by detection/scanning, V30.
/// Analytics has no version field: detection directly uses documented KnownVersion, without this call.
/// Retained for V30 registry structure; connect it if native version fields become available.
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
            select(Some("aider-analytics-doc-1")),
            Selection {
                impl_id: "analytics_doc1",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unknown_or_missing_falls_back() {
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
        assert_eq!(select(Some("x")).basis, VersionBasis::LatestFallback);
    }
}
