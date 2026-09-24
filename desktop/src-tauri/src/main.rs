#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use rusqlite::Connection;

#[tauri::command]
fn sqlite_probe() -> Result<serde_json::Value, String> {
    let dir = std::env::temp_dir().join("llm-usage-m0-probe");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let db_path = dir.join("probe.db");
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
    }))
}

#[tauri::command]
fn read_sample_file() -> Result<serde_json::Value, String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../sample-data.txt");
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(serde_json::json!({
        "path": "sample-data.txt",
        "len": bytes.len(),
    }))
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![sqlite_probe, read_sample_file])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
