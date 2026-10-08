//! Read local GitHub Copilot quota records from copilot-user-cache.json.
//!
//! References: 2026-10-01 local checks and the official CLI cache-directory reference.
//! VS Code Copilot Chat and Copilot CLI share an account quota cache:
//! copilot-user-cache.json under Windows %LOCALAPPDATA%/copilot/,
//! macOS ~/Library/Caches/copilot/, or Linux $XDG_CACHE_HOME/copilot
//! with ~/.cache/copilot/ fallback. JSON follows comment lines.
//! Quota fields live at copilotUserCache.<hash>.response.quota_snapshots.<quota_id>.
//!
//! These account premium-request quotas span devices/interfaces. The observed sample had
//! an allowance of 1500, not a universal limit. Display native requests separately, without token conversion.
//! timestamp_utc is the service snapshot time. The 2026-10-01 conclusion about absent
//! per-call tokens applied to the inspected chronicle session-store.db. Native VS Code
//! chatSessions/*.jsonl usage/round records are read by copilot_chat; quotas remain separate.

use std::collections::BTreeMap;
use std::path::PathBuf;

/// Account quota snapshot in request units, separate from tokens.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CopilotQuota {
    /// Quota ID: premium_interactions, chat or completions.
    pub quota_id: String,
    /// Billing-window allowance; zero is an unlimited placeholder.
    pub entitlement: Option<i64>,
    /// Remaining allowance in milli-requests; missing/inexact values stay unknown.
    pub remaining: Option<i64>,
    /// Used = allowance - remaining only for a positive, non-unlimited allowance.
    pub used: Option<i64>,
    /// Native remaining percentage.
    pub percent_remaining: Option<f64>,
    /// Unlimited flag; chat/completions are usually unlimited and have no derived used count.
    pub unlimited: bool,
    /// Service snapshot timestamp from timestamp_utc, in milliseconds.
    pub observed_at_ms: Option<i64>,
}

/// Parsed account plan label and quota snapshots.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CopilotUsageCache {
    pub plan: Option<String>,
    pub quotas: Vec<CopilotQuota>,
}

/// Strip comment lines before parsing copilot-user-cache.json.
pub fn parse_cache(text: &str) -> Option<CopilotUsageCache> {
    // Remove lines starting with // after whitespace, preserving the JSON body.
    let json = text
        .trim_start_matches('\u{feff}')
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let value: serde_json::Value = crate::adapters::run_policy::json_from_str(&json).ok()?;
    let cache = value.get("copilotUserCache")?.as_object()?;
    // Select the greatest retrievedAt; missing timestamps sort before known timestamps.
    let entry = cache
        .values()
        .filter_map(|v| v.as_object())
        .max_by_key(|o| {
            o.get("retrievedAt")
                .and_then(|v| v.as_str())
                .and_then(parse_iso_ms)
        })?;
    let response = entry.get("response")?.as_object()?;
    let mut out = CopilotUsageCache {
        plan: response
            .get("copilot_plan")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        quotas: Vec::new(),
    };
    let snapshots = response.get("quota_snapshots").and_then(|v| v.as_object());
    if let Some(snapshots) = snapshots {
        // Emit quota IDs in stable lexical order.
        let ordered: BTreeMap<&String, &serde_json::Value> = snapshots.iter().collect();
        for (quota_id, snap) in ordered {
            let Some(snap) = snap.as_object() else {
                continue;
            };
            // has_quota=false means this account lacks the quota; skip it.
            if snap
                .get("has_quota")
                .and_then(|v| v.as_bool())
                .is_some_and(|b| !b)
            {
                continue;
            }
            // Preserve fractional premium requests as integer milli-requests without whole-request rounding.
            let milli = |v: &serde_json::Value| -> Option<i64> {
                if let Some(n) = v.as_i64() {
                    return n.checked_mul(1000).filter(|n| *n >= 0);
                }
                let n = v.as_f64()? * 1000.0;
                if !n.is_finite() || n < 0.0 || n >= i64::MAX as f64 || (n - n.round()).abs() > 1e-6
                {
                    return None;
                }
                Some(n.round() as i64)
            };
            let entitlement = snap.get("entitlement").and_then(milli);
            let remaining = snap
                .get("quota_remaining")
                .or_else(|| snap.get("remaining"))
                .and_then(milli);
            let unlimited = snap
                .get("unlimited")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            // Derive used only for a positive limited allowance; otherwise leave it unknown.
            let used = if !unlimited {
                entitlement
                    .zip(remaining)
                    .and_then(|(limit, left)| (limit > 0 && left <= limit).then(|| limit - left))
            } else {
                None
            };
            let percent_remaining = snap
                .get("percent_remaining")
                .and_then(serde_json::Value::as_f64)
                .filter(|n| n.is_finite() && (0.0..=100.0).contains(n));
            let observed_at_ms = snap
                .get("timestamp_utc")
                .and_then(|v| v.as_str())
                .and_then(parse_iso_ms)
                .filter(|n| *n >= crate::domain::MIN_PLAUSIBLE_MS);
            out.quotas.push(CopilotQuota {
                quota_id: quota_id.clone(),
                entitlement,
                remaining,
                used,
                percent_remaining,
                unlimited,
                observed_at_ms,
            });
        }
    }
    Some(out)
}

