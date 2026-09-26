//! 应用状态：单写者存储句柄、本机来源主机身份、设置与刷新作业状态。
//! M6 合同：GUI/headless 共享配置与采集队列；查询与写入共用一个 Storage，
//! 通过 Mutex 串行化（单写者合同），后台扫描期间查询等待有界。

use llm_usage_core::adapters::framework::SourceRunReport;
use llm_usage_core::storage::Storage;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

/// 用户可见设置（settings 表持久化；语言初值 zh-CN，多语言实施随 F3 合同）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub timezone: String,
    /// 0=周一 … 6=周日。
    pub week_start: u8,
    /// 有限保留天数；None = 无限。
    pub retention_days: Option<u32>,
    /// 前台自动刷新间隔（秒）；0 = 关闭自动提取。
    pub refresh_interval_secs: u64,
    pub language: String,
    /// 手工添加的数据源根目录。
    pub manual_roots: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            timezone: system_timezone_name(),
            week_start: 1,
            retention_days: None,
            refresh_interval_secs: 60,
            language: "zh-CN".to_string(),
            manual_roots: Vec::new(),
        }
    }
}

/// 系统 IANA 时区名；取不到时退回 UTC（记录在设置中可见可改）。
fn system_timezone_name() -> String {
    std::env::var("TZ")
        .ok()
        .or_else(|| {
            // Windows 无 TZ 时用 jiff 系统时区探测（TimeZone::system 直接返回实例）。
            jiff::tz::TimeZone::system().iana_name().map(str::to_string)
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "UTC".to_string())
}

/// 刷新作业的可观察状态（UI 轮询；进度按实例粒度）。
#[derive(Debug, Clone, Default, Serialize)]
pub struct RefreshState {
    pub running: bool,
    pub started_ms: i64,
    pub last_finished_ms: i64,
    pub trigger: String,
    /// 逐实例摘要：instance → (状态, 事件数, 诊断数)。
    pub instances: Vec<RefreshInstanceSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RefreshInstanceSummary {
    pub instance_id: String,
    pub agent: String,
    pub status: String,
    pub error: Option<String>,
    pub added: i64,
    pub updated: i64,
    pub files: usize,
    pub events: u64,
    pub diagnostics: u64,
}

pub struct AppState {
    pub storage: Mutex<Storage>,
    pub host_id: Mutex<String>,
    pub settings: Mutex<AppSettings>,
    pub refresh: Mutex<RefreshState>,
    pub db_path: PathBuf,
}

impl AppState {
    /// 打开/迁移应用数据库并初始化主机身份与设置。
    pub fn init(db_path: PathBuf, hostname: &str) -> Result<Self, String> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let storage = Storage::open(&db_path).map_err(|e| e.to_string())?;
        let now = now_ms();
        let host_id = storage
            .ensure_local_host(hostname, now)
            .map_err(|e| e.to_string())?;
        let settings = load_settings(&storage);
        // 时区分区修复：老版本扫描以 UTC 写日分区而用户统计时区不同 ⇒
        // 在用户时区下重算事件覆盖范围（推导非猜测；2026-09-26 缺陷修复）。
        repair_tz_partitions(&storage, &settings.timezone);
        Ok(AppState {
            storage: Mutex::new(storage),
            host_id: Mutex::new(host_id),
            settings: Mutex::new(settings),
            refresh: Mutex::new(RefreshState::default()),
            db_path,
        })
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

const SETTINGS_KEY: &str = "app_settings";

/// 若日汇总只有其他时区分区而缺用户时区分区，按事件范围在用户时区重算。
/// 封存日在目标时区不存在封存行，重算安全；失败不阻塞启动（下次扫描再修）。
pub fn repair_tz_partitions(storage: &Storage, timezone: &str) {
    let has_user_tz: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM daily_usage WHERE tz_version = ?1",
            [timezone],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if has_user_tz > 0 {
        return;
    }
    let range: Option<(i64, i64)> = storage
        .conn()
        .query_row(
            "SELECT MIN(occurred_at_ms), MAX(occurred_at_ms) FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    if let Some((min_ms, max_ms)) = range {
        if let Err(e) = llm_usage_core::ingest::recompute_days_in_tz(
            storage,
            timezone,
            min_ms,
            max_ms,
            now_ms(),
        ) {
            eprintln!("tz partition repair failed (will retry on next scan): {e}");
        }
    }
}

/// 查询连接：优先常驻只读连接（懒建，打开失败带短重试——观察到的
/// 间歇性 disk I/O error 多为打开瞬间冲突）；仍失败时回退写连接互斥锁，
/// 保证 UI 查询永不因只读路径失败而硬错（宁可短暂排队）。
/// 查询连接：只读连接（带短重试——观察到的间歇性 disk I/O error 发生在
/// 打开瞬间）；仍失败时回退写连接互斥锁，UI 查询宁可短暂排队也不硬错。
pub enum ReadConn<'a> {
    Ro(Storage),
    Writer(std::sync::MutexGuard<'a, Storage>),
}

impl std::ops::Deref for ReadConn<'_> {
    type Target = Storage;
    fn deref(&self) -> &Storage {
        match self {
            ReadConn::Ro(s) => s,
            ReadConn::Writer(guard) => guard,
        }
    }
}

pub fn read_conn(state: &AppState) -> ReadConn<'_> {
    for _ in 0..3 {
        if let Ok(s) = Storage::open_readonly(&state.db_path) {
            return ReadConn::Ro(s);
        }
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
    eprintln!("read path: readonly unavailable; falling back to writer connection");
    ReadConn::Writer(state.storage.lock().unwrap())
}

pub fn load_settings(storage: &Storage) -> AppSettings {
    let raw: Option<String> = storage
        .conn()
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [SETTINGS_KEY],
            |r| r.get(0),
        )
        .ok();
    raw.and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default()
}

pub fn save_settings(storage: &Storage, settings: &AppSettings) -> Result<(), String> {
    let json = serde_json::to_string(settings).map_err(|e| e.to_string())?;
    storage
        .conn()
        .execute(
            "INSERT INTO settings (key, value, schema_version, updated_at_ms)
             VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at_ms = excluded.updated_at_ms",
            rusqlite::params![
                SETTINGS_KEY,
                json,
                now_ms()
            ],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 运行报告 → UI 摘要。
pub fn summarize_reports(reports: &[SourceRunReport]) -> Vec<RefreshInstanceSummary> {
    reports
        .iter()
        .map(|r| RefreshInstanceSummary {
            instance_id: r.instance_id.clone(),
            agent: r
                .instance_id
                .split('@')
                .next()
                .unwrap_or("unknown")
                .to_string(),
            status: r.finish.as_str().to_string(),
            error: r.error.clone(),
            added: r.outcome.as_ref().map(|o| o.added).unwrap_or(0),
            updated: r.outcome.as_ref().map(|o| o.updated).unwrap_or(0),
            files: r.files.len(),
            events: r.files.iter().map(|f| f.events).sum(),
            diagnostics: r.files.iter().map(|f| f.diagnostics).sum(),
        })
        .collect()
}
