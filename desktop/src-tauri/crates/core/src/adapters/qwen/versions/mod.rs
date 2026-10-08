//! Qwen legacy ChatRecord registry maps format IDs to implementations.
//! See architecture.md#adapter-layout / #unknown-version.
//!
//! Legacy ChatRecord uses fixed source commit prefix 085e98c0 as its format ID, not a release allowlist.
//! Each record.version (CLI version) is stored as schema_version without controlling dispatch.
//! This registry preserves the shared adapter interface; chatrecord_085e98c0 is its sole format
//! from fixed source A18. Add entries/selection variants when a new fixed-source format is verified.
//!
//! Selection rules follow the shared adapter interface:
//! - Registered format ID: KnownVersion, dispatch through the mapping.
//! - Unregistered/missing ID: LatestFallback through the latest built-in parser.
//!   Legacy ChatRecord detection supplies the registered fixed ID; SDK reading is separate.

pub mod chatrecord_085e98c0;

/// This constant selects the latest built-in implementation without network access.
pub const LATEST_IMPL_ID: &str = "chatrecord_085e98c0";

/// Format ID is the fixed-source commit prefix defining this schema.
pub const QWEN_FORMAT_VERSION: &str = "chatrecord-085e98c0";

/// Registered format IDs mapped to implementations.
/// Keys identify fixed-source formats, not allowed CLI releases. Store record.version as
/// schema_version per record without a release allowlist; retain the shared registry interface.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[
    // A18 fixed source: qwen-code commit 085e98c00cac2f8dd29eb39c760409bc6da889a9.
    ("chatrecord-085e98c0", "chatrecord_085e98c0"),
];

/// Version selection result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// Detection and scanning share this V30 format-selection function.
/// Legacy ChatRecord callers supply the registered fixed ID (KnownVersion).
/// Missing/unregistered branches preserve the shared interface and are not used by that route.
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
        // Detection/scanning of legacy ChatRecord use the registered fixed-source ID: known_version.
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
        // Shared fallback branch; legacy ChatRecord callers always supply the registered fixed ID.
        assert_eq!(
            select(Some("chatrecord-ffffffff")).basis,
            VersionBasis::LatestFallback
        );
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
    }
}
