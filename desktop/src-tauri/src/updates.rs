use crate::app_state::AppState;
use crate::update_package::{self, PackageIdentity};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
use tauri::Manager;

const RELEASE_API: &str = "https://api.github.com/repos/owent/llm-usage/releases/latest";
const DOCUMENTATION_FEED: &str = "https://llm-usage.atframe.work/updates/latest.json";
pub(crate) const MAX_DOWNLOAD: u64 = 512 * 1024 * 1024;
const DAY: i64 = 86_400_000;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Schedule {
    Manual,
    Launch,
    #[default]
    Daily,
    Weekly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateSettings {
    pub schedule: Schedule,
    pub auto_download: bool,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self {
            schedule: Schedule::Daily,
            auto_download: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CheckClock {
    success_ms: Option<i64>,
    attempt_ms: Option<i64>,
}

fn due(schedule: Schedule, clock: &CheckClock, now: i64, launch: bool) -> bool {
    if schedule == Schedule::Manual || (schedule == Schedule::Launch && !launch) {
        return false;
    }
    if !launch
        && clock
            .attempt_ms
            .is_some_and(|last| now >= last && now.saturating_sub(last) < 3_600_000)
    {
        return false;
    }
    if schedule == Schedule::Launch {
        return true;
    }
    let interval = if schedule == Schedule::Weekly {
        7 * DAY
    } else {
        DAY
    };
    clock
        .success_ms
        .is_none_or(|last| now < last || now.saturating_sub(last) >= interval)
        && clock
            .attempt_ms
            .is_none_or(|last| now < last || now.saturating_sub(last) >= 3_600_000)
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateStatus {
    pub phase: String,
    pub current_version: String,
    pub package_kind: String,
    pub version: Option<String>,
    pub asset_name: Option<String>,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub last_checked_ms: Option<i64>,
    pub error: Option<String>,
    pub previous_error: Option<String>,
}

impl Default for UpdateStatus {
    fn default() -> Self {
        Self {
            phase: "idle".into(),
            current_version: env!("CARGO_PKG_VERSION").into(),
            package_kind: "unknown".into(),
            version: None,
            asset_name: None,
            downloaded_bytes: 0,
            total_bytes: 0,
            last_checked_ms: None,
            error: None,
            previous_error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Candidate {
    pub version: String,
    pub name: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    state: String,
    size: u64,
    digest: Option<String>,
    browser_download_url: String,
}

fn stable_version(value: &str) -> Result<semver::Version, String> {
    let version = semver::Version::parse(value).map_err(|_| "invalid_release_version")?;
    if !version.pre.is_empty() || !version.build.is_empty() || version.to_string() != value {
        return Err("invalid_release_version".into());
    }
    Ok(version)
}

#[derive(Debug, PartialEq, Eq)]
struct CheckedRelease {
    version: Option<String>,
    candidate: Option<Candidate>,
}

fn check_release(
    bytes: &[u8],
    current: &str,
    identity: Option<&PackageIdentity>,
) -> Result<CheckedRelease, String> {
    let release: Release = serde_json::from_slice(bytes).map_err(|_| "invalid_release_metadata")?;
    if release.draft || release.prerelease {
        return Err("release_is_not_stable".into());
    }
    let value = release
        .tag_name
        .strip_prefix('v')
        .ok_or("invalid_release_tag")?;
    if stable_version(value)? <= stable_version(current)? {
        return Ok(CheckedRelease {
            version: None,
            candidate: None,
        });
    }
    let Some(identity) = identity else {
        return Ok(CheckedRelease {
            version: Some(value.into()),
            candidate: None,
        });
    };
    let name = identity.asset_name(value)?;
    let matches: Vec<_> = release.assets.iter().filter(|a| a.name == name).collect();
    if matches.len() != 1 {
        return Err("matching_update_package_missing_or_duplicated".into());
    }
    let asset = matches[0];
    let candidate = Candidate {
        version: value.into(),
        name,
        url: asset.browser_download_url.clone(),
        size: asset.size,
        sha256: asset
            .digest
            .as_deref()
            .and_then(|d| d.strip_prefix("sha256:"))
            .ok_or("update_digest_missing")?
            .into(),
    };
    validate_candidate(&candidate, identity)?;
    if asset.state != "uploaded" {
        return Err("update_asset_not_uploaded".into());
    }
    Ok(CheckedRelease {
        version: Some(value.into()),
        candidate: Some(candidate),
    })
}

#[cfg(test)]
fn select_release(
    bytes: &[u8],
    current: &str,
    identity: &PackageIdentity,
) -> Result<Option<Candidate>, String> {
    Ok(check_release(bytes, current, Some(identity))?.candidate)
}

pub(crate) fn validate_candidate(
    value: &Candidate,
    identity: &PackageIdentity,
) -> Result<(), String> {
    stable_version(&value.version)?;
    if value.name != identity.asset_name(&value.version)?
        || value.size == 0
        || value.size > MAX_DOWNLOAD
        || value.sha256.len() != 64
        || !value
            .sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        || value.url
            != format!(
                "https://github.com/owent/llm-usage/releases/download/v{}/{}",
                value.version, value.name
            )
    {
        return Err("invalid_update_asset".into());
    }
    Ok(())
}

fn allowed_url(url: &str, metadata: bool) -> bool {
    if metadata {
        return url == RELEASE_API || url == DOCUMENTATION_FEED;
    }
    url.starts_with("https://github.com/owent/llm-usage/releases/download/")
        || url.starts_with("https://release-assets.githubusercontent.com/")
        || url.starts_with("https://objects.githubusercontent.com/")
}

fn http_get(url: &str, metadata: bool) -> Result<ureq::http::Response<ureq::Body>, String> {
    let agent = ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(0)
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_global(Some(Duration::from_secs(if metadata { 30 } else { 600 })))
        .user_agent(concat!("llm-usage-updater/", env!("CARGO_PKG_VERSION")))
        .build()
        .new_agent();
    let mut target = url.to_string();
    for _ in 0..4 {
        if !allowed_url(&target, metadata) {
            return Err("update_redirect_rejected".into());
        }
        let response = agent
            .get(&target)
            .header(
                "Accept",
                if metadata && target == RELEASE_API {
                    "application/vnd.github+json"
                } else if metadata {
                    "application/json"
                } else {
                    "application/octet-stream"
                },
            )
            .call()
            .map_err(|_| "update_network_request_failed")?;
        if response.status() == 200 {
            return Ok(response);
        }
        if matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308) {
            target = response
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .ok_or("invalid_update_redirect")?
                .into();
            if metadata && target != url {
                return Err("update_redirect_rejected".into());
            }
        } else {
            return Err(format!("update_http_{}", response.status().as_u16()));
        }
    }
    Err("too_many_update_redirects".into())
}

fn read_metadata(url: &str) -> Result<Vec<u8>, String> {
    http_get(url, true)?
        .body_mut()
        .with_config()
        .limit(1024 * 1024)
        .read_to_vec()
        .map_err(|_| "update_metadata_too_large_or_unreadable".into())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentationFeed {
    schema: u32,
    repository: String,
    generated_at: String,
    release: serde_json::Value,
}

fn documentation_release(bytes: &[u8], now: i64) -> Result<Vec<u8>, String> {
    let feed: DocumentationFeed =
        serde_json::from_slice(bytes).map_err(|_| "invalid_documentation_update_feed")?;
    if feed.schema != 1 || feed.repository != "owent/llm-usage" || !feed.generated_at.ends_with('Z')
    {
        return Err("invalid_documentation_update_feed".into());
    }
    let generated = feed
        .generated_at
        .parse::<jiff::Timestamp>()
        .map_err(|_| "invalid_documentation_update_feed")?
        .as_millisecond();
    if generated > now.saturating_add(300_000) || now.saturating_sub(generated) > 7 * DAY {
        return Err("stale_documentation_update_feed".into());
    }
    let release =
        serde_json::to_vec(&feed.release).map_err(|_| "invalid_documentation_update_feed")?;
    check_release(&release, "0.0.0", None)?;
    Ok(release)
}

fn fetch_release_with(
    mut read: impl FnMut(&str) -> Result<Vec<u8>, String>,
    now: i64,
    mut cancel: impl FnMut() -> Result<(), String>,
) -> Result<Vec<u8>, String> {
    cancel()?;
    match read(RELEASE_API) {
        Ok(bytes) => Ok(bytes),
        Err(primary) => {
            cancel()?;
            let fallback =
                read(DOCUMENTATION_FEED).and_then(|bytes| documentation_release(&bytes, now));
            cancel()?;
            fallback.map_err(|secondary| {
                format!(
                    "update_metadata_sources_failed: github={primary}; documentation={secondary}"
                )
            })
        }
    }
}

fn fetch_release(cancel: impl FnMut() -> Result<(), String>) -> Result<Vec<u8>, String> {
    fetch_release_with(read_metadata, crate::scanner::now_ms(), cancel)
}

#[derive(Default)]
struct Operation {
    status: UpdateStatus,
    identity: Option<PackageIdentity>,
    candidate: Option<Candidate>,
    clock: CheckClock,
    cache: PathBuf,
}

#[derive(Default)]
pub struct UpdateService {
    inner: Mutex<Operation>,
    cancel: AtomicBool,
}

fn busy(phase: &str) -> bool {
    matches!(
        phase,
        "checking" | "downloading" | "verifying" | "installing"
    )
}

fn clock_key(cache: &Path) -> String {
    format!(
        "update_clock:{}",
        cache.file_name().unwrap_or_default().to_string_lossy()
    )
}

fn save_clock(state: &AppState, cache: &Path, clock: &CheckClock) -> Result<(), String> {
    state
        .storage
        .lock()
        .unwrap()
        .conn()
        .execute(
            "INSERT INTO settings(key,value,schema_version,updated_at_ms) VALUES(?1,?2,1,?3)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value,updated_at_ms=excluded.updated_at_ms",
            rusqlite::params![
                clock_key(cache),
                serde_json::to_string(clock).map_err(|e| e.to_string())?,
                crate::scanner::now_ms()
            ],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn start_scheduler(app: tauri::AppHandle, state: Arc<AppState>) {
    let service = app.state::<UpdateService>();
    {
        let mut op = service.inner.lock().unwrap();
        op.identity = update_package::detect_identity().ok();
        if op.identity.is_none() && cfg!(debug_assertions) {
            op.status.package_kind = "development".into();
        }
        if let Some(identity) = op.identity.clone() {
            op.status.package_kind = identity.kind.clone();
            op.status.previous_error =
                update_package::previous_error(&identity).unwrap_or_else(Some);
        }
        op.cache = update_package::cache_directory(&state.db_path);
        let raw: Option<String> = state
            .storage
            .lock()
            .unwrap()
            .conn()
            .query_row(
                "SELECT value FROM settings WHERE key=?1",
                [clock_key(&op.cache)],
                |row| row.get(0),
            )
            .ok();
        op.clock = raw
            .and_then(|v| serde_json::from_str(&v).ok())
            .unwrap_or_default();
        op.status.last_checked_ms = op.clock.success_ms;
    }
    std::thread::spawn(move || {
        restore_download(&app);
        let mut launch = true;
        loop {
            let schedule = state.settings.lock().unwrap().updates.schedule;
            let should_check = {
                let service = app.state::<UpdateService>();
                let op = service.inner.lock().unwrap();
                !busy(&op.status.phase)
                    && due(schedule, &op.clock, crate::scanner::now_ms(), launch)
            };
            if should_check {
                let _ = begin_check(&app, Arc::clone(&state));
            }
            launch = false;
            std::thread::sleep(Duration::from_secs(60));
        }
    });
}

fn restore_download(app: &tauri::AppHandle) {
    let service = app.state::<UpdateService>();
    let (cache, identity, previous, known) = {
        let mut op = service.inner.lock().unwrap();
        if busy(&op.status.phase)
            || op.identity.is_none()
            || !op.cache.join("download.json").exists()
        {
            return;
        }
        let previous = op.status.phase.clone();
        op.status.phase = "verifying".into();
        (
            op.cache.clone(),
            op.identity.clone(),
            previous,
            op.candidate.clone(),
        )
    };
    if let Some(identity) = identity {
        let result = (|| {
            let bytes = update_package::read_small(&cache.join("download.json"))?;
            let candidate: Candidate =
                serde_json::from_slice(&bytes).map_err(|_| "invalid_download_metadata")?;
            validate_candidate(&candidate, &identity)?;
            if known.as_ref().is_some_and(|value| value != &candidate) {
                return Err("obsolete_download".into());
            }
            if stable_version(&candidate.version)? <= stable_version(env!("CARGO_PKG_VERSION"))? {
                return Err("obsolete_download".into());
            }
            update_package::verify_file(
                &cache.join(&candidate.name),
                candidate.size,
                &candidate.sha256,
            )?;
            update_package::validate_download(&identity, &candidate, &cache, || {
                cancelled(&service)
            })?;
            Ok::<_, String>(candidate)
        })();
        let mut op = service.inner.lock().unwrap();
        match result {
            Ok(candidate) => {
                op.status.version = Some(candidate.version.clone());
                op.status.asset_name = Some(candidate.name.clone());
                op.status.downloaded_bytes = candidate.size;
                op.status.total_bytes = candidate.size;
                op.candidate = Some(candidate);
                op.status.phase = "ready".into();
            }
            Err(error) => {
                op.status.phase = previous;
                if error == "update_cancelled" {
                    op.status.phase = "cancelled".into();
                } else if error != "obsolete_download" {
                    op.status.phase = "error".into();
                    op.status.error = Some(error);
                }
            }
        }
    }
}

fn finish_error(app: &tauri::AppHandle, error: String) {
    let service = app.state::<UpdateService>();
    let mut op = service.inner.lock().unwrap();
    op.status.phase = if error == "update_cancelled" {
        "cancelled"
    } else {
        "error"
    }
    .into();
    op.status.error = (error != "update_cancelled").then_some(error);
}

fn cancelled(service: &UpdateService) -> Result<(), String> {
    if service.cancel.load(Ordering::SeqCst) {
        Err("update_cancelled".into())
    } else {
        Ok(())
    }
}

fn begin_check(app: &tauri::AppHandle, state: Arc<AppState>) -> Result<(), String> {
    let service = app.state::<UpdateService>();
    let (cache, attempt) = {
        let mut op = service.inner.lock().unwrap();
        if busy(&op.status.phase) {
            return Err("update_operation_running".into());
        }
        op.clock.attempt_ms = Some(crate::scanner::now_ms());
        op.status.phase = "checking".into();
        op.status.error = None;
        service.cancel.store(false, Ordering::SeqCst);
        (op.cache.clone(), op.clock.clone())
    };
    let app = app.clone();
    std::thread::spawn(move || {
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), String> {
                save_clock(&state, &cache, &attempt)?;
                cancelled(&app.state::<UpdateService>())?;
                let bytes = fetch_release(|| cancelled(&app.state::<UpdateService>()))?;
                let service = app.state::<UpdateService>();
                cancelled(&service)?;
                let identity = service.inner.lock().unwrap().identity.clone();
                let checked = check_release(&bytes, env!("CARGO_PKG_VERSION"), identity.as_ref())?;
                let success = {
                    let mut op = service.inner.lock().unwrap();
                    op.clock.success_ms = Some(crate::scanner::now_ms());
                    op.clock.clone()
                };
                save_clock(&state, &cache, &success)?;
                cancelled(&service)?;
                {
                    let mut op = service.inner.lock().unwrap();
                    op.status.last_checked_ms = op.clock.success_ms;
                    op.status.version = checked.version.clone();
                    op.status.asset_name = checked.candidate.as_ref().map(|v| v.name.clone());
                    op.status.total_bytes = checked.candidate.as_ref().map_or(0, |v| v.size);
                    op.status.downloaded_bytes = 0;
                    op.status.phase = if checked.version.is_some() {
                        "available"
                    } else {
                        "up_to_date"
                    }
                    .into();
                    op.candidate = checked.candidate;
                }
                let has_candidate = service.inner.lock().unwrap().candidate.is_some();
                if has_candidate {
                    restore_download(&app);
                }
                let auto = state.settings.lock().unwrap().updates.auto_download;
                if auto && has_candidate {
                    begin_download(&app, true)?;
                }
                Ok(())
            }))
            .unwrap_or_else(|_| Err("update_worker_failed".into()));
        if let Err(error) = result {
            finish_error(&app, error);
        }
    });
    Ok(())
}

fn begin_download(app: &tauri::AppHandle, automatic: bool) -> Result<(), String> {
    let service = app.state::<UpdateService>();
    let (candidate, cache) = {
        let mut op = service.inner.lock().unwrap();
        if automatic && op.status.phase != "available" {
            return Ok(());
        }
        if busy(&op.status.phase) {
            return Err("update_operation_running".into());
        }
        let candidate = op.candidate.clone().ok_or("no_update_available")?;
        validate_candidate(
            &candidate,
            op.identity
                .as_ref()
                .ok_or("update_package_identity_unknown")?,
        )?;
        service.cancel.store(false, Ordering::SeqCst);
        op.status.phase = "downloading".into();
        op.status.error = None;
        op.status.downloaded_bytes = 0;
        (candidate, op.cache.clone())
    };
    let app = app.clone();
    std::thread::spawn(move || {
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), String> {
                std::fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
                update_package::require_space(&cache, candidate.size.saturating_mul(4))?;
                let service = app.state::<UpdateService>();
                let destination = cache.join(&candidate.name);
                let partial = cache.join(format!("{}.part", candidate.name));
                let download = (|| {
                    let mut response = http_get(&candidate.url, false)?;
                    let mut input = response.body_mut().as_reader();
                    let mut output = update_package::create_file(&partial)?;
                    stream_download(
                        &mut input,
                        &mut output,
                        &candidate,
                        || cancelled(&service),
                        |n| {
                            service.inner.lock().unwrap().status.downloaded_bytes = n;
                        },
                    )?;
                    output.sync_all().map_err(|e| e.to_string())?;
                    service.inner.lock().unwrap().status.phase = "verifying".into();
                    update_package::verify_file(&partial, candidate.size, &candidate.sha256)?;
                    cancelled(&service)?;
                    update_package::replace_cache_file(&partial, &destination)?;
                    let identity = service
                        .inner
                        .lock()
                        .unwrap()
                        .identity
                        .clone()
                        .ok_or("update_package_identity_unknown")?;
                    update_package::validate_download(&identity, &candidate, &cache, || {
                        cancelled(&service)
                    })?;
                    update_package::write_json(&cache.join("download.json"), &candidate)?;
                    Ok::<_, String>(())
                })();
                if download.is_err() {
                    let _ = std::fs::remove_file(&partial);
                }
                download?;
                service.inner.lock().unwrap().status.phase = "ready".into();
                Ok(())
            }))
            .unwrap_or_else(|_| Err("update_worker_failed".into()));
        if let Err(error) = result {
            finish_error(&app, error);
        }
    });
    Ok(())
}

fn stream_download(
    input: &mut impl Read,
    output: &mut impl Write,
    candidate: &Candidate,
    mut cancel: impl FnMut() -> Result<(), String>,
    mut progress: impl FnMut(u64),
) -> Result<(), String> {
    let mut buffer = [0u8; 64 * 1024];
    let mut total = 0;
    loop {
        cancel()?;
        let n = input
            .read(&mut buffer)
            .map_err(|_| "update_download_interrupted")?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > candidate.size || total > MAX_DOWNLOAD {
            return Err("update_download_size_mismatch".into());
        }
        output
            .write_all(&buffer[..n])
            .map_err(|_| "update_download_write_failed")?;
        progress(total);
    }
    if total != candidate.size {
        return Err("update_download_truncated".into());
    }
    cancel()
}

#[tauri::command]
pub fn update_status(service: tauri::State<'_, UpdateService>) -> UpdateStatus {
    service.inner.lock().unwrap().status.clone()
}

#[tauri::command]
pub fn check_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    begin_check(&app, Arc::clone(&state))
}

#[tauri::command]
pub fn download_update(app: tauri::AppHandle) -> Result<(), String> {
    begin_download(&app, false)
}

#[tauri::command]
pub fn cancel_update(service: tauri::State<'_, UpdateService>) -> Result<(), String> {
    let phase = service.inner.lock().unwrap().status.phase.clone();
    if phase == "installing" {
        return Err("update_already_installing".into());
    }
    service.cancel.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub async fn install_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let state = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || {
        let service = app.state::<UpdateService>();
        let _writer = state
            .storage
            .try_lock()
            .map_err(|_| "application_busy_retry_update".to_string())?;
        if state.refresh.lock().unwrap().running
            || state.clear_job_running.load(Ordering::SeqCst)
            || !state.cleanup_control.begin()
        {
            return Err("application_busy_retry_update".into());
        }
        struct Maintenance<'a>(&'a llm_usage_core::cancellation::OperationControl);
        impl Drop for Maintenance<'_> {
            fn drop(&mut self) {
                self.0.finish();
            }
        }
        let _maintenance = Maintenance(&state.cleanup_control);
        let _pause = crate::power::PauseIntent::new(&state.automatic_pause_requests);
        let (candidate, identity, cache) = {
            let mut op = service
                .inner
                .try_lock()
                .map_err(|_| "update_operation_running".to_string())?;
            if op.status.phase != "ready" {
                return Err("update_not_ready".into());
            }
            op.status.phase = "verifying".into();
            service.cancel.store(false, Ordering::SeqCst);
            (
                op.candidate.clone().ok_or("no_update_available")?,
                op.identity
                    .clone()
                    .ok_or("update_package_identity_unknown")?,
                op.cache.clone(),
            )
        };
        match update_package::prepare_and_launch(
            &identity,
            &candidate,
            &cache,
            &state.db_path,
            || cancelled(&service),
        ) {
            Ok(()) => {
                service.inner.lock().unwrap().status.phase = "installing".into();
                app.exit(0);
                Ok(())
            }
            Err(error) => {
                finish_error(&app, error.clone());
                Err(error)
            }
        }
    })
    .await
    .map_err(|_| "update_install_worker_failed".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed_fixture() -> (serde_json::Value, i64) {
        let generated = "2026-10-09T00:00:00.000Z";
        let now = generated
            .parse::<jiff::Timestamp>()
            .unwrap()
            .as_millisecond();
        (
            serde_json::json!({"schema":1,"repository":"owent/llm-usage","generated_at":generated,"release":{"tag_name":"v0.4.0","draft":false,"prerelease":false,"assets":[]}}),
            now,
        )
    }

    #[test]
    fn failed_github_requests_try_only_one_documentation_snapshot() {
        let (feed, now) = feed_fixture();
        for error in [
            "update_http_403",
            "update_http_429",
            "update_http_500",
            "update_network_request_failed",
            "update_metadata_too_large_or_unreadable",
        ] {
            let mut requests = Vec::new();
            let bytes = fetch_release_with(
                |url| {
                    requests.push(url.to_string());
                    if url == RELEASE_API {
                        Err(error.into())
                    } else {
                        Ok(serde_json::to_vec(&feed).unwrap())
                    }
                },
                now,
                || Ok(()),
            )
            .unwrap();
            assert_eq!(requests, vec![RELEASE_API, DOCUMENTATION_FEED]);
            let checked = check_release(&bytes, "0.3.0", None).unwrap();
            assert_eq!(checked.version.as_deref(), Some("0.4.0"));
            assert!(checked.candidate.is_none());
        }
    }

    #[test]
    fn successful_api_metadata_never_uses_fallback_to_bypass_asset_errors() {
        let (feed, now) = feed_fixture();
        let mut requests = Vec::new();
        let bytes = fetch_release_with(
            |url| {
                requests.push(url.to_string());
                Ok(serde_json::to_vec(&feed["release"]).unwrap())
            },
            now,
            || Ok(()),
        )
        .unwrap();
        assert_eq!(requests, vec![RELEASE_API]);
        assert!(check_release(
            &bytes,
            "0.3.0",
            Some(&PackageIdentity::test_portable("windows", "x64"))
        )
        .is_err());
        assert!(check_release(&bytes, "0.4.0", None)
            .unwrap()
            .version
            .is_none());
    }

    #[test]
    fn documentation_snapshot_schema_freshness_and_stability_are_required() {
        let (original, now) = feed_fixture();
        let bytes = serde_json::to_vec(&original).unwrap();
        assert!(documentation_release(&bytes, now + 7 * DAY).is_ok());
        assert!(documentation_release(&bytes, now + 7 * DAY + 1).is_err());
        assert!(documentation_release(&bytes, now - 300_001).is_err());
        for (key, value) in [
            ("schema", serde_json::json!(2)),
            ("repository", serde_json::json!("other/repository")),
            ("generated_at", serde_json::json!("invalid")),
            ("extra", serde_json::json!(true)),
        ] {
            let mut feed = original.clone();
            feed[key] = value;
            assert!(documentation_release(&serde_json::to_vec(&feed).unwrap(), now).is_err());
        }
        for flag in ["draft", "prerelease"] {
            let mut feed = original.clone();
            feed["release"][flag] = serde_json::json!(true);
            assert!(documentation_release(&serde_json::to_vec(&feed).unwrap(), now).is_err());
        }
    }

    #[test]
    fn cancelled_or_failed_fallback_retains_the_correct_failure() {
        let (_, now) = feed_fixture();
        let mut requests = Vec::new();
        let mut checks = 0;
        let result = fetch_release_with(
            |url| {
                requests.push(url.to_string());
                Err("update_http_429".into())
            },
            now,
            || {
                checks += 1;
                if checks == 2 {
                    Err("update_cancelled".into())
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(result.unwrap_err(), "update_cancelled");
        assert_eq!(requests, vec![RELEASE_API]);
        let result = fetch_release_with(
            |url| {
                Err(if url == RELEASE_API {
                    "update_http_403"
                } else {
                    "update_http_404"
                }
                .into())
            },
            now,
            || Ok(()),
        );
        let error = result.unwrap_err();
        assert!(error.contains("github=update_http_403"));
        assert!(error.contains("documentation=update_http_404"));
        assert!(allowed_url(DOCUMENTATION_FEED, true));
        assert!(!allowed_url(DOCUMENTATION_FEED, false));
        assert!(!allowed_url(
            "http://llm-usage.atframe.work/updates/latest.json",
            true
        ));
        assert!(!allowed_url(
            "https://llm-usage.atframe.work/updates/latest.json?source=other",
            true
        ));
    }

    #[test]
    fn published_documentation_fixture_uses_the_same_package_digest_checks() {
        let raw = include_bytes!("../../../docs/site/public/updates/latest.json");
        let feed: DocumentationFeed = serde_json::from_slice(raw).unwrap();
        let now = feed
            .generated_at
            .parse::<jiff::Timestamp>()
            .unwrap()
            .as_millisecond();
        let bytes = documentation_release(raw, now).unwrap();
        for platform in ["windows", "linux", "macos"] {
            for arch in ["x64", "arm64"] {
                let identity = PackageIdentity::test_portable(platform, arch);
                let checked = check_release(&bytes, "0.0.0", Some(&identity)).unwrap();
                let candidate = checked.candidate.unwrap();
                assert_eq!(
                    candidate.name,
                    identity.asset_name(&candidate.version).unwrap()
                );
                assert_eq!(candidate.sha256.len(), 64);
                let mut invalid: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                for asset in invalid["assets"].as_array_mut().unwrap() {
                    asset["digest"] = serde_json::Value::Null;
                }
                assert!(check_release(
                    &serde_json::to_vec(&invalid).unwrap(),
                    "0.0.0",
                    Some(&identity)
                )
                .is_err());
            }
        }
    }

    #[test]
    fn version_checks_do_not_require_or_invent_an_installation_identity() {
        let release =
            serde_json::json!({"tag_name":"v0.4.0","draft":false,"prerelease":false,"assets":[]});
        let bytes = serde_json::to_vec(&release).unwrap();
        let found = check_release(&bytes, "0.3.0", None).unwrap();
        assert_eq!(found.version.as_deref(), Some("0.4.0"));
        assert!(found.candidate.is_none());
        for current in ["0.4.0", "0.5.0"] {
            let checked = check_release(&bytes, current, None).unwrap();
            assert!(checked.version.is_none() && checked.candidate.is_none());
        }
        assert!(check_release(
            &bytes,
            "0.3.0",
            Some(&PackageIdentity::test_portable("windows", "x64"))
        )
        .is_err());
    }

    #[test]
    fn informational_checks_still_reject_invalid_or_unstable_release_metadata() {
        let original =
            serde_json::json!({"tag_name":"v0.4.0","draft":false,"prerelease":false,"assets":[]});
        for (key, value) in [
            ("draft", serde_json::json!(true)),
            ("prerelease", serde_json::json!(true)),
            ("tag_name", serde_json::json!("v0.4.0-preview")),
            ("tag_name", serde_json::json!("v0.4.0+build")),
            ("tag_name", serde_json::json!("0.4.0")),
        ] {
            let mut release = original.clone();
            release[key] = value;
            assert!(check_release(&serde_json::to_vec(&release).unwrap(), "0.3.0", None).is_err());
        }
    }

    #[test]
    fn persisted_deadlines_manual_launch_failure_and_clock_rollback() {
        let clock = CheckClock {
            success_ms: Some(DAY),
            attempt_ms: Some(DAY),
        };
        assert!(!due(Schedule::Manual, &clock, 20 * DAY, true));
        assert!(due(Schedule::Launch, &clock, DAY, true));
        assert!(!due(Schedule::Launch, &clock, 20 * DAY, false));
        assert!(!due(Schedule::Daily, &clock, 2 * DAY - 1, true));
        assert!(due(Schedule::Daily, &clock, 2 * DAY, false));
        assert!(!due(Schedule::Weekly, &clock, 8 * DAY - 1, true));
        assert!(due(Schedule::Weekly, &clock, 8 * DAY, false));
        assert!(due(Schedule::Daily, &clock, 0, true));
        let failed = CheckClock {
            success_ms: None,
            attempt_ms: Some(0),
        };
        assert!(!due(Schedule::Daily, &failed, 3_599_999, true));
        assert!(due(Schedule::Daily, &failed, 3_600_000, false));
        let decoded: CheckClock =
            serde_json::from_str(&serde_json::to_string(&clock).unwrap()).unwrap();
        assert!(!due(Schedule::Weekly, &decoded, 2 * DAY, true));
        let defaults: UpdateSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(defaults.schedule, Schedule::Daily);
        assert!(!defaults.auto_download);
        let mut old = serde_json::to_value(crate::app_state::AppSettings::default()).unwrap();
        old.as_object_mut().unwrap().remove("updates");
        let decoded: crate::app_state::AppSettings = serde_json::from_value(old).unwrap();
        assert_eq!(decoded.updates.schedule, Schedule::Daily);
        assert!(!decoded.updates.auto_download);
        assert!(due(
            Schedule::Weekly,
            &CheckClock {
                success_ms: Some(i64::MIN),
                attempt_ms: Some(i64::MIN)
            },
            i64::MAX,
            false
        ));
    }

    #[test]
    fn release_selection_never_switches_package_architecture_or_downgrades() {
        for platform in ["windows", "linux", "macos"] {
            for arch in ["x64", "arm64"] {
                let identity = PackageIdentity::test_portable(platform, arch);
                let name = identity.asset_name("0.10.0").unwrap();
                let candidate = Candidate {
                    version: "0.10.0".into(),
                    name: name.clone(),
                    url: format!(
                        "https://github.com/owent/llm-usage/releases/download/v0.10.0/{name}"
                    ),
                    size: 100,
                    sha256: "a".repeat(64),
                };
                let asset = serde_json::json!({"name":name,"state":"uploaded","size":100,"digest":format!("sha256:{}",candidate.sha256),"browser_download_url":candidate.url});
                let mut release = serde_json::json!({"tag_name":"v0.10.0","draft":false,"prerelease":false,"assets":[asset.clone()]});
                assert_eq!(
                    select_release(&serde_json::to_vec(&release).unwrap(), "0.9.0", &identity)
                        .unwrap(),
                    Some(candidate)
                );
                assert!(select_release(
                    &serde_json::to_vec(&release).unwrap(),
                    "0.10.0",
                    &identity
                )
                .unwrap()
                .is_none());
                assert!(
                    select_release(&serde_json::to_vec(&release).unwrap(), "1.0.0", &identity)
                        .unwrap()
                        .is_none()
                );
                release["assets"].as_array_mut().unwrap().push(asset);
                assert!(
                    select_release(&serde_json::to_vec(&release).unwrap(), "0.9.0", &identity)
                        .is_err()
                );
                release["assets"] = serde_json::json!([]);
                assert!(
                    select_release(&serde_json::to_vec(&release).unwrap(), "0.9.0", &identity)
                        .is_err()
                );
            }
        }
    }

    #[test]
    fn invalid_assets_redirects_and_incomplete_streams_fail_closed() {
        let identity = PackageIdentity::test_portable("windows", "x64");
        let mut candidate = Candidate {
            version: "1.0.0".into(),
            name: identity.asset_name("1.0.0").unwrap(),
            size: 3,
            sha256: "a".repeat(64),
            url: String::new(),
        };
        assert!(validate_candidate(&candidate, &identity).is_err());
        candidate.url = format!(
            "https://github.com/owent/llm-usage/releases/download/v1.0.0/{}",
            candidate.name
        );
        assert!(validate_candidate(&candidate, &identity).is_ok());
        candidate.sha256 = "a".repeat(63);
        assert!(validate_candidate(&candidate, &identity).is_err());
        assert!(!allowed_url("https://github.com.evil.test/package", false));
        assert!(!allowed_url(
            "http://release-assets.githubusercontent.com/a",
            false
        ));
        assert!(!allowed_url(
            "https://api.github.com/repos/other/releases/latest",
            true
        ));
        let mut bytes = Vec::new();
        assert!(
            stream_download(&mut &b"ab"[..], &mut bytes, &candidate, || Ok(()), |_| {}).is_err()
        );
        assert!(
            stream_download(&mut &b"abcd"[..], &mut bytes, &candidate, || Ok(()), |_| {}).is_err()
        );
        assert!(stream_download(
            &mut &b"abc"[..],
            &mut bytes,
            &candidate,
            || Err("update_cancelled".into()),
            |_| {}
        )
        .is_err());
        assert!(
            stream_download(&mut &b"abc"[..], &mut bytes, &candidate, || Ok(()), |_| {}).is_ok()
        );
    }

    #[test]
    fn installer_selection_requires_its_exact_asset_digest_and_stable_version() {
        let mut identity = PackageIdentity::test_portable("windows", "x64");
        identity.kind = "installer".into();
        let name = identity.asset_name("0.3.0").unwrap();
        assert_eq!(name, "LLMUsage_0.3.0_x64-setup.exe");
        let asset = serde_json::json!({"name":name,"state":"uploaded","size":100,"digest":format!("sha256:{}","a".repeat(64)),"browser_download_url":format!("https://github.com/owent/llm-usage/releases/download/v0.3.0/{name}")});
        let release = serde_json::json!({"tag_name":"v0.3.0","draft":false,"prerelease":false,"assets":[asset]});
        assert_eq!(
            select_release(&serde_json::to_vec(&release).unwrap(), "0.2.2", &identity)
                .unwrap()
                .unwrap()
                .name,
            name
        );
        for (field, value) in [
            ("digest", serde_json::Value::Null),
            ("state", serde_json::json!("new")),
            (
                "name",
                serde_json::json!("LLMUsage-0.3.0-windows-x64-portable.tar.zst"),
            ),
        ] {
            let mut invalid = release.clone();
            invalid["assets"][0][field] = value;
            assert!(
                select_release(&serde_json::to_vec(&invalid).unwrap(), "0.2.2", &identity).is_err()
            );
        }
        for tag in ["v0.3.0-preview", "v0.3.0+build", "0.3.0", "v00.3.0"] {
            let mut invalid = release.clone();
            invalid["tag_name"] = serde_json::json!(tag);
            assert!(
                select_release(&serde_json::to_vec(&invalid).unwrap(), "0.2.2", &identity).is_err()
            );
        }
        identity.arch = "arm64".into();
        assert!(identity.asset_name("0.3.0").is_err());
        identity.platform = "linux".into();
        assert!(identity.asset_name("0.3.0").is_err());
    }
}
