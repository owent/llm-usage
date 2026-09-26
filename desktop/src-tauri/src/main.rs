//! llm-usage 桌面客户端入口（M6）。
//! GUI 模式：单窗口 + 后台间隔调度；headless 模式（--headless）：无 WebView，
//! 执行一次全源采集后退出（供系统定时任务使用，V24 的无窗口提取路径）。
//! M0 试验命令（sqlite_probe/read_sample_file）已被真实功能取代；
//! 对应回归语义保留在 core 测试与 M0 验证记录中。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_state;
mod commands;
mod scanner;

use app_state::AppState;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown-host".to_string())
}

fn db_path() -> std::path::PathBuf {
    // 无 Tauri 句柄阶段（headless/启动前）：%APPDATA%/llm-usage-desktop/ 或
    // ~/.local/share/llm-usage-desktop/ 下的 llm-usage.sqlite；
    // 与 architecture.md「应用数据库放系统应用数据目录」一致。
    // 目录名避开 Roaming 下已存在的同名占位文件 llm-usage（第三方遗留，不改动它）。
    if let Ok(appdata) = std::env::var("APPDATA") {
        return std::path::PathBuf::from(appdata)
            .join("llm-usage-desktop")
            .join("llm-usage.sqlite");
    }
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        return std::path::PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("llm-usage-desktop")
            .join("llm-usage.sqlite");
    }
    std::path::PathBuf::from("llm-usage.sqlite")
}

fn main() {
    let headless = std::env::args().any(|a| a == "--headless" || a == "--scan-once");
    let state = match AppState::init(db_path(), &hostname()) {
        Ok(s) => Arc::new(s),
        Err(e) if e == "__EXIT_SCHEMA_MISMATCH__" => {
            println!("user declined database rebuild; exiting");
            return;
        }
        Err(e) => panic!("init app state: {e}"),
    };

    if headless {
        // 无 WebView headless 提取：一次采集后退出；不启动窗口/调度线程。
        let started = scanner::run_refresh(&state, llm_usage_core::jobs::TriggerKind::Interval);
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
        return;
    }

    let stop = Arc::new(AtomicBool::new(false));
    let scheduler_state = Arc::clone(&state);
    scanner::spawn_scheduler(scheduler_state, stop);

    tauri::Builder::default()
        .manage(Arc::clone(&state))
        .invoke_handler(tauri::generate_handler![
            commands::summary,
            commands::heatmap,
            commands::list_sources,
            commands::set_source_enabled,
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
            commands::storage_stats,
            commands::manual_cleanup,
            commands::pick_open_path,
            commands::clear_all_data,
            commands::event_details,
            commands::export_filter_options,
            commands::chart_series,
            commands::diagnostic_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
