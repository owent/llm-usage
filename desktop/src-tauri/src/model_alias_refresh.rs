//! Opt-in public model-name metadata. Never use display mappings as usage or price data.

use crate::app_state::{load_settings, AppState};
use llm_usage_core::model_aliases::{self, read_state, MAX_CATALOG_BYTES};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const CATALOG_URL: &str = "https://llm-usage.atframe.work/models/aliases.json";
pub const FALLBACK_URL: &str =
    "https://raw.githubusercontent.com/owent/llm-usage/main/docs/site/public/models/aliases.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Settings {
    #[serde(default)]
    pub online_refresh_enabled: bool,
    #[serde(default = "default_ttl")]
    pub cache_ttl_days: u32,
}

fn default_ttl() -> u32 {
    3
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            online_refresh_enabled: false,
            cache_ttl_days: default_ttl(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Outcome {
    pub status: String,
    pub generation: u64,
    pub inserted_rules: usize,
    pub error: Option<String>,
    pub first_error: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct RefreshState {
    pub running: bool,
    pub last_outcome: Option<Outcome>,
}

pub fn safe_error(error: &llm_usage_core::CoreError) -> String {
    match error {
        llm_usage_core::CoreError::Validation(code) if code.starts_with("model_alias_") => {
            code.clone()
        }
        _ => "model_alias_storage".into(),
    }
}

/// No redirects, authentication, query strings, local data or arbitrary endpoints.
pub fn http_fetch(url: &str) -> Result<Vec<u8>, String> {
    if ![CATALOG_URL, FALLBACK_URL].contains(&url) {
        return Err("model_alias_source".into());
    }
    let agent = ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(0)
        .timeout_connect(Some(std::time::Duration::from_secs(10)))
        .timeout_global(Some(std::time::Duration::from_secs(25)))
        .user_agent(concat!("llm-usage/", env!("CARGO_PKG_VERSION")))
        .build()
        .new_agent();
    let mut response = agent
        .get(url)
        .call()
        .map_err(|_| "model_alias_download".to_string())?;
    if response.status() != 200 {
        return Err(format!("model_alias_http_{}", response.status().as_u16()));
    }
    response
        .body_mut()
        .with_config()
        .limit(MAX_CATALOG_BYTES as u64)
        .read_to_vec()
        .map_err(|_| "model_alias_response_size".into())
}

pub fn cache_fresh(state: &model_aliases::State, now: i64, ttl: u32) -> bool {
    state.cache.as_ref().is_some_and(|cache| {
        cache.fetched_at_ms <= now.saturating_add(300_000)
            && now.saturating_sub(cache.fetched_at_ms) < i64::from(ttl.clamp(1, 365)) * 86_400_000
    })
}

/// Validate downloaded catalogues before writer/cache mutation; preserve the first failure.
pub fn download_with(
    now: i64,
    fetch: &dyn Fn(&str) -> Result<Vec<u8>, String>,
) -> Result<(String, Option<String>), (String, Option<String>)> {
    let mut first = None;
    for url in [CATALOG_URL, FALLBACK_URL] {
        let result = fetch(url).and_then(|bytes| {
            if bytes.len() > MAX_CATALOG_BYTES {
                return Err("model_alias_size".into());
            }
            let raw = String::from_utf8(bytes).map_err(|_| "model_alias_utf8".to_string())?;
            model_aliases::parse_catalog(&raw, now).map_err(|error| safe_error(&error))?;
            Ok(raw)
        });
        match result {
            Ok(raw) => return Ok((raw, first)),
            Err(error) => {
                if first.is_none() {
                    first = Some(error.clone());
                } else {
                    return Err((error, first));
                }
            }
        }
    }
    Err(("model_alias_download".into(), first))
}

fn finish(state: &AppState, outcome: Outcome) {
    {
        let storage = state.storage.lock().unwrap();
        crate::commands::log_operation(
            &storage,
            "model_alias_refresh",
            &format!(
                "status={} generation={} error={:?} first_error={:?}",
                outcome.status, outcome.generation, outcome.error, outcome.first_error
            ),
        );
    }
    let mut refresh = state.model_alias_refresh.lock().unwrap();
    refresh.running = false;
    refresh.last_outcome = Some(outcome);
}

pub fn maybe_auto_refresh(state: &Arc<AppState>, force: bool) -> bool {
    let prepared = {
        let storage = state.storage.lock().unwrap();
        let settings = load_settings(&storage).model_aliases;
        if !settings.online_refresh_enabled {
            return false;
        }
        match read_state(&storage) {
            Ok((catalog, etag)) => {
                if !force
                    && cache_fresh(&catalog, crate::scanner::now_ms(), settings.cache_ttl_days)
                {
                    return false;
                }
                model_aliases::settings_stamp(&storage)
                    .map(|stamp| (settings, catalog, etag, stamp))
                    .map_err(|error| safe_error(&error))
            }
            Err(error) => Err(safe_error(&error)),
        }
    };
    {
        let mut refresh = state.model_alias_refresh.lock().unwrap();
        if refresh.running {
            return false;
        }
        refresh.running = true;
    }
    let state = Arc::clone(state);
    std::thread::spawn(move || {
        let (settings, previous, etag, stamp) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                finish(
                    &state,
                    Outcome {
                        status: "failed".into(),
                        error: Some(error),
                        ..Default::default()
                    },
                );
                return;
            }
        };
        let downloaded = download_with(crate::scanner::now_ms(), &http_fetch);
        let outcome = match downloaded {
            Ok((raw, first_error)) => {
                // Match settings and complete state after network I/O, using the one writer.
                let storage = state.storage.lock().unwrap();
                if load_settings(&storage).model_aliases != settings {
                    Outcome {
                        status: "superseded".into(),
                        generation: previous.generation,
                        first_error,
                        ..Default::default()
                    }
                } else {
                    match model_aliases::import_online_catalog(
                        &storage,
                        &raw,
                        crate::scanner::now_ms(),
                        &etag,
                        stamp,
                    ) {
                        Ok(imported) => Outcome {
                            status: "fetched".into(),
                            generation: imported.generation,
                            inserted_rules: imported.inserted_rules,
                            first_error,
                            error: None,
                        },
                        Err(error) => Outcome {
                            status: if safe_error(&error) == "model_alias_stale_worker" {
                                "superseded"
                            } else {
                                "failed_used_cache"
                            }
                            .into(),
                            generation: previous.generation,
                            error: Some(safe_error(&error)),
                            first_error,
                            ..Default::default()
                        },
                    }
                }
            }
            Err((error, first_error)) => Outcome {
                status: if previous.cache.is_some() {
                    "failed_used_cache"
                } else {
                    "failed"
                }
                .into(),
                generation: previous.generation,
                error: Some(error),
                first_error,
                ..Default::default()
            },
        };
        finish(&state, outcome);
    });
    true
}

#[tauri::command]
pub fn model_alias_status(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let storage = crate::app_state::read_conn(&state);
    let settings = load_settings(&storage).model_aliases;
    let data = read_state(&storage);
    let refresh = state.model_alias_refresh.lock().unwrap();
    Ok(match data {
        Ok((catalog, _)) => {
            serde_json::json!({"enabled":settings.online_refresh_enabled,"ttl_days":settings.cache_ttl_days,
            "running":refresh.running,"last_outcome":refresh.last_outcome,"generation":catalog.generation,
            "revision":model_aliases::revision(&storage),"rules":catalog.candidates.len(),
            "cache":catalog.cache.as_ref().map(|c|serde_json::json!({"fetched_at_ms":c.fetched_at_ms,"rule_version":c.rule_version,"content_hash":c.content_hash})),"error":null})
        }
        Err(error) => {
            serde_json::json!({"enabled":settings.online_refresh_enabled,"ttl_days":settings.cache_ttl_days,
            "running":refresh.running,"last_outcome":refresh.last_outcome,"generation":null,"revision":"model-alias-invalid",
            "rules":0,"cache":null,"error":safe_error(&error)})
        }
    })
}

#[tauri::command]
pub fn refresh_model_aliases(state: tauri::State<'_, Arc<AppState>>) -> Result<bool, String> {
    if !load_settings(&state.storage.lock().unwrap())
        .model_aliases
        .online_refresh_enabled
    {
        return Err(
            serde_json::json!({"code":"disabled","message":"model_alias_disabled"}).to_string(),
        );
    }
    Ok(maybe_auto_refresh(&state, true))
}

#[tauri::command]
pub fn import_model_aliases(
    state: tauri::State<'_, Arc<AppState>>,
    path: String,
) -> Result<model_aliases::ImportOutcome, String> {
    use std::io::Read;
    let fail = || {
        serde_json::json!({"code":"invalid_model_aliases","message":"model_alias_file"}).to_string()
    };
    let file = std::fs::File::open(path).map_err(|_| fail())?;
    let meta = file.metadata().map_err(|_| fail())?;
    if !meta.is_file() || meta.len() > MAX_CATALOG_BYTES as u64 {
        return Err(fail());
    }
    let mut file = file.take(MAX_CATALOG_BYTES as u64 + 1);
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|_| fail())?;
    if bytes.len() > MAX_CATALOG_BYTES {
        return Err(fail());
    }
    let raw = String::from_utf8(bytes).map_err(|_| fail())?;
    let storage = state.storage.lock().unwrap();
    let outcome = model_aliases::import_catalog(
        &storage,
        &raw,
        crate::scanner::now_ms(),
        "manual",
        None,
        false,
    )
    .map_err(|error| {
        serde_json::json!({"code":"invalid_model_aliases","message":safe_error(&error)}).to_string()
    })?;
    crate::commands::log_operation(
        &storage,
        "model_alias_import",
        &format!(
            "generation={} rules={}",
            outcome.generation, outcome.inserted_rules
        ),
    );
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_are_off_and_requests_only_use_the_two_public_paths() {
        assert!(!Settings::default().online_refresh_enabled);
        assert_eq!(Settings::default().cache_ttl_days, 3);
        let calls = std::cell::RefCell::new(Vec::new());
        let error = download_with(1_794_960_000_000, &|url| {
            calls.borrow_mut().push(url.to_string());
            Err("test_failure".into())
        })
        .unwrap_err();
        assert_eq!(*calls.borrow(), [CATALOG_URL, FALLBACK_URL]);
        assert_eq!(error.1.as_deref(), Some("test_failure"));
        assert!(http_fetch("https://example.invalid/private?token=not-sent").is_err());
    }
    #[test]
    fn malformed_downloads_are_rejected_before_any_cache_write() {
        let result = download_with(1_794_960_000_000, &|_| {
            Ok(b"{\"private\":\"do-not-echo\"}".to_vec())
        })
        .unwrap_err();
        assert!(!result.0.contains("do-not-echo"));
        let state = model_aliases::State {
            cache: Some(model_aliases::CachedCatalog {
                raw: String::new(),
                content_hash: String::new(),
                fetched_at_ms: 1000,
                published_at_ms: 1000,
                rule_version: String::new(),
            }),
            ..Default::default()
        };
        assert!(cache_fresh(&state, 1000 + 3 * 86_400_000 - 1, 3));
        assert!(!cache_fresh(&state, 1000 + 3 * 86_400_000, 3));
    }

    #[test]
    fn legacy_settings_stay_off_and_monotonic_settings_stamp_rejects_aba_workers() {
        use crate::app_state::{save_settings, AppSettings};
        let storage = llm_usage_core::storage::Storage::open_in_memory().unwrap();
        let mut old = serde_json::to_value(AppSettings::default()).unwrap();
        old.as_object_mut().unwrap().remove("model_aliases");
        storage
            .conn()
            .execute(
                "INSERT INTO settings VALUES('app_settings',?1,1,0)",
                [old.to_string()],
            )
            .unwrap();
        let mut settings = load_settings(&storage);
        assert_eq!(settings.model_aliases, Settings::default());
        settings.model_aliases.online_refresh_enabled = true;
        save_settings(&storage, &settings).unwrap();
        let stamp = model_aliases::settings_stamp(&storage).unwrap();
        settings.model_aliases.online_refresh_enabled = false;
        save_settings(&storage, &settings).unwrap();
        settings.model_aliases.online_refresh_enabled = true;
        save_settings(&storage, &settings).unwrap();
        assert!(model_aliases::settings_stamp(&storage).unwrap() > stamp);
        let raw = include_str!("../../../docs/site/public/models/aliases.json");
        assert!(model_aliases::import_online_catalog(
            &storage,
            raw,
            1_794_960_000_000,
            "missing",
            stamp
        )
        .is_err());
        assert_eq!(read_state(&storage).unwrap().0.generation, 0);
        settings.model_aliases.cache_ttl_days = 0;
        save_settings(&storage, &settings).unwrap();
        assert_eq!(load_settings(&storage).model_aliases.cache_ttl_days, 3);
    }

    #[test]
    fn validated_fallback_retains_first_failure_and_bad_downloads_leave_successful_cache() {
        let storage = llm_usage_core::storage::Storage::open_in_memory().unwrap();
        let raw = include_str!("../../../docs/site/public/models/aliases.json");
        let now = 1_794_960_000_000;
        let (body, first) = download_with(now, &|url| {
            if url == CATALOG_URL {
                Err("model_alias_download".into())
            } else {
                Ok(raw.as_bytes().to_vec())
            }
        })
        .unwrap();
        assert_eq!(first.as_deref(), Some("model_alias_download"));
        model_aliases::import_online_catalog(&storage, &body, now, "missing", 0).unwrap();
        let (before, token) = read_state(&storage).unwrap();
        assert!(download_with(now, &|_| Ok(vec![0xff])).is_err());
        assert_eq!(read_state(&storage).unwrap().1, token);
        assert_eq!(before.cache.unwrap().raw, raw);
        assert!(cache_fresh(&read_state(&storage).unwrap().0, now + 1, 3));
    }
}
