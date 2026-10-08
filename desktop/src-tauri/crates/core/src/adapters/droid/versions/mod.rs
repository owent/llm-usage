//! Droid version registry (V30 layout). settings.json has no version field;
//! droid-settings-doc-1 comes from third-party parser tokscale at 1d9a939.

pub mod settings_doc1;

/// Identifier of the current format implementation.
pub const LATEST_IMPL_ID: &str = "settings_doc1";

/// Format version established by documentation/source inspection.
pub const DROID_FORMAT_VERSION: &str = "droid-settings-doc-1";

/// Supported format versions mapped to implementations.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[("droid-settings-doc-1", "settings_doc1")];

/// Result of selecting a version implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// Select by format version (V30 registry layout). settings.json currently has no version;
/// detection uses the documented KnownVersion; reserve dispatch for a future version field.
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
            select(Some("droid-settings-doc-1")).basis,
            VersionBasis::KnownVersion
        );
    }
}
