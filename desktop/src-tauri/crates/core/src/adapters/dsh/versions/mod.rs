//! DSH registry maps format versions to implementations or fallback.
//! See architecture.md#adapter-layout / #unknown-version, V30.
//!
//! Native rc.2 format 4 has nonempty real samples; retain the separate legacy session-log-doc-1 ID.
//! KnownVersion identifies a file format, not all client versions or inherited history.
//! select retains the shared fallback interface; detection rejects unknown explicit format versions.

pub mod session_log_doc1;
pub mod session_v4;

/// This constant selects the latest built-in implementation without network access.
pub const LATEST_IMPL_ID: &str = "session_v4";

/// Legacy format ID based on README 46a7f68 event/replacement rules, not a product version.
/// Native format-4 samples do not verify this separate legacy format.
pub const DSH_FORMAT_VERSION: &str = "session-log-doc-1";

/// Registered format IDs mapped to implementations.
/// No per-product version mappings; keep native format 4 and the legacy ID separate.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[
    ("session-log-doc-1", "session_log_doc1"),
    ("4", "session_v4"),
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
                // Detection rejects unknown explicit formats; retain the shared selection interface.
                None => Selection {
                    impl_id: LATEST_IMPL_ID,
                    basis: crate::domain::VersionBasis::LatestFallback,
                },
            }
        }
        // Detection requires a native header or legacy shape; None preserves the common interface.
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
        // Test fallback branches directly; successful detection supplies a supported native or legacy format.
        // Preserve the same LatestFallback result shape as other registries for V30 checks.
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
