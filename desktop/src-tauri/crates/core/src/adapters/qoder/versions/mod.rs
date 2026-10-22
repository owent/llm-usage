//! Qoder per-record CLI versions; source-derived verification is distinct from native acceptance.

pub mod probe_only;
pub mod transcript_v1;

/// Identifier of the current format implementation.
pub const LATEST_IMPL_ID: &str = "transcript_v1";

/// Format version established by documentation/source inspection.
pub const QODER_FORMAT_VERSION: &str = "qoder-cli-transcript-1";

/// Supported format versions mapped to implementations.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[("1.1.76", "transcript_v1")];

/// Result of selecting a version implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// Select each record's version; compatible structures do not require an allowlist entry.
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
        assert_eq!(select(Some("1.1.76")).basis, VersionBasis::KnownVersion);
    }

    #[test]
    fn unknown_or_missing_falls_back() {
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
    }
}
