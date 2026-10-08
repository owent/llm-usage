//! Hermes registry: state.db schema_version INTEGER maps to format implementations
//! and unknown-version selection, architecture.md#adapter-layout / #unknown-version.
//!
//! Version marker is one integer row in state.db schema_version; fixed source
//! hermes_state_common.py sets SCHEMA_VERSION=30; detection also checks columns/primary key,
//! without relying on the integer alone.
//!
//! Official 0.21.5 image CLI/resume already has redacted native samples and independent totals.
//! Database schema describes migration, without per-row client versions for mixed history.
//! Registry therefore stays empty: every version uses marked LatestFallback for normal reads.
//!
//! Selection rules:
//! - Registered versions use KnownVersion; currently no entries.
//! - Unregistered/missing versions, including absent schema_version table, use LatestFallback
//!   and try the latest built-in session_model_usage_v1 parser first.
//! - Detection rejects known incompatible missing tables/columns/pre-v22 primary keys,
//!   without entering fallback.

pub mod session_model_usage_v1;

/// Current implementation explicitly selects the latest built-in parser, without downloading.
pub const LATEST_IMPL_ID: &str = "session_model_usage_v1";

/// Verified schema_version values mapped to implementations.
/// Empty: database schemas do not identify row client versions; one native sample is insufficient.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[];

/// Version-selection result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// Select by original source version; detection/scanning share this policy, V30.
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
        // Identifiable agent/format without version: try the latest implementation.
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
    fn registry_is_empty_so_everything_is_latest_fallback() {
        // Native schema_version=30 does not identify client versions of mixed historical rows.
        assert!(VERIFIED_VERSION_IMPLS.is_empty());
        assert_eq!(
            select(Some("30")),
            Selection {
                impl_id: "session_model_usage_v1",
                basis: VersionBasis::LatestFallback
            }
        );
        assert_eq!(select(Some("22")).basis, VersionBasis::LatestFallback);
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
    }
}
