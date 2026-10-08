//! MiMo Code version registry: TEXT session.version selects a format implementation or
//! fallback (architecture.md#adapter-layout / #unknown-version).
//!
//! MiMo Code 0.1.15 has native redacted samples, but session versions still use
//! LatestFallback: those samples do not establish dedicated per-record version mappings.
//! The initial source reference was 456678b6a5afb0eef3fe2754575637218cfb3c84; the registry stays empty.
//! All versions use LatestFallback with compatibility metadata, compat=unverified.
//! Keep this registry separate from OpenCode (A14); shared engine names do not identify versions.

pub mod step_finish_parts_v1;

/// Current implementation ID: this constant selects the latest built-in parser without network access.
pub const LATEST_IMPL_ID: &str = "step_finish_parts_v1";

/// Verified session.version values mapped to implementations.
/// Empty: native samples have not established a dedicated mapping for each record version.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[];

/// Version selection result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// Select by the original record version; detection and scanning share this V30 policy.
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
    fn registry_is_empty_so_everything_is_latest_fallback() {
        assert!(VERIFIED_VERSION_IMPLS.is_empty());
        assert_eq!(select(Some("0.1.0")).basis, VersionBasis::LatestFallback);
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
    }
}
