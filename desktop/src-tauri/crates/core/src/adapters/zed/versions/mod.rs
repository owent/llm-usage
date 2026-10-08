//! Zed registry: map format versions to implementations and fallback selection.
//! architecture.md#adapter-layout / #unknown-version, V30 directory rules.
//!
//! threads.db lacks product versions; registry uses documented format IDs
//! zed-threads-db-1 / 2: bd74733 schema and 76659a55 external-provider native samples.
//! Reject schema changes in detection/scanning, without version fallback.

pub mod threads_db_v1;

/// Current implementation; explicitly selects the latest built-in parser.
pub const LATEST_IMPL_ID: &str = "threads_db_v1";

/// Documented formats include verified 1.22.0 external-provider mappings, without validating all history.
pub const ZED_FORMAT_VERSION: &str = "zed-threads-db-2";

/// Verified format versions mapped to implementations.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[
    ("zed-threads-db-1", "threads_db_v1"),
    ("zed-threads-db-2", "threads_db_v1"),
];

/// Version-selection result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// Select by format version; shared by detection and scanning, V30.
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
    fn doc_anchor_dispatches_known() {
        assert_eq!(
            select(Some("zed-threads-db-1")),
            Selection {
                impl_id: "threads_db_v1",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unknown_or_missing_falls_back_to_latest() {
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
        assert_eq!(select(Some("other")).basis, VersionBasis::LatestFallback);
    }
}
