//! Cline legacy UI registry maps format IDs to implementations or fallback.
//! See architecture.md#adapter-layout / #unknown-version, V30.
//!
//! Legacy ui_messages.json has no CLI-version field; its documentation-based format ID is
//! ui-messages-doc-1, defined by fixed source dcf8c3c. This legacy route reads no version:
//! successful detection returns KnownVersion. Scanning rejects undocumented say kinds and
//! non-say record types. Native SDK files use their own detection/scanning implementation.
//!
//! Keep the select interface shared with Claude/Gemini for V30 structural checks:
//! - Documentation-based format ID: KnownVersion, dispatch through the mapping.
//! - None/other values: LatestFallback. Legacy UI detection supplies the fixed ID, so these
//!   branches preserve the common interface rather than classify SDK versions.

pub mod sdk_messages_v1;
pub mod ui_messages_doc1;

/// This constant selects the latest built-in implementation without network access.
pub const LATEST_IMPL_ID: &str = "ui_messages_doc1";

/// Legacy UI-format ID based on message definitions in fixed source dcf8c3c, not a CLI version.
/// Native verification of this legacy format remains pending; SDK verification is separate.
pub const CLINE_FORMAT_VERSION: &str = "ui-messages-doc-1";

/// Registered legacy UI-format IDs mapped to implementations.
/// Legacy Cline UI has no per-CLI-version entry; its sole format ID is ui-messages-doc-1.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[("ui-messages-doc-1", "ui_messages_doc1")];

/// Version selection result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// Legacy UI detection and scanning share format-ID selection for V30.
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
                // Legacy UI detection supplies the fixed format ID; other values do not occur on that route.
                None => Selection {
                    impl_id: LATEST_IMPL_ID,
                    basis: crate::domain::VersionBasis::LatestFallback,
                },
            }
        }
        // Legacy UI detection supplies the fixed format ID; None preserves the common interface.
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
            select(Some("ui-messages-doc-1")),
            Selection {
                impl_id: "ui_messages_doc1",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unreachable_branches_keep_unified_shape() {
        // Legacy UI detection supplies the fixed ID, so None/other values are interface checks.
        // Preserve the LatestFallback result shape used by other registries for V30.
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
