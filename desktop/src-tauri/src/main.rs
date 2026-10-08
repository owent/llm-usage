//! Desktop entry: single GUI window/shared scheduler. --headless uses saved
//! system-task intent/due rules; --scan-once manually collects every enabled source.
//! Neither CLI mode creates a WebView or local telemetry receiver.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

mod app_state;
mod commands;
mod db_backup;
mod file_watch;
#[cfg(windows)]
mod installation;
mod otel_receiver;
mod power;
mod price_refresh;
mod process_guard;
mod receiver_auth;
mod scanner;
mod source_intervals;
mod system_tasks;
mod telemetry_setup;
mod tray;

use app_state::AppState;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown-host".to_string())
}

fn data_dir_arg(
    args: impl IntoIterator<Item = String>,
) -> Result<Option<std::path::PathBuf>, String> {
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--data-dir" {
            let dir = args
                .next()
                .filter(|v| !v.is_empty())
                .ok_or("--data-dir requires an absolute directory")?;
            let path = std::path::PathBuf::from(dir);
            if !path.is_absolute() {
                return Err("--data-dir requires an absolute directory".into());
            }
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn db_path() -> Result<std::path::PathBuf, String> {
    if let Some(dir) = data_dir_arg(std::env::args())? {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        return Ok(dir
            .canonicalize()
            .map_err(|e| e.to_string())?
            .join("llm-usage.sqlite"));
    }
    // Before a Tauri handle, headless/prestartup DB is under APPDATA/llm-usage-desktop/ or
    // ~/.local/share/llm-usage-desktop/, named llm-usage.sqlite;
    // matching architecture.md system application-data storage rules.
    // Avoid existing third-party Roaming placeholder file llm-usage; preserve it.
    if let Ok(appdata) = std::env::var("APPDATA") {
        return Ok(std::path::PathBuf::from(appdata)
            .join("llm-usage-desktop")
            .join("llm-usage.sqlite"));
    }
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        return Ok(std::path::PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("llm-usage-desktop")
            .join("llm-usage.sqlite"));
    }
    Ok(std::path::PathBuf::from("llm-usage.sqlite"))
}

