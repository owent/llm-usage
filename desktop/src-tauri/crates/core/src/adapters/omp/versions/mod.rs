//! omp registry maps numeric session versions to implementations or compatibility fallback.
//! See architecture.md#adapter-layout / #unknown-version.
//!
//! Native checks found version=3 in all 58 local session headers (Scoop 18.2.7,
//! 2026-08-21 through 2026-09-24), supported by fixed oh-my-pi 62bc57b source. Add entries
//! without deleting historical implementations; session_v3 is currently the sole format.
//!
//! Selection differs from pi because the verified format scopes differ:
//! - Registered version: KnownVersion, dispatch through the mapping.
//! - Unregistered numeric/missing version: try the latest parser with LatestFallback.
//!   Validated records retain compatibility metadata; an unregistered number alone does not cause rejection.
//! - Fixed pi source verifies v1/v2 as incompatible, but old omp file formats remain
//!   unverified. omp therefore has no known_incompatible entries:
//!   missing/unregistered versions try session_v3. Scanning evaluates structural incompatibility
//!   under V30: observed records/zero events/structural diagnostics retain previous results.

pub mod session_v3;

/// This constant selects the latest built-in implementation without network access.
pub const LATEST_IMPL_ID: &str = "session_v3";

/// Registered numeric session versions mapped to implementations.
/// Each version has native-sample checks; shared formats retain separate version entries.
pub const VERIFIED_VERSION_IMPLS: &[(i64, &str)] = &[
    // All 58 native session headers used version=3; fixed oh-my-pi 62bc57b source agrees.
    (3, "session_v3"),
];

/// Version selection result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// Detection/scanning select by the original numeric session version through this function (V30).
pub fn select(found: Option<i64>) -> Selection {
    let known = found.and_then(|v| {
        VERIFIED_VERSION_IMPLS
            .iter()
            .find(|(recorded, _)| *recorded == v)
            .map(|(_, impl_id)| *impl_id)
    });
    match known {
        Some(impl_id) => Selection {
            impl_id,
            basis: crate::domain::VersionBasis::KnownVersion,
        },
        // With known product identity/input format, unregistered/missing versions try the latest parser.
        // Older omp formats remain unverified, without verified incompatibility; do not reject them by version alone.
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
    fn verified_versions_dispatch_known() {
        assert_eq!(
            select(Some(3)),
            Selection {
                impl_id: "session_v3",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unrecorded_version_falls_back_to_latest() {
        assert_eq!(
            select(Some(4)),
            Selection {
                impl_id: LATEST_IMPL_ID,
                basis: VersionBasis::LatestFallback
            }
        );
        assert_eq!(
            select(Some(2)),
            Selection {
                impl_id: LATEST_IMPL_ID,
                basis: VersionBasis::LatestFallback
            }
        );
        // Missing legacy version likewise tries compatibility, unlike pi:
        // fixed source verifies versionless pi v1 as incompatible, while old omp remains unverified.
        assert_eq!(
            select(None),
            Selection {
                impl_id: LATEST_IMPL_ID,
                basis: VersionBasis::LatestFallback
            }
        );
    }
}
