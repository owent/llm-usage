#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::Manager;

static PROBE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct ProbeDirectory(PathBuf);
impl Drop for ProbeDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tauri::command]
fn sqlite_probe() -> Result<serde_json::Value, String> {
    let dir = std::env::temp_dir().join(format!(
        "llm-usage-m0-probe-{}-{}",
        std::process::id(),
        PROBE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&dir).map_err(|e| e.to_string())?;
    let dir = ProbeDirectory(dir);
    let db_path = dir.0.join("probe.db");
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS probe (id INTEGER PRIMARY KEY, note TEXT NOT NULL)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM probe", [])
        .map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO probe (note) VALUES ('alpha')", [])
        .map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO probe (note) VALUES ('beta')", [])
        .map_err(|e| e.to_string())?;
    let rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM probe", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let sqlite_version: String = conn
        .query_row("SELECT sqlite_version()", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "rows": rows,
        "sqliteVersion": sqlite_version,
        "coreSchemaVersion": llm_usage_core::storage::schema::SCHEMA_VERSION,
    }))
}

#[tauri::command]
fn read_sample_file(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    read_sample_from(&app.path().resource_dir().map_err(|e| e.to_string())?)
}

fn read_sample_from(resource_dir: &Path) -> Result<serde_json::Value, String> {
    let bytes = std::fs::read(resource_dir.join("sample-data.txt"))
        .map_err(|e| format!("sample-data.txt: {e}"))?;
    Ok(serde_json::json!({
        "path": "sample-data.txt",
        "len": bytes.len(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_probes_are_independent_when_concurrent() {
        let probes: Vec<_> = (0..8).map(|_| std::thread::spawn(sqlite_probe)).collect();
        for probe in probes {
            assert_eq!(probe.join().unwrap().unwrap()["rows"], 2);
        }
    }

    #[test]
    fn sample_is_read_from_the_installed_resource_directory() {
        let dir =
            std::env::temp_dir().join(format!("llm-usage-resource-test-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let dir = ProbeDirectory(dir);
        // 不回退到仍存在的开发源码：安装目录缺资源必须失败。
        assert!(read_sample_from(&dir.0).is_err());
        std::fs::write(dir.0.join("sample-data.txt"), b"installed sample").unwrap();
        assert_eq!(read_sample_from(&dir.0).unwrap()["len"], 16);
    }
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![sqlite_probe, read_sample_file])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
