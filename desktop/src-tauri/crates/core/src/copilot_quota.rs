//! GitHub Copilot 本机额度记录提取（`copilot-user-cache.json`）。
//!
//! 载体证据（2026-10-01 本机核验 + 官方 cli-config-dir-reference「cache 目录」）：
//! VS Code Copilot Chat 与 Copilot CLI 共享的账户额度缓存文件
//! `copilot-user-cache.json`，Windows 位于 `%LOCALAPPDATA%/copilot/`，
//! macOS `~/Library/Caches/copilot/`，Linux `$XDG_CACHE_HOME/copilot` 或
//! `~/.cache/copilot/`。文件以 `//` 注释行开头，随后为 JSON：
//! `copilotUserCache.<hash>.response.quota_snapshots.<quota_id>`。
//!
//! **口径边界**：这是账户级「premium 请求额度」（所有设备/入口共享同一 1500
//! 额度），是请求配额而非逐次 token；按额度快照独立展示，绝不折算成 token。
//! `timestamp_utc` 为服务端快照时刻。（2026-10-01 更正：chronicle session-store.db
//! 无逐次 token 的结论只覆盖该库；VS Code 原生 `chatSessions/*.jsonl` 会话日志
//! 携带逐请求 token，由 `copilot_chat` 适配器接入，额度记录继续独立保留。）

use std::collections::BTreeMap;
use std::path::PathBuf;

/// 单条额度快照（账户级；请求计数，非 token）。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CopilotQuota {
    /// 额度标识：premium_interactions / chat / completions。
    pub quota_id: String,
    /// 本计费周期配额上限（0 表示无上限占位）。
    pub entitlement: Option<i64>,
    /// 剩余额度，千分之一请求；缺失/不可精确映射为未知。
    pub remaining: Option<i64>,
    /// 已用额度 = entitlement − remaining（有上限且非 unlimited 时才有意义）。
    pub used: Option<i64>,
    /// 剩余百分比（服务端原值）。
    pub percent_remaining: Option<f64>,
    /// 是否无上限（chat/completions 通常 true：不计入已用）。
    pub unlimited: bool,
    /// 快照观测时刻（来自 timestamp_utc，毫秒）。
    pub observed_at_ms: Option<i64>,
}

/// 解析结果：账户计划标识 + 各额度快照。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CopilotUsageCache {
    pub plan: Option<String>,
    pub quotas: Vec<CopilotQuota>,
}

/// 解析 `copilot-user-cache.json` 文本（先剥离 `//` 注释行再按 JSON 解析）。
pub fn parse_cache(text: &str) -> Option<CopilotUsageCache> {
    // 文件头是 `//` 注释行；从第一个 `{` 起才是 JSON。
    let json = text
        .trim_start_matches('\u{feff}')
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let value: serde_json::Value = serde_json::from_str(&json).ok()?;
    let cache = value.get("copilotUserCache")?.as_object()?;
    // 取最新一条（按 retrievedAt 排序；缺失则任取其一，通常仅一条）。
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
        // 按 quota_id 字典序稳定输出。
        let ordered: BTreeMap<&String, &serde_json::Value> = snapshots.iter().collect();
        for (quota_id, snap) in ordered {
            let Some(snap) = snap.as_object() else {
                continue;
            };
            // has_quota=false 表示该额度对本账户不适用：跳过。
            if snap
                .get("has_quota")
                .and_then(|v| v.as_bool())
                .is_some_and(|b| !b)
            {
                continue;
            }
            // premium 额度可为小数；用千分之一请求的整数存储，禁止取整。
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
            // 已用仅在有正上限且非 unlimited 时可派生（配额语义）；否则未知不补零。
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

/// 解析缓存文件默认路径（平台 cache 目录；COPILOT_CACHE_HOME 覆盖）。
/// 传入显式 env map 便于测试，不直接读进程环境。
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
    // Linux / 其他：XDG_CACHE_HOME 优先，否则 ~/.cache。
    let base = env
        .get("XDG_CACHE_HOME")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".cache"));
    Some(base.join("copilot").join(file))
}

/// 读取并解析本机 Copilot 额度缓存（文件缺失/不可解析返回 None）。
pub fn read_from(
    env: &BTreeMap<String, String>,
    home: Option<&std::path::Path>,
) -> Option<CopilotUsageCache> {
    let path = cache_path(env, home)?;
    let text = std::fs::read_to_string(path).ok()?;
    parse_cache(&text)
}

// ---- 归一到通用额度时序（agent 无关 quota_history）----

use crate::error::CoreError;
use crate::quota_history::QuotaObservation;
use crate::storage::Storage;

/// 统计 Agent 名。
pub const AGENT: &str = "copilot";

/// 把解析出的额度快照映射为通用 [`QuotaObservation`]。
/// 仅纳入有真实上限且非 unlimited 的额度（premium_interactions）：
/// 账户级请求配额（所有设备共享 ⇒ locality_verified=false），按请求计数、非 token。
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
            // 账户级配额跨设备共享，不可证明属于本机：locality_verified=false。
            locality_verified: false,
            detail: cache
                .plan
                .as_ref()
                .map(|plan| serde_json::json!({ "plan": plan })),
        })
        .collect()
}

/// 便捷：从本机缓存读取并记录到通用 quota_history；返回记录的额度条数。
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
        // timestamp_utc -07:00 → UTC 毫秒。
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
