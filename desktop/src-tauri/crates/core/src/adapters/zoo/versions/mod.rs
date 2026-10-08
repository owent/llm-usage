//! Zoo Code registry maps format identifiers to implementations or fallback.
//! See architecture.md#adapter-layout / #unknown-version.
//!
//! ui_messages.json has no CLI-version field; use the documentation-based format ID
//! zoo-ui-messages-doc-1, initially based on f780647, without identifying a client version.
//! Successful detection returns KnownVersion for this format; scanning checks native message kinds.
//! Invalid kinds fail validation; supported non-usage messages do not generate usage.
//! Keep Zoo separate from Cline (A19); its product identity must not become Roo Code.

pub mod ui_messages_doc1;

/// This constant selects the latest built-in implementation without network access.
pub const LATEST_IMPL_ID: &str = "ui_messages_doc1";

/// UI-format identifier from initial fixed-source f780647, not a CLI version.
/// Native 3.86.0 samples verify current UI reading rules without adding per-release dispatch.
pub const ZOO_FORMAT_VERSION: &str = "zoo-ui-messages-doc-1";

/// Registered format IDs mapped to implementations.
/// Zoo has no per-CLI-version entry; the format ID remains zoo-ui-messages-doc-1.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] =
    &[("zoo-ui-messages-doc-1", "ui_messages_doc1")];

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
                // Detection supplies the fixed format ID; other select values are outside this route.
                None => Selection {
                    impl_id: LATEST_IMPL_ID,
                    basis: crate::domain::VersionBasis::LatestFallback,
                },
            }
        }
        // Detection supplies the fixed format ID; retain None for the shared interface.
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
            select(Some("zoo-ui-messages-doc-1")),
            Selection {
                impl_id: "ui_messages_doc1",
                basis: VersionBasis::KnownVersion
            }
        );
    }
}
