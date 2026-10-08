//! VS Copilot telemetry version registry (V30 layout).
//!
//! Verified local VS 18 Community data, 2026-10-01: OTLP JSON envelope
//! with one resourceSpans batch per line, service.name=vs-copilot, chat spans containing
//! gen_ai.usage.* (string intValue) and nanosecond Unix timestamps.

pub mod traces_v1;

/// Identifier of the current format implementation.
pub const LATEST_IMPL_ID: &str = "traces_v1";

/// Verified format version; native-data check on 2026-10-01.
pub const VS_COPILOT_FORMAT_VERSION: &str = "vs-copilot-otlp-traces-v1";

/// Supported format versions mapped to implementations.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[("1", "traces_v1")];

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
    fn v1_dispatches_known() {
        assert_eq!(
            select(Some("1")),
            Selection {
                impl_id: "traces_v1",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unknown_or_missing_falls_back() {
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
        assert_eq!(select(Some("2")).basis, VersionBasis::LatestFallback);
    }
}
