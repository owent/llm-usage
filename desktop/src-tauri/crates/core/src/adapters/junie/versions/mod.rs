//! Junie version registry (V30 layout).
//!
//! events.jsonl lacks a format-version field. The initial documented format is
//! junie-events-doc-1, based on third-party tokscale at fixed commit 1d9a939;
//! official source for the closed product is unavailable. Scanning rejects format deviations.

pub mod events_doc1;

/// Identifier of the current format implementation.
pub const LATEST_IMPL_ID: &str = "events_doc1";

/// Documented format version based on the third-party parser.
pub const JUNIE_FORMAT_VERSION: &str = "junie-events-doc-1";

/// Supported format versions mapped to implementations.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[("junie-events-doc-1", "events_doc1")];

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
            select(Some("junie-events-doc-1")),
            Selection {
                impl_id: "events_doc1",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unknown_or_missing_falls_back() {
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
    }
}
