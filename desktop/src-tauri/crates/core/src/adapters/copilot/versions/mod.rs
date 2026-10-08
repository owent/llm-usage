//! Copilot CLI version registry (V30 layout).
//!
//! Uses session-store.db schema_version (locally observed 8) and M0-verified 1.0.73
//! per-turn fields. schema_version=8 selects KnownVersion; other/missing versions use
//! LatestFallback compatibility attempts; detection validates the column set.

pub mod usage_events_v8;

/// Identifier of the current format implementation.
pub const LATEST_IMPL_ID: &str = "usage_events_v8";

/// Verified format version: schema_version=8, native-data check on 2026-09-29.
pub const COPILOT_FORMAT_VERSION: &str = "assistant-usage-events-v8";

/// Supported format versions mapped to implementations.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] =
    &[("assistant-usage-events-v8", "usage_events_v8")];

/// Result of selecting a version implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// Select by format version; detection and scanning share this function (V30).
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
    fn v8_dispatches_known() {
        assert_eq!(
            select(Some("assistant-usage-events-v8")),
            Selection {
                impl_id: "usage_events_v8",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unknown_or_missing_falls_back() {
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
        assert_eq!(select(Some("v9")).basis, VersionBasis::LatestFallback);
    }
}