fn parse_iso_ms(raw: &str) -> Option<i64> {
    let ts: jiff::Timestamp = raw.trim().parse().ok()?;
    Some(ts.as_millisecond())
}

/// Resolve platform cache paths with COPILOT_CACHE_HOME override.
/// Accept an explicit environment map for isolated tests, without process-environment reads.
pub fn cache_path(
    env: &BTreeMap<String, String>,
    home: Option<&std::path::Path>,
) -> Option<PathBuf> {
    let file = "copilot-user-cache.json";
    if let Some(dir) = env
        .get("COPILOT_CACHE_HOME")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        return Some(PathBuf::from(dir).join(file));
    }
    if cfg!(windows) {
        if let Some(local) = env.get("LOCALAPPDATA").filter(|s| !s.trim().is_empty()) {
            return Some(PathBuf::from(local).join("copilot").join(file));
        }
    }
    let home = home?;
    if cfg!(target_os = "macos") {
        return Some(
            home.join("Library")
                .join("Caches")
                .join("copilot")
                .join(file),
        );
    }
    // Linux/other platforms prefer XDG_CACHE_HOME, then ~/.cache.
    let base = env
        .get("XDG_CACHE_HOME")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".cache"));
    Some(base.join("copilot").join(file))
}

/// Read the local cache; missing/unparseable files return None.
pub fn read_from(
    env: &BTreeMap<String, String>,
    home: Option<&std::path::Path>,
) -> Option<CopilotUsageCache> {
    let path = cache_path(env, home)?;
    use std::io::Read as _;
    const MAX_CACHE_BYTES: u64 = 8 * 1024 * 1024;
    let mut text = String::new();
    crate::adapters::run_policy::checked_file(&path)
        .ok()?
        .take(MAX_CACHE_BYTES + 1)
        .read_to_string(&mut text)
        .ok()?;
    if text.len() as u64 > MAX_CACHE_BYTES {
        return None;
    }
    parse_cache(&text)
}

// Map to Agent-independent quota_history.

use crate::error::CoreError;
use crate::quota_history::QuotaObservation;
use crate::storage::Storage;

/// Agent name used in statistics.
pub const AGENT: &str = "copilot";

/// Map parsed snapshots to generic QuotaObservation.
/// Include positive limited quotas such as premium_interactions;
/// account quotas span devices, so locality_verified=false; retain request units rather than tokens.
pub fn to_observations(cache: &CopilotUsageCache) -> Vec<QuotaObservation> {
    cache
        .quotas
        .iter()
        .filter(|q| {
            q.quota_id == "premium_interactions"
                && !q.unlimited
                && q.entitlement.is_some_and(|n| n > 0)
                && q.observed_at_ms.is_some()
        })
        .map(|q| QuotaObservation {
            observed_at_ms: q.observed_at_ms,
            agent: AGENT.to_string(),
            quota_id: q.quota_id.clone(),
            kind: "rate_limit".to_string(),
            unit: "milli_requests".to_string(),
            limit_value: q.entitlement,
            used: q.used,
            remaining: q.remaining,
            percent_remaining: q.percent_remaining,
            window_start_ms: None,
            window_end_ms: None,
            // An account quota does not establish local-device usage; locality_verified=false.
            locality_verified: false,
            detail: cache
                .plan
                .as_ref()
                .map(|plan| serde_json::json!({ "plan": plan })),
        })
        .collect()
}

