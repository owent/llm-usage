//! COMMANDCODE version registry (V30 layout); documented format commandcode-tree-v3.

pub mod tree_v3;

/// Identifier of the current format implementation.
pub const LATEST_IMPL_ID: &str = "tree_v3";

/// Format version established by documentation/source inspection.
pub const COMMANDCODE_FORMAT_VERSION: &str = "commandcode-tree-v3";

/// Supported format versions mapped to implementations.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[("commandcode-tree-v3", "tree_v3")];

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
    fn doc_anchor_dispatches_known() {
        assert_eq!(
            select(Some("commandcode-tree-v3")).basis,
            VersionBasis::KnownVersion
        );
    }

    #[test]
    fn unknown_or_missing_falls_back() {
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
    }
}
