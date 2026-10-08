//! pi registry maps session versions to implementations or compatibility fallback.
//! See architecture.md#adapter-layout / #unknown-version.
//!
//! Version references: fixed pi-mono b45597504eeaba1f11a9920a1d1048c361ed4b8e source and
//! native local samples. See the session_v3 header for original format references.
//! - Verified: session version 3 only, from CURRENT_SESSION_VERSION=3 in fixed source and
//!   checked native samples; select session_v3 with KnownVersion.
//! - Verified incompatible: versions 1 and 2 lack entry id/parentId in fixed source.
//!   Reject with a reason until version-specific samples permit separate implementations.
//! - Missing version: v1/v2 did not write this field; treat as incompatible legacy format
//!   with found: None.
//! - Other numeric versions (such as 4): try the latest built-in parser with LatestFallback
//!   metadata; an unregistered number alone does not cause rejection.
//!
//! Detection and scanning use select as the shared version policy for V30.
//! Extend the registry by adding entries without deleting historical implementations.

pub mod session_v3;

/// This constant selects the latest built-in implementation without network access.
pub const LATEST_IMPL_ID: &str = "session_v3";

/// Registered session versions mapped to implementations.
/// Each version needs fixed-source/sample checks; shared formats retain separate version entries.
pub const VERIFIED_VERSION_IMPLS: &[(i64, &str)] = &[
    // Fixed CURRENT_SESSION_VERSION=3 source and native session-error-zero-usage sample.
    (3, "session_v3"),
];

/// Versions verified incompatible in fixed source: v1/v2 entries lack id/parentId.
pub const EVIDENCED_INCOMPATIBLE_VERSIONS: &[i64] = &[1, 2];

/// v1/v2 incompatibility reason; do not try fallback without version-specific samples.
pub const REASON_V1_V2_INCOMPATIBLE: &str =
    "pi-mono fixed source: v1/v2 format differs (no id/parentId); per-version fixture 前不尝试";

/// Missing-version reason: v1/v2 omitted the field, so reject the legacy format.
pub const REASON_LEGACY_MISSING: &str = "pi-mono fixed source: v1/v2 era session headers carry no version field; legacy shape treated as evidenced incompatible";

/// Preserve the old public constant name for the only verified session version.
pub const SUPPORTED_SESSION_VERSION: i64 = 3;

/// Version selection result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// Detection and scanning select by the original source session version through this function (V30).
/// Err gives a verified incompatibility reason; callers reject and record diagnostics.
pub fn select(found: Option<i64>) -> Result<Selection, &'static str> {
    match found {
        Some(version) => {
            if let Some((_, impl_id)) = VERIFIED_VERSION_IMPLS.iter().find(|(v, _)| *v == version) {
                return Ok(Selection {
                    impl_id,
                    basis: crate::domain::VersionBasis::KnownVersion,
                });
            }
            if EVIDENCED_INCOMPATIBLE_VERSIONS.contains(&version) {
                return Err(REASON_V1_V2_INCOMPATIBLE);
            }
            // Try unregistered numeric versions with the latest parser and compatibility metadata (V30).
            Ok(Selection {
                impl_id: LATEST_IMPL_ID,
                basis: crate::domain::VersionBasis::LatestFallback,
            })
        }
        // Missing version identifies the v1/v2 legacy format; do not try fallback.
        None => Err(REASON_LEGACY_MISSING),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::VersionBasis;

    #[test]
    fn verified_version_dispatches_known() {
        assert_eq!(
            select(Some(3)),
            Ok(Selection {
                impl_id: "session_v3",
                basis: VersionBasis::KnownVersion
            })
        );
    }

    #[test]
    fn evidenced_incompatible_versions_rejected() {
        for version in [1, 2] {
            assert_eq!(select(Some(version)), Err(REASON_V1_V2_INCOMPATIBLE));
        }
        // A missing version likewise identifies the verified incompatible legacy format.
        assert_eq!(select(None), Err(REASON_LEGACY_MISSING));
    }

    #[test]
    fn unrecorded_version_falls_back_to_latest() {
        assert_eq!(
            select(Some(4)),
            Ok(Selection {
                impl_id: LATEST_IMPL_ID,
                basis: VersionBasis::LatestFallback
            })
        );
    }
}