/// Read and store local-cache observations in quota_history; return the number recorded.
pub fn collect(
    storage: &Storage,
    env: &BTreeMap<String, String>,
    home: Option<&std::path::Path>,
    timezone: &str,
    now_ms: i64,
) -> Result<u64, CoreError> {
    match read_from(env, home) {
        Some(cache) => {
            crate::quota_history::record(storage, &to_observations(&cache), timezone, now_ms)
        }
        None => Ok(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"// Disposable cache for Copilot user responses, safe to delete.
// User settings belong in settings.json.
{
  "copilotUserCache": {
    "v1:hash": {
      "schemaVersion": 1,
      "retrievedAt": "2026-09-30T14:31:38.369Z",
      "response": {
        "login": "sample-user",
        "copilot_plan": "individual_pro",
        "quota_snapshots": {
          "chat": { "entitlement": 0, "percent_remaining": 100, "quota_id": "chat", "quota_remaining": 0, "remaining": 0, "unlimited": true, "timestamp_utc": "2026-09-30T07:33:03.355-07:00", "has_quota": true },
          "completions": { "entitlement": 0, "percent_remaining": 100, "quota_id": "completions", "quota_remaining": 0, "remaining": 0, "unlimited": true, "timestamp_utc": "2026-09-30T07:33:03.355-07:00", "has_quota": true },
          "premium_interactions": { "entitlement": 1500, "percent_remaining": 9.1, "quota_id": "premium_interactions", "quota_remaining": 137.4, "remaining": 137, "unlimited": false, "timestamp_utc": "2026-09-30T07:33:03.355-07:00", "has_quota": true }
        }
      }
    }
  }
}"#;

    #[test]
    fn parses_premium_interactions_usage() {
        let cache = parse_cache(SAMPLE).expect("parse");
        assert_eq!(cache.plan.as_deref(), Some("individual_pro"));
        let premium = cache
            .quotas
            .iter()
            .find(|q| q.quota_id == "premium_interactions")
            .expect("premium");
        assert_eq!(premium.entitlement, Some(1_500_000));
        assert_eq!(premium.remaining, Some(137_400));
        assert_eq!(premium.used, Some(1_362_600));
        assert!(!premium.unlimited);
        // Parse timestamp_utc with its -07:00 offset into UTC milliseconds.
        assert!(premium.observed_at_ms.is_some_and(|t| t > 0));
    }

    #[test]
    fn unlimited_quotas_have_no_used() {
        let cache = parse_cache(SAMPLE).unwrap();
        let chat = cache.quotas.iter().find(|q| q.quota_id == "chat").unwrap();
        assert!(chat.unlimited);
        assert_eq!(chat.used, None, "unlimited 不派生已用");
    }

    #[test]
    fn junk_or_missing_returns_none() {
        assert!(parse_cache("not json").is_none());
        assert!(parse_cache("// only a comment\n").is_none());
    }

    #[test]
    fn cache_path_prefers_env_override() {
        let mut env = BTreeMap::new();
        env.insert("COPILOT_CACHE_HOME".to_string(), "/tmp/cc".to_string());
        let p = cache_path(&env, None).unwrap();
        assert!(p.ends_with("copilot-user-cache.json"));
        assert!(p.to_string_lossy().contains("cc"));
    }

    #[test]
    fn missing_or_invalid_quota_fields_remain_unknown() {
        let cache = parse_cache(
            r#"{"copilotUserCache":{"entry":{"response":{"quota_snapshots":{
          "premium_interactions":{"entitlement":100,"timestamp_utc":"2026-10-01T00:00:00Z"}
        }}}}}"#,
        )
        .unwrap();
        let q = &cache.quotas[0];
        assert_eq!(q.remaining, None);
        assert_eq!(q.used, None);
        assert_eq!(q.percent_remaining, None);
        assert_eq!(to_observations(&cache)[0].remaining, None);
        let invalid = parse_cache(
            r#"{"copilotUserCache":{"entry":{"response":{"quota_snapshots":{
          "premium_interactions":{"entitlement":100,"quota_remaining":-2,"percent_remaining":110}
        }}}}}"#,
        )
        .unwrap();
        assert_eq!(invalid.quotas[0].remaining, None);
        assert_eq!(invalid.quotas[0].percent_remaining, None);
        assert!(
            to_observations(&invalid).is_empty(),
            "missing source time cannot create a fresh snapshot"
        );
    }

    #[test]
    fn collect_preserves_source_time_fraction_and_repeated_cache_identity() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../build/copilot-review/quota-test")
            .join(format!(
                "{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(&dir).unwrap();
        let storage = Storage::open(&dir.join("quota.sqlite")).unwrap();
        std::fs::write(dir.join("copilot-user-cache.json"), SAMPLE).unwrap();
        let env = BTreeMap::from([(
            "COPILOT_CACHE_HOME".to_string(),
            dir.to_string_lossy().to_string(),
        )]);
        assert_eq!(
            collect(&storage, &env, None, "UTC", 1_800_000_000_000).unwrap(),
            1
        );
        assert_eq!(
            collect(&storage, &env, None, "UTC", 1_800_000_001_000).unwrap(),
            0
        );
        let q = &crate::quota_history::latest(&storage, Some(AGENT)).unwrap()[0];
        assert_eq!(
            q.observed_at_ms,
            parse_iso_ms("2026-09-30T14:33:03.355Z").unwrap()
        );
        assert_eq!(q.unit, "milli_requests");
        assert_eq!(q.used, Some(1_362_600));
        assert_eq!(
            crate::quota_history::daily_series(&storage, AGENT, "premium_interactions", "UTC")
                .unwrap()[0]
                .local_day,
            "2026-09-30"
        );
    }
}
