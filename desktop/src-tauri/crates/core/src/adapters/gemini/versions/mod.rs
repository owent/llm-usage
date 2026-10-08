//! Gemini registry maps documentation-based format IDs to implementations.
//! See architecture.md#adapter-layout / #unknown-version.
//!
//! Gemini session JSON has no version field; GEMINI_FORMAT_VERSION identifies the format
//! defined by official A10 documentation, with native samples pending. Therefore:
//! - Detection does not dispatch by CLI version or use latest_fallback.
//! - This registry preserves the directory/call interface shared with other adapters.
//!   Keys identify documented formats, not CLI versions; register new formats (such as session-doc-2) after checks.
//!
//! Shared selection interface: registered ID returns KnownVersion; unregistered/missing IDs
//! return LatestFallback. Current Gemini callers supply the fixed registered format ID
//! and do not rely on unknown-version fallback.

pub mod session_doc1;

/// Documentation-based session JSON format ID under A10, not a CLI version; native samples pending.
pub const GEMINI_FORMAT_VERSION: &str = "session-doc-1";

/// This constant selects the latest built-in implementation without network access.
pub const LATEST_IMPL_ID: &str = "session_doc1";

/// Registered documentation-based format IDs mapped to implementations; currently one entry.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[
    // A10 defines the top-level structure and message token categories.
    (GEMINI_FORMAT_VERSION, "session_doc1"),
];

/// Version selection result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// Select by format ID through the same interface as Codex for shared registry callers.
/// Gemini has no version field; callers supply the fixed registered format ID (KnownVersion).
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
        // Missing format ID does not occur in Gemini callers, which supply the documented ID.
        // Keep this fallback branch for the shared Codex-style interface.
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
    fn documented_format_version_dispatches_known() {
        assert_eq!(
            select(Some(GEMINI_FORMAT_VERSION)),
            Selection {
                impl_id: "session_doc1",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unregistered_version_keeps_codex_shaped_fallback() {
        // Shared fallback interface: unregistered/missing IDs use the latest parser with LatestFallback.
        // Current Gemini callers do not take this branch; test the shared registry interface.
        assert_eq!(
            select(Some("session-doc-9")),
            Selection {
                impl_id: LATEST_IMPL_ID,
                basis: VersionBasis::LatestFallback
            }
        );
        assert_eq!(
            select(None),
            Selection {
                impl_id: LATEST_IMPL_ID,
                basis: VersionBasis::LatestFallback
            }
        );
    }
}
