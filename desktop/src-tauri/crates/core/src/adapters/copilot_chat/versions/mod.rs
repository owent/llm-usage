//! VS Code Copilot Chat session-log version registry, V30 directory rules.
//!
//! Source: chatSessionOperationLog.ts storageSchema fixes version: 3
//! in microsoft/vscode; native VS Code 1.140.0 data checked locally
//! on 2026-10-01, ten saved request objects, not ten underlying model calls. v3 is KnownVersion;
//! other/missing versions attempt LatestFallback.

pub mod session_log_v3;

/// Current format implementation ID.
pub const LATEST_IMPL_ID: &str = "session_log_v3";

/// Format versions checked against native local data, 2026-10-01.
pub const COPILOT_CHAT_FORMAT_VERSION: &str = "vscode-chat-session-log-v3";

/// Verified format versions mapped to implementations.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[("3", "session_log_v3")];

/// Version-selection result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// Select by format version; shared by detection and scanning, V30.
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
    fn v3_dispatches_known() {
        assert_eq!(
            select(Some("3")),
            Selection {
                impl_id: "session_log_v3",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unknown_or_missing_falls_back() {
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
        assert_eq!(select(Some("4")).basis, VersionBasis::LatestFallback);
    }
}
