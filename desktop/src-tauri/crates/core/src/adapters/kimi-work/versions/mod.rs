//! Kimi Work registry maps wire protocol_version to implementations or fallback.
//! See architecture.md#adapter-layout / #unknown-version, V30.
//!
//! References are actual local reads/redacted test data, not official protocol documentation.
//! - Verified 1.4 from Kimi Work embedded kimi-code home, 2026-09-25 inventory and
//!   tests/fixtures/kimi-work native redacted data: wire_v14 with KnownVersion.
//! - Unregistered/missing protocol_version uses LatestFallback until incompatibility is verified.
//!   Kimi Code verifies 1.5; in this product, 1.5 still uses
//!   latest_fallback. A12/A13 registries and verification scopes remain independent.
//!
//! select defines the version policy shared by detection/scanning.

pub mod wire_v14;

/// This constant selects the latest built-in implementation without network access.
pub const LATEST_IMPL_ID: &str = "wire_v14";

/// Register wire protocol_version mappings after reviewing version-specific samples.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[
    // All 69 local Kimi Work files (daimon embedded kimi-code home) used 1.4.
    // Redacted native samples: conv-main and agent-44-subagent.
    ("1.4", "wire_v14"),
];

/// Version selection result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// Select using source wire protocol_version for both detection and scanning (V30).
/// No verified incompatible Kimi Work versions; unregistered/missing values use latest_fallback.
pub fn select(found: Option<&str>) -> Selection {
    match found.and_then(|v| VERIFIED_VERSION_IMPLS.iter().find(|(known, _)| *known == v)) {
        Some((_, impl_id)) => Selection {
            impl_id,
            basis: crate::domain::VersionBasis::KnownVersion,
        },
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
    fn verified_version_dispatches_known() {
        assert_eq!(
            select(Some("1.4")),
            Selection {
                impl_id: "wire_v14",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unrecorded_and_missing_versions_fall_back_to_latest() {
        // Kimi Code verifies 1.5, but this registry does not: use latest_fallback.
        // The two products retain independent verification scopes despite sharing a wire family (A12/A13).
        for found in [Some("1.5"), Some("9.9"), None] {
            assert_eq!(
                select(found),
                Selection {
                    impl_id: LATEST_IMPL_ID,
                    basis: VersionBasis::LatestFallback
                }
            );
        }
    }
}