fn main() {
    if std::env::args().any(|arg| arg == "--uninstall-cleanup") {
        #[cfg(windows)]
        if let Err(error) = installation::cleanup() {
            eprintln!("uninstall cleanup failed: {error}");
            std::process::exit(1);
        }
        // This branch precedes db_path, migrations, collection and GUI startup.
        return;
    }
    let headless = std::env::args().any(|a| a == "--headless" || a == "--scan-once");
    let path = db_path().unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    let _owner = match process_guard::acquire(&path) {
        Ok(Some(owner)) => owner,
        Ok(None) => {
            if headless {
                let request = if std::env::args().any(|arg| arg == "--scan-once") {
                    process_guard::request_refresh(&path, scanner::now_ms())
                        .map(|_| true)
                        .map_err(|e| e.to_string())
                } else {
                    system_tasks::request_if_enabled(&path, scanner::now_ms())
                };
                match request {
                    Ok(true) => println!("refresh queued for the running application"),
                    Ok(false) => println!("background refresh skipped: task disabled"),
                    Err(error) => {
                        eprintln!("could not queue refresh: {error}");
                        std::process::exit(1);
                    }
                }
            } else {
                rfd::MessageDialog::new()
                    .set_title("LLM Usage")
                    .set_description(
                        "应用已在运行，请使用已打开的窗口。\nLLM Usage is already running.",
                    )
                    .show();
            }
            return;
        }
        Err(error) => {
            eprintln!("could not acquire application lock: {error}");
            std::process::exit(1);
        }
    };
    let state = match AppState::init(path, &hostname(), !headless) {
        Ok(s) => Arc::new(s),
        Err(e) if e == "__EXIT_SCHEMA_MISMATCH__" => {
            println!("user declined database rebuild; exiting");
            return;
        }
        Err(e) => {
            eprintln!("could not initialize application: {e}");
            std::process::exit(1);
        }
    };

    if headless {
        // Headless without WebView: collect once then exit, no window/scheduler thread.
        let started = if std::env::args().any(|arg| arg == "--scan-once") {
            scanner::run_refresh(&state, llm_usage_core::jobs::TriggerKind::Manual)
        } else {
            scanner::run_background_refresh(&state).unwrap_or_else(|error| {
                eprintln!("background scan failed: {error}");
                std::process::exit(1);
            })
        };
        let refresh = state.refresh.lock().unwrap();
        for instance in &refresh.instances {
            if let Some(error) = &instance.error {
                println!(
                    "instance {} status={} error={error}",
                    instance.instance_id, instance.status
                );
            }
        }
        let events: u64 = refresh.instances.iter().map(|i| i.events).sum();
        let diags: u64 = refresh.instances.iter().map(|i| i.diagnostics).sum();
        println!(
            "headless scan executed={} instances={} events={events} diagnostics={diags}",
            started,
            refresh.instances.len()
        );
        if refresh.instances.iter().any(|i| i.status == "failed") {
            std::process::exit(1);
        }
        return;
    }

    let stop = Arc::new(AtomicBool::new(false));
    let scheduler_state = Arc::clone(&state);
    scanner::spawn_scheduler(scheduler_state, stop);

    // M5 local OTLP receiver: opt-in, 127.0.0.1 only, thread ends with the process;
    // setting changes take effect after restart.
    {
        let (enabled, port) = {
            let settings = state.settings.lock().unwrap();
            (settings.otel_receiver_enabled, settings.otel_receiver_port)
        };
        if enabled {
            let out_dir = state
                .db_path
                .parent()
                .map(|p| p.join("otel"))
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            match otel_receiver::ensure_started(port, out_dir.clone()) {
                Ok(()) => println!(
                    "otel receiver listening on 127.0.0.1:{port} (spans -> {})",
                    out_dir.join("spans.jsonl").display()
                ),
                Err(e) => eprintln!("otel receiver failed to start: {e}"),
            }
        }
    }

    tauri::Builder::default()
        .setup(|app| {
            let state = app.state::<Arc<AppState>>();
            if let Err(error) = crate::tray::apply(app.handle(), &state.settings.lock().unwrap()) {
                eprintln!("tray setup failed: {error}");
            }
            // Size from primary-display resolution, 60-85%, maximum 1600x1000.
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(Some(monitor)) = window.primary_monitor() {
                    let size = monitor.size();
                    let scale = monitor.scale_factor();
                    let phys_w = size.width as f64 / scale;
                    let phys_h = size.height as f64 / scale;
                    let w = (phys_w * 0.72).clamp(900.0, 1600.0) as u32;
                    let h = (phys_h * 0.78).clamp(600.0, 1000.0) as u32;
                    let _ = window.set_size(tauri::PhysicalSize::new(
                        (w as f64 * scale) as u32,
                        (h as f64 * scale) as u32,
                    ));
                }
            }
            Ok(())
        })
        .manage(Arc::clone(&state))
        .on_window_event(crate::tray::close_requested)
        .invoke_handler(tauri::generate_handler![
            commands::summary,
            commands::heatmap,
            commands::list_sources,
            commands::set_source_enabled,
            commands::set_source_schedule,
            commands::refresh_sources,
            commands::refresh_status,
            commands::get_settings,
            commands::set_settings,
            commands::app_info,
            commands::export_data,
            commands::reload_settings,
            commands::pick_save_path,
            commands::system_task_status,
            commands::set_auto_start,
            commands::set_refresh_task,
            commands::list_users,
            commands::create_user,
            commands::set_current_user,
            commands::assign_source_user,
            commands::import_exchange,
            commands::preview_exchange,
            commands::budget_status,
            commands::storage_stats,
            commands::manual_cleanup,
            commands::cancel_cleanup,
            commands::pick_open_path,
            commands::clear_all_data,
            commands::clear_all_preview,
            commands::event_details,
            commands::export_filter_options,
            commands::chart_series,
            commands::diagnostic_logs,
            commands::cost_summary,
            commands::recompute_costs,
            commands::list_price_snapshots,
            commands::import_price_snapshot,
            commands::refresh_prices_online,
            commands::price_refresh_status,
            commands::quota_summary,
            commands::quota_series,
            telemetry_setup::telemetry_check,
            telemetry_setup::telemetry_preview,
            telemetry_setup::telemetry_apply,
            telemetry_setup::telemetry_undo,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
