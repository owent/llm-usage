//! Claude registry maps format versions to implementations or fallback.
//! See architecture.md#adapter-layout / #unknown-version, V30.
//!
//! Native 2.1.197 assistants include their own version. Select each record independently;
//! first-line/installed versions cannot identify history. Retain the legacy format ID and read other versions compatibly.

pub mod transcript_doc1;

/// This constant selects the latest built-in implementation without network access.
pub const LATEST_IMPL_ID: &str = "transcript_doc1";

/// Documentation-based format ID, not CLI version; upstream transcript formats are unstable.
/// The legacy A01 format does not establish a verified CLI version.
pub const CLAUDE_FORMAT_VERSION: &str = "transcript-doc-1";

/// Registered format versions mapped to implementations.
/// Retain the legacy format ID; only native 2.1.197 is explicitly registered.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[
    ("transcript-doc-1", "transcript_doc1"),
    ("2.1.197", "transcript_doc1"),
];

/// Version selection result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// Detection and scanning share this V30 format-selection function.
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
                // Read unverified native versions compatibly.
                None => Selection {
                    impl_id: LATEST_IMPL_ID,
                    basis: crate::domain::VersionBasis::LatestFallback,
                },
            }
        }
        // Keep compatibility metadata when no version is supplied.
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
        // An unregistered native version does not become verified support.
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
