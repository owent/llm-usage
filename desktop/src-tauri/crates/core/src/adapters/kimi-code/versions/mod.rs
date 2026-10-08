//! Kimi Code registry maps wire protocol_version to implementations or fallback.
//! See architecture.md#adapter-layout / #unknown-version, V30.
//!
//! References are actual local reads/redacted test data, not official protocol documentation.
//! - Verified 1.5 from desktop 1.0.3 and M0 samples;
//!   native tests/fixtures/kimi-code select wire_v15 with KnownVersion.
//! - Unregistered/missing protocol_version first tries the latest built-in parser
//!   with LatestFallback compatibility metadata, without rejecting unlisted numbers alone.
//! - Local active_compat files also verify protocol 1.4 with wire_v15 in this product.
//!   Kimi Work remains a separate A12/A13 registry; its samples cannot establish this entry.
//!
//! select defines the version policy shared by detection/scanning.

pub mod wire_v15;

/// This constant selects the latest built-in implementation without network access.
pub const LATEST_IMPL_ID: &str = "wire_v15";

/// Register wire protocol_version mappings after reviewing version-specific samples.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[
    // Native desktop 1.0.3 M0/M4 samples: session-main and subagent-agent-0.
    ("1.5", "wire_v15"),
    ("1.4", "wire_v15"), // Native active_compat files verify protocol 1.4 against wire_v15 independently of Kimi Work.
];

/// Version selection result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// Select using source wire protocol_version for both detection and scanning (V30).
/// No verified incompatible Kimi Code versions; unregistered/missing values use latest_fallback.
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
            select(Some("1.5")),
            Selection {
                impl_id: "wire_v15",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unrecorded_and_missing_versions_fall_back_to_latest() {
        // Try compatibility for unregistered/missing versions without verified incompatibility (V30).
        // Protocol 1.4 is registered after native active_compat verification with wire_v15.
        for found in [Some("9.9"), None] {
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
