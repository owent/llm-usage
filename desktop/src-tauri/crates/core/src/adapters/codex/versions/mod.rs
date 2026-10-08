//! Codex registry maps release versions to implementations or compatibility fallback.
//! See architecture.md#adapter-layout / #unknown-version.
//!
//! Register versions after native-sample checks: redacted test data or full local reads with calculated expectations.
//! Extend the registry by adding entries without deleting historical implementations.
//! Format implementations:
//! - rollout_v1: token_usage_record per-call records in 0.153+. Native 0.155.0-alpha.16.3
//!   samples verify reading/parsing/storage/queries; 0.153/0.154 samples share that checked format.
//! - rollout_legacy: 0.139–0.151 lack per-call records; event_msg/token_count echoes
//!   last_token_usage. Full local reads on 2026-09-26 covered 238 files across 21 versions:
//!   zero token_usage_record records, 13,481 token_count records grouped by total differences,
//!   and 222/238 matched reconciliations. See rollout_legacy.rs for methods and criteria.
//!
//! Selection rules:
//! - Registered version: KnownVersion, dispatch through the mapping.
//! - Unregistered/missing version: try the latest built-in parser (rollout_v1) with LatestFallback.
//!   Validated records retain compatibility metadata; an unregistered number alone does not cause rejection.
//! - No Codex version is verified incompatible; there are no known_incompatible entries.
//!   Scanning handles incompatible structures under V30 while retaining previous results.

pub mod rollout_legacy;
pub mod rollout_v1;

/// This constant selects the latest built-in implementation without network access.
pub const LATEST_IMPL_ID: &str = "rollout_v1";

/// Registered release versions mapped to implementations.
/// Each version has checked native format samples; shared formats retain separate version entries.
/// 0.139–0.151 rollout_legacy: full local reads grouped each version by total differences and
/// compared summed calls with final snapshots. 222/238 matched; the 16 mismatches were
/// explained. Representative 0.139.0/0.142.5/0.146.0-alpha.3 samples verify
/// reading/parsing/storage/queries; see rollout_legacy.rs and the M2-D validation record.
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[
    // M2-A native samples: reading/parsing/storage/query expectations for three sessions/49 calls.
    ("0.155.0-alpha.16.3", "rollout_v1"),
    // M2-D version-specific redacted rollout-v*.sanitized.json samples and _expectations.md:
    // independent jq calculations; token_usage_record has the checked 0.155 shape, shared through rollout_v1.
    ("0.154.0-alpha.6.2", "rollout_v1"),
    ("0.154.0-alpha.6.1", "rollout_v1"),
    ("0.153.0", "rollout_v1"),
    // Legacy token_count/last_token_usage: full local reads of 238 files and redacted
    // rollout-legacy-*.sanitized.json reading/parsing/storage/query checks, 2026-09-26.
    ("0.151.0-alpha.7.1", "rollout_legacy"),
    ("0.149.0-alpha.4.1", "rollout_legacy"),
    ("0.148.0-alpha.9", "rollout_legacy"),
    ("0.147.0-alpha.6.5", "rollout_legacy"),
    ("0.146.0-alpha.9.2", "rollout_legacy"),
    ("0.146.0-alpha.3.1", "rollout_legacy"),
    ("0.146.0-alpha.3", "rollout_legacy"),
    ("0.145.0-alpha.27", "rollout_legacy"),
    ("0.145.0-alpha.18", "rollout_legacy"),
    ("0.144.5", "rollout_legacy"),
    ("0.144.2", "rollout_legacy"),
    ("0.144.0-alpha.4", "rollout_legacy"),
    ("0.142.5", "rollout_legacy"),
    ("0.142.4", "rollout_legacy"),
    ("0.142.3", "rollout_legacy"),
    ("0.142.2", "rollout_legacy"),
    ("0.142.0", "rollout_legacy"),
    ("0.142.0-alpha.6", "rollout_legacy"),
    ("0.142.0-alpha.1", "rollout_legacy"),
    ("0.140.0-alpha.2", "rollout_legacy"),
    ("0.139.0", "rollout_legacy"),
];

/// Preserve the old public constant name used by tests/examples.
pub const SUPPORTED_CLI_VERSIONS: &[&str] = &["0.155.0-alpha.16.3"];

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

