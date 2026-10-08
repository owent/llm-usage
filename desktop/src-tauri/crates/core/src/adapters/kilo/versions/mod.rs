//! Kilo registry maps session.version to implementations or compatibility fallback.
//! See architecture.md#adapter-layout / #unknown-version.
//!
//! kilo.db stores the CLI version in session.version, fixed for each session when messages are written.
//! Detection uses the highest numeric version as a file marker; each message selects by its owning session.
//!
//! Register versions after checking redacted native samples and expected results:
//!
//! - 7.4.8: session-7.4.8-edges covers error-message zero tokens, missing total and two models.
//! - 7.4.9: session-7.4.9-family checks reading, parsing, storage and queries for five sessions/51 calls.
//! - 7.8.1: session-7.8.1-k3 redacts a native local session with 34 k3-256k calls,
//!   top-level modelID/providerID and five token fields; session reconciliation matched.
//!
//! All three use the same message.data.tokens shape through message_tokens_v1.
//!
//! Selection rules:
//! - Registered version: KnownVersion, dispatch through the mapping.
//! - Unregistered/missing version: try the latest built-in parser with LatestFallback.
//!   Validated records retain compatibility metadata. An earlier local inventory found 7.3.42–7.7.12;
//!   at that stage only 7.4.8/7.4.9 had verified samples, and other versions used LatestFallback.
//! - No Kilo version is verified incompatible. Detection rejects schema deviations;
//!   scanning handles incompatible structures under V30 while retaining previous results.

pub mod message_tokens_v1;

/// This constant selects the latest built-in implementation without network access.
pub const LATEST_IMPL_ID: &str = "message_tokens_v1";

/// Registered session.version values mapped to implementations.
/// Each version has native redacted samples and manually calculated expectations. Shared formats
/// retain separate version entries; add entries without deleting historical implementations.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[
    ("7.4.8", "message_tokens_v1"),
    ("7.4.9", "message_tokens_v1"),
    ("7.8.1", "message_tokens_v1"),
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

/// Compare numeric version segments: 7.4.10 exceeds 7.4.9, unlike string ordering.
/// Parse nonnumeric segments as i64::MIN; equal parsed vectors use string ordering as a tie-breaker.
pub(crate) fn version_max<'a>(a: &'a str, b: &'a str) -> &'a str {
    let parse = |v: &str| -> Vec<i64> {
        v.split(['.', '-', '+'])
            .map(|part| part.parse::<i64>().unwrap_or(i64::MIN))
            .collect()
    };
    let (na, nb) = (parse(a), parse(b));
    if na != nb {
        return if na > nb { a } else { b };
    }
    if a >= b {
        a
    } else {
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::VersionBasis;

    #[test]
    fn verified_versions_dispatch_known() {
        for v in ["7.4.8", "7.4.9", "7.8.1"] {
            assert_eq!(
                select(Some(v)),
                Selection {
                    impl_id: "message_tokens_v1",
                    basis: VersionBasis::KnownVersion
                }
            );
        }
    }

    #[test]
    fn unrecorded_version_falls_back_to_latest() {
        // Unregistered versions, including those observed locally, use latest_fallback without rejection.
        for v in ["7.3.42", "7.4.20", "7.7.12", "9.0.0"] {
            assert_eq!(select(Some(v)).basis, VersionBasis::LatestFallback);
        }
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
    }

    #[test]
    fn version_max_is_numeric_aware() {
        assert_eq!(version_max("7.4.9", "7.4.10"), "7.4.10");
        assert_eq!(version_max("7.4.10", "7.4.9"), "7.4.10");
        assert_eq!(version_max("7.10.0", "7.9.9"), "7.10.0");
        assert_eq!(version_max("7.4.8", "7.4.8"), "7.4.8");
    }
}
