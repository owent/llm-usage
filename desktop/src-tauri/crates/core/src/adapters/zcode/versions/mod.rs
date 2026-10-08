//! ZCode registry maps release versions to implementations or compatibility fallback.
//! See architecture.md#adapter-layout / #unknown-version.
//!
//! Register versions after checking native redacted samples and expected results (M0/M4).
//! Add entries without deleting historical implementations. The sole format is modelio_v1;
//! native 3.14.3 main-session/subagent files verify reading/parsing/storage/queries.
//!
//! Select each model_io record by request.headers["x-zcode-app-version"],
//! verified in native 3.14.3. The same machine also has daily logs with context.schemaVersion=1
//! and protocol client 0.16.9; neither identifies this adapter's release version or controls dispatch.
//!
//! Selection rules:
//! - Registered version: KnownVersion, dispatch through the mapping.
//! - Unregistered/missing version: try the latest built-in parser with LatestFallback.
//!   Validated records retain active_compat metadata; an unregistered number alone does not cause rejection.
//! - No ZCode version is verified incompatible; there are no known_incompatible entries.
//!   Scanning handles structural incompatibility under V30, rejecting it while retaining previous results.

pub mod modelio_v1;

/// This constant selects the latest built-in implementation without network access.
pub const LATEST_IMPL_ID: &str = "modelio_v1";

/// Registered release versions mapped to implementations.
/// Each version has checked native format samples; shared formats retain separate version entries.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[
    // M0/M4 native samples extracted read-only on 2026-09-25: eight records manually calculated
    // under two field meanings. adapters.md: native 3.14.3 verified; cross-version behavior pending.
    ("3.14.3", "modelio_v1"),
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
                None => Selection {
                    impl_id: LATEST_IMPL_ID,
                    basis: crate::domain::VersionBasis::LatestFallback,
                },
            }
        }
        // If product identity/input format is known but the version is missing, try the latest implementation.
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
            select(Some("3.14.3")),
            Selection {
                impl_id: "modelio_v1",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unrecorded_version_falls_back_to_latest() {
        // synthetic-future-version checks unregistered 9.9.9 through latest_fallback compatibility.
        assert_eq!(
            select(Some("9.9.9")),
            Selection {
                impl_id: LATEST_IMPL_ID,
                basis: VersionBasis::LatestFallback
            }
        );
        assert_eq!(
            select(None),
            Selection {
                impl_id: LATEST_IMPL_ID,
                basis: VersionBasis::LatestFallback
            }
        );
    }
}