/// Select the scan implementation using persisted parse_context.cli_version first, avoiding
/// repeated header reads. Initial/rescan/missing-context paths read bounded first-line session_meta.
/// Either failed route selects None: LatestFallback through rollout_v1, matching detection.
fn select_for_scan(
    target: &crate::adapters::framework::ScanTarget,
    stored: &crate::adapters::framework::StoredScanState,
) -> Selection {
    let from_context = if target.rescan {
        None
    } else {
        stored
            .parse_context
            .as_ref()
            .and_then(|v| v.get("cli_version"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    };
    match from_context {
        Some(version) => select(Some(version.as_str())),
        None => select(first_line_cli_version(&target.path).as_deref()),
    }
}

/// Read first-line session_meta.payload.cli_version within bounds; read/JSON failures return None.
fn first_line_cli_version(path: &std::path::Path) -> Option<String> {
    let limits = crate::adapters::jsonl::JsonlLimits {
        chunk_bytes: 64 * 1024,
        max_line_bytes: crate::adapters::jsonl::DEFAULT_MAX_LINE_BYTES,
        max_lines: Some(1),
        time_budget: Some(std::time::Duration::from_secs(5)),
    };
    let outcome = crate::adapters::jsonl::read_jsonl(path, 0, 1, &limits).ok()?;
    let first = outcome.lines.first()?;
    let line = crate::adapters::run_policy::json_from_str::<serde_json::Value>(&first.text).ok()?;
    if line.get("type").and_then(|t| t.as_str()) != Some("session_meta") {
        return None;
    }
    line.get("payload")
        .and_then(|p| p.get("cli_version"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

/// Dispatch scans through the same registered version policy used by detection (V30).
/// This dispatcher does not reset consumed cursors on implementation changes; the scan framework decides when to rescan.
/// Before registration these legacy files were incompatible with unadvanced cursors, so initial parsing had no retained scan state.
pub fn dispatch_scan(
    target: &crate::adapters::framework::ScanTarget,
    stored: &crate::adapters::framework::StoredScanState,
    limits: &crate::adapters::framework::ScanLimits,
    now_ms: i64,
) -> Result<crate::adapters::framework::ScanOutcome, crate::error::CoreError> {
    match select_for_scan(target, stored).impl_id {
        "rollout_legacy" => rollout_legacy::scan(target, stored, limits, now_ms),
        _ => rollout_v1::scan(target, stored, limits, now_ms),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::VersionBasis;

    #[test]
    fn verified_versions_dispatch_known() {
        assert_eq!(
            select(Some("0.155.0-alpha.16.3")),
            Selection {
                impl_id: "rollout_v1",
                basis: VersionBasis::KnownVersion
            }
        );
        // M2-D version-specific samples verify these shared-format versions.
        for v in ["0.153.0", "0.154.0-alpha.6.1", "0.154.0-alpha.6.2"] {
            assert_eq!(
                select(Some(v)),
                Selection {
                    impl_id: "rollout_v1",
                    basis: VersionBasis::KnownVersion
                }
            );
        }
        // Register verified 0.139–0.151 versions as rollout_legacy after sample/full-local-read checks.
        for v in [
            "0.139.0",
            "0.140.0-alpha.2",
            "0.142.0-alpha.1",
            "0.142.0-alpha.6",
            "0.142.0",
            "0.142.2",
            "0.142.3",
            "0.142.4",
            "0.142.5",
            "0.144.0-alpha.4",
            "0.144.2",
            "0.144.5",
            "0.145.0-alpha.18",
            "0.145.0-alpha.27",
            "0.146.0-alpha.3",
            "0.146.0-alpha.3.1",
            "0.146.0-alpha.9.2",
            "0.147.0-alpha.6.5",
            "0.148.0-alpha.9",
            "0.149.0-alpha.4.1",
            "0.151.0-alpha.7.1",
        ] {
            assert_eq!(
                select(Some(v)),
                Selection {
                    impl_id: "rollout_legacy",
                    basis: VersionBasis::KnownVersion
                },
                "version {v}"
            );
        }
        // Unverified releases (such as 0.141.0 or future versions) still use rollout_v1 compatibility fallback.
        assert_eq!(select(Some("0.141.0")).basis, VersionBasis::LatestFallback);
        assert_eq!(
            select(Some("0.142.5-x")).basis,
            VersionBasis::LatestFallback
        );
    }

    #[test]
    fn unrecorded_version_falls_back_to_latest() {
        assert_eq!(
            select(Some("0.199.0-alpha.1")),
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
