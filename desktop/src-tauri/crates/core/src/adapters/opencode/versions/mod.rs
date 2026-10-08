//! OpenCode registry maps session.version (upstream application version stored as TEXT)
//! to format implementations or compatibility fallback. See architecture.md#adapter-layout /
//! #unknown-version.
//!
//! Register versions after checking redacted native samples and expected results. Official CLI
//! 1.18.34 has verified local-model main-loop/cache reads. Select each record; empty sessions verify no other records.
//! Support updates reevaluate old processing positions by stable part keys without adding duplicate usage.
//!
//! Selection rules:
//! - Registered version: KnownVersion.
//! - Unregistered/missing version: LatestFallback, try the latest built-in parser
//!   (step_finish_parts_v1).
//! - Detection rejects verified incompatible shapes: missing tables/key columns or only the new
//!   core derived-view format; they do not enter compatibility fallback.

pub mod step_finish_parts_v1;

/// This constant selects the latest built-in implementation without network access.
pub const LATEST_IMPL_ID: &str = "step_finish_parts_v1";

/// Registered session.version values mapped to implementations.
/// Native redacted samples and per-record upgrade requirements define the verified scope.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[("1.18.34", LATEST_IMPL_ID)];

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
                None => Selection {
                    impl_id: LATEST_IMPL_ID,
                    basis: crate::domain::VersionBasis::LatestFallback,
                },
            }
        }
        // Detection returns Pending when no usage parts exist. A usage record whose session version
        // is missing or invalid uses the latest implementation with compatibility metadata.
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
    fn registry_only_certifies_the_verified_version() {
        assert_eq!(select(Some("1.18.34")).basis, VersionBasis::KnownVersion);
        assert_eq!(select(Some("1.17.13")).basis, VersionBasis::LatestFallback);
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
    }
}
