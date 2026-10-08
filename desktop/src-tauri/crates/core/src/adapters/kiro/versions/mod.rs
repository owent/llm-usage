//! Kiro V30 registry: register the two input formats separately.
//! References: third-party tokscale 1d9a939; closed-source AWS client, not installed locally.
//! - CLI ~/.kiro/sessions/cli/*.json user_turn_metadatas reports turn counts.
//!   Auto agents often write zero; read explicit positive counts and exclude estimates.
//! - kiro-cli ~/.local/share/kiro-cli/data.sqlite3 conversations_v2
//!   request_metadata has millisecond timestamps and five reported token buckets.
//!
//! Third-party parsing estimates IDE session.json/messages.jsonl usage; do not implement that route.

pub mod cli_turns_v1;
pub mod sqlite_v1;

/// Current CLI-turn implementation ID; SQLite uses the second registry entry.
pub const LATEST_IMPL_ID: &str = "cli_turns_v1";

/// Format identifier based on documentation/source.
pub const KIRO_FORMAT_VERSION: &str = "kiro-cli-turns-1";
/// kiro-cli SQLite format identifier.
pub const KIRO_SQLITE_FORMAT_VERSION: &str = "kiro-cli-sqlite-1";

/// Format IDs mapped to implementations for the two input types, without native version acceptance.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[
    ("kiro-cli-turns-1", "cli_turns_v1"),
    ("kiro-cli-sqlite-1", "sqlite_v1"),
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
    fn both_carriers_dispatch_known() {
        assert_eq!(
            select(Some("kiro-cli-turns-1")),
            Selection {
                impl_id: "cli_turns_v1",
                basis: VersionBasis::KnownVersion
            }
        );
        assert_eq!(
            select(Some("kiro-cli-sqlite-1")),
            Selection {
                impl_id: "sqlite_v1",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unknown_falls_back() {
        assert_eq!(select(Some("other")).basis, VersionBasis::LatestFallback);
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
    }
}
