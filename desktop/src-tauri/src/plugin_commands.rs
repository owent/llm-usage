use crate::app_state::AppState;
use llm_usage_core::plugins::{self, Package, PackageInfo};
use llm_usage_core::CoreError;
use std::sync::{Arc, Mutex};
use tauri::State;

#[derive(Default)]
pub struct PackagePreview(Mutex<Option<Package>>);

fn safe_error(error: CoreError) -> String {
    match error {
        CoreError::Validation(code) => code,
        CoreError::Io(_) => "plugin_io".into(),
        _ => "plugin_storage".into(),
    }
}

#[tauri::command]
pub fn inspect_plugin_package(
    preview: State<'_, PackagePreview>,
) -> Result<Option<serde_json::Value>, String> {
    let mut held = preview.0.lock().unwrap();
    *held = None;
    let Some(path) = rfd::FileDialog::new()
        .add_filter("LLM Usage plugin", &["zst"])
        .pick_file()
    else {
        return Ok(None);
    };
    let package = Package::open(&path, crate::scanner::now_ms()).map_err(safe_error)?;
    let info = serde_json::to_value(package.info()).map_err(|_| "plugin_preview")?;
    *held = Some(package);
    Ok(Some(info))
}

#[tauri::command]
pub fn install_plugin_package(
    state: State<'_, Arc<AppState>>,
    preview: State<'_, PackagePreview>,
    digest: String,
) -> Result<Vec<PackageInfo>, String> {
    let mut held = preview.0.lock().unwrap();
    let package = held
        .as_ref()
        .filter(|package| package.info().digest == digest)
        .ok_or("plugin_preview_expired")?;
    let storage = state.storage.lock().unwrap();
    plugins::install(&storage, package).map_err(safe_error)?;
    *held = None;
    plugins::list(&storage).map_err(safe_error)
}

#[tauri::command]
pub fn list_plugins(state: State<'_, Arc<AppState>>) -> Result<Vec<PackageInfo>, String> {
    plugins::list(&crate::app_state::read_conn(&state)).map_err(safe_error)
}

#[tauri::command]
pub fn set_plugin_enabled(
    state: State<'_, Arc<AppState>>,
    id: String,
    version: String,
    digest: String,
    enabled: bool,
) -> Result<Vec<PackageInfo>, String> {
    let storage = state.storage.lock().unwrap();
    plugins::set_enabled(&storage, &id, &version, &digest, enabled).map_err(safe_error)?;
    plugins::list(&storage).map_err(safe_error)
}

#[tauri::command]
pub fn remove_plugin_package(
    state: State<'_, Arc<AppState>>,
    id: String,
    version: String,
    digest: String,
) -> Result<Vec<PackageInfo>, String> {
    let storage = state.storage.lock().unwrap();
    plugins::remove(&storage, &id, &version, &digest).map_err(safe_error)?;
    plugins::list(&storage).map_err(safe_error)
}
