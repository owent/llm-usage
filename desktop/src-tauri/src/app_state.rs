//! 应用状态：单写者存储句柄、本机来源主机身份、设置与刷新作业状态。
//! M6 约定：GUI/headless 共享配置与采集队列；查询与写入共用一个 Storage，
//! 通过 Mutex 串行化（单写者约定），后台扫描期间查询等待有界。

use llm_usage_core::adapters::framework::SourceRunReport;
use llm_usage_core::storage::Storage;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

/// 分级归档保留（天；年 None=终身）。默认：明细 7/小时 3/日 90/周 3 年/
/// 月 10 年/年终身（当前规则，二轮调整为降低聚合消耗）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionTiers {
    pub events_days: u32,
    pub hourly_days: u32,
    pub daily_days: u32,
    pub weekly_days: u32,
    pub monthly_days: u32,
    pub yearly_days: Option<u32>,
}

impl Default for RetentionTiers {
    fn default() -> Self {
        RetentionTiers {
            events_days: 7,
            hourly_days: 3,
            daily_days: 90,
            weekly_days: 1095,
            monthly_days: 3650,
            yearly_days: None,
        }
    }
}

/// 供应商级费用估算偏好：精确渠道优先，未知渠道仅允许无歧义的同型号官方参考。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderPricingDefault {
    pub provider_id: String,
    pub region: String,
    pub channel: String,
    /// 缓存写默认 TTL 档（分钟；5/60 合法，None=未设，写分量不计价）。
    #[serde(default)]
    pub cache_ttl_minutes: Option<u32>,
}

/// 费用估算设置（数据规范：估算与预算默认关闭）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PricingSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub provider_defaults: Vec<ProviderPricingDefault>,
    /// models.dev 在线刷新（默认关闭；开启后仅 HTTPS GET 公开价格目录）。
    #[serde(default)]
    pub online_refresh_enabled: bool,
    /// 原始响应缓存 TTL（天；默认 3，范围 1–365）。新鲜期内不发网络请求。
    #[serde(default = "default_online_cache_ttl_days")]
    pub online_cache_ttl_days: u32,
}

/// 手动 Default：派生 Default 会把 online_cache_ttl_days 设为 u32::default()=0，
/// 与约定「默认 3」矛盾，导致新装/缺 pricing 字段的配置界面显示 0。
impl Default for PricingSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            provider_defaults: Vec::new(),
            online_refresh_enabled: false,
            online_cache_ttl_days: default_online_cache_ttl_days(),
        }
    }
}

fn default_online_cache_ttl_days() -> u32 {
    crate::price_refresh::DEFAULT_TTL_DAYS
}

impl PricingSettings {
    /// 转核心估算选项（provider 键 casefold）。
    pub fn estimate_options(&self) -> llm_usage_core::pricing::EstimateOptions {
        use llm_usage_core::pricing::EstimateOptions;
        let mut options = EstimateOptions::default();
        for d in &self.provider_defaults {
            let provider = d.provider_id.to_lowercase();
            options.provider_channels.insert(
                provider.clone(),
                (d.region.to_lowercase(), d.channel.to_lowercase()),
            );
            if let Some(ttl) = d.cache_ttl_minutes {
                options.cache_ttl_minutes.insert(provider, ttl);
            }
        }
        options
    }
}

/// 用户可见设置（settings 表持久化；语言初值 zh-CN，多语言实施随 F3 约定）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub timezone: String,
    /// None = 跟随语言地区习惯（zh 系→周一；en-US/CA→周日；其余周一）。
    pub week_start: Option<u8>,
    /// 分级归档保留。
    #[serde(default)]
    pub retention: RetentionTiers,
    /// 今日数据刷新提取间隔（秒）；0 = 关闭自动提取。默认每小时。
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval_secs: u64,
    #[serde(default = "default_pause_on_saver")]
    pub pause_on_battery_saver: bool,
    #[serde(default)]
    pub close_to_tray: bool,
    #[serde(default)]
    pub file_watch_enabled: bool,
    pub language: String,
    /// 主题：system（跟随系统）/ light / dark。
    #[serde(default = "default_theme")]
    pub theme: String,
    /// 手工添加的数据源根目录。
    #[serde(default)]
    pub manual_roots: Vec<String>,
    /// Explicitly restrict discovery to configured roots, including OS launches.
    #[serde(default)]
    pub manual_roots_only: bool,
    /// 本机来源身份显示名（仅辨认用途，不改 host_id 键）。
    #[serde(default)]
    pub hostname_alias: Option<String>,
    /// 本地 OTLP 接收器（M5，按需启用；默认关闭。仅 127.0.0.1）。
    #[serde(default)]
    pub otel_receiver_enabled: bool,
    #[serde(default = "default_otel_receiver_port")]
    pub otel_receiver_port: u16,
    /// 费用估算（F2；默认关闭，启用前不展示费用面板）。
    #[serde(default)]
    pub pricing: PricingSettings,
}

fn default_otel_receiver_port() -> u16 {
    4318
}

fn default_theme() -> String {
    "system".to_string()
}

fn default_refresh_interval() -> u64 {
    3600
}

fn default_pause_on_saver() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            timezone: system_timezone_name(),
            week_start: None,
            retention: RetentionTiers::default(),
            refresh_interval_secs: default_refresh_interval(),
            pause_on_battery_saver: true,
            close_to_tray: false,
            file_watch_enabled: false,
            language: "zh-CN".to_string(),
            theme: default_theme(),
            manual_roots: Vec::new(),
            manual_roots_only: false,
            hostname_alias: None,
            otel_receiver_enabled: false,
            otel_receiver_port: default_otel_receiver_port(),
            pricing: PricingSettings::default(),
        }
    }
}

impl AppSettings {
    /// 周起始生效值：显式设置优先；否则按语言地区（zh→周一；en-US/CA→周日；
    /// 其余周一）。core 仅支持 0/6。
    pub fn effective_week_start(&self) -> u8 {
        match self.week_start {
            Some(w) if w == 0 || w == 6 => w,
            _ => {
                let lang = self.language.to_lowercase();
                if lang.starts_with("zh") {
                    0
                } else if lang.starts_with("en") && (lang.contains("us") || lang.contains("ca")) {
                    6
                } else {
                    0
                }
            }
        }
    }
}

/// 系统 IANA 时区名；取不到时退回 UTC（记录在设置中可见可改）。
/// OS 当前用户名（Windows USERPROFILE / Unix USER 环境变量推导）。
fn os_username() -> String {
    std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .ok()
        .filter(|s| !s.is_empty() && *s != "default")
        .unwrap_or_else(|| "default".to_string())
}

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
    /// 采集进度（百分比 + 预计剩余秒；UI 轮询展示）。
    pub progress_percent: u8,
    pub eta_seconds: Option<u64>,
    /// 已完成的适配器名（按序）。
    pub completed_adapters: Vec<String>,
    /// A manual refresh received during a running scan is consumed once afterward.
    #[serde(skip)]
    pub pending_manual: bool,
    /// Only full scans reset the inherited global cadence, never source-only runs.
    #[serde(skip)]
    pub last_global_finished_ms: i64,
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
    readers: [Mutex<Option<Storage>>; 2],
    pub host_id: Mutex<String>,
    pub settings: Mutex<AppSettings>,
    pub refresh: Mutex<RefreshState>,
    pub source_intervals: Mutex<crate::source_intervals::SourceIntervals>,
    pub scheduler_wakeups: std::sync::atomic::AtomicU64,
    pub automatic_pause_requests: std::sync::atomic::AtomicUsize,
    pub disabled_during_scan: Mutex<std::collections::BTreeSet<String>>,
    pub db_path: PathBuf,
    /// 当前统计用户（v6 多用户约定；默认 "default"，存 settings 表）。
    pub current_user: Mutex<String>,
    /// 「清理全部数据并重新采集」后台任务运行中（防重复触发；UI 不阻塞）。
    pub clear_job_running: std::sync::atomic::AtomicBool,
    pub cleanup_control: std::sync::Arc<llm_usage_core::cancellation::OperationControl>,
    /// F2 在线刷新（models.dev）状态：运行标记 + 最近一次结果（UI 轮询）。
    pub price_refresh: Mutex<crate::price_refresh::PriceRefreshState>,
}

impl AppState {
    /// 打开/迁移应用数据库并初始化主机身份与设置。
    pub fn init(db_path: PathBuf, hostname: &str, interactive: bool) -> Result<Self, String> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        // 预发布阶段：不做逐版本迁移；版本不匹配时提示用户"删除重建或退出"。
        let storage = match Storage::open(&db_path) {
            Ok(s) => s,
            Err(llm_usage_core::error::CoreError::SchemaMismatch { found, expected })
            | Err(llm_usage_core::error::CoreError::SchemaTooNew {
                found,
                supported: expected,
            }) => {
                if !interactive {
                    return Err(format!("database schema mismatch: found {found}, expected {expected}; open the desktop app to review recovery options"));
                }
                let msg = format!(
                    "数据库版本不兼容（当前 {found}，期望 {expected}）。

                     是否允许删除现有数据库并重新创建？
                     这将清除所有已采集的统计数据。"
                );
                let choice = rfd::MessageDialog::new()
                    .set_title("数据库版本过低")
                    .set_description(&msg)
                    .set_buttons(rfd::MessageButtons::YesNo)
                    .set_level(rfd::MessageLevel::Warning)
                    .show();
                if choice != rfd::MessageDialogResult::Yes {
                    return Err("__EXIT_SCHEMA_MISMATCH__".to_string());
                }
                // 重建前一致备份（M1/V15 约定：无备份不删除；空间不足也中止）。
                crate::db_backup::consistent_backup_legacy(
                    &db_path,
                    "llm-usage-rebuild",
                    crate::scanner::now_ms(),
                )
                .map_err(|e| {
                    format!("abort rebuild: database kept untouched. backup failed: {e}")
                })?;
                for suffix in ["", "-wal", "-shm"] {
                    let p = std::path::PathBuf::from(format!("{}{}", db_path.display(), suffix));
                    let _ = std::fs::remove_file(&p);
                }
                Storage::open(&db_path).map_err(|e| e.to_string())?
            }
            Err(e) => return Err(e.to_string()),
        };
        let now = now_ms();
        let host_id = storage
            .ensure_local_host(hostname, now)
            .map_err(|e| e.to_string())?;
        // F2：幂等导入仓库种子价格快照（本地元数据；失败不阻塞采集，仅提示）。
        if let Err(e) = storage.ensure_seed_price_snapshot(now) {
            eprintln!("price seed snapshot import failed: {e}");
        }
        let settings = load_settings(&storage);
        llm_usage_core::schedules::pin_legacy_timezones(&storage, &settings.timezone)
            .map_err(|e| e.to_string())?;
        // 时区分区修复：老版本扫描以 UTC 写日分区而用户统计时区不同 ⇒
        // 在用户时区下重算事件覆盖范围（推导非猜测；2026-09-26 缺陷修复）。
        repair_tz_partitions(&storage, &settings.timezone, false)?;
        // 默认用户名取 OS 当前用户（不再使用 "default"）；首次初始化时
        // 把 v6 迁移创建的 default 用户重命名为 OS 用户名（保留同一 user_id
        // 键，来源归属不变）。
        let os_user = os_username();
        let stored: Option<String> = storage
            .conn()
            .query_row(
                "SELECT value FROM settings WHERE key = 'current_user'",
                [],
                |r| r.get(0),
            )
            .ok();
        let current_user = match stored {
            Some(u) => u,
            None => {
                // 首次：把 default 改名为 OS 用户名（或创建）。
                let _ = storage.conn().execute(
                    "UPDATE users SET name = ?1 WHERE user_id = 'default' AND name = 'default'",
                    rusqlite::params![os_user],
                );
                let _ = storage.conn().execute(
                    "INSERT INTO settings (key, value, schema_version, updated_at_ms)
                     VALUES ('current_user', 'default', 1, ?1)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    rusqlite::params![now],
                );
                "default".to_string()
            }
        };
        Ok(AppState {
            storage: Mutex::new(storage),
            host_id: Mutex::new(host_id),
            settings: Mutex::new(settings),
            readers: std::array::from_fn(|_| Mutex::new(None)),
            refresh: Mutex::new(RefreshState::default()),
            source_intervals: Mutex::new(crate::source_intervals::SourceIntervals::default()),
            scheduler_wakeups: std::sync::atomic::AtomicU64::new(0),
            automatic_pause_requests: std::sync::atomic::AtomicUsize::new(0),
            disabled_during_scan: Mutex::new(Default::default()),
            current_user: Mutex::new(current_user),
            db_path,
            clear_job_running: std::sync::atomic::AtomicBool::new(false),
            cleanup_control: std::sync::Arc::default(),
            price_refresh: Mutex::new(crate::price_refresh::PriceRefreshState::default()),
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
pub fn repair_tz_partitions(storage: &Storage, timezone: &str, force: bool) -> Result<(), String> {
    let has_user_tz: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM daily_usage WHERE tz_version = ?1",
            [timezone],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let repaired: bool = storage
        .conn()
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM settings WHERE key=?1 AND value='1')",
            [format!("hourly_fields_version:{timezone}")],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if has_user_tz > 0 && repaired && !force {
        return Ok(());
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
        llm_usage_core::ingest::recompute_days_in_tz(storage, timezone, min_ms, max_ms, now_ms())
            .map_err(|e| format!("timezone partition rebuild failed: {e}"))?;
    }
    // F2：其他时区分区已被本次重建取代——清理其未封存日成本行（封存的历史
    // 金额保留；当前时区的行由后续扫描回填/显式重算重建）。
    storage
        .conn()
        .execute(
            "DELETE FROM daily_cost_usage WHERE sealed = 0 AND tz_version != ?1",
            [timezone],
        )
        .map_err(|e| e.to_string())?;
    storage
        .conn()
        .execute(
            "INSERT INTO settings(key,value,schema_version,updated_at_ms) VALUES (?1,'1',1,?2)
        ON CONFLICT(key) DO UPDATE SET value='1',updated_at_ms=excluded.updated_at_ms",
            rusqlite::params![format!("hourly_fields_version:{timezone}"), now_ms()],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 查询连接：只读连接（带短重试——观察到的间歇性 disk I/O error 发生在
/// 打开瞬间）；仍失败时回退写连接互斥锁，UI 查询宁可短暂排队也不硬错。
pub enum ReadConn<'a> {
    Ro(Box<Storage>),
    Pooled(std::sync::MutexGuard<'a, Option<Storage>>),
    Writer(std::sync::MutexGuard<'a, Storage>),
}

impl std::ops::Deref for ReadConn<'_> {
    type Target = Storage;
    fn deref(&self) -> &Storage {
        match self {
            ReadConn::Ro(s) => s,
            ReadConn::Pooled(slot) => slot.as_ref().expect("initialized read connection"),
            ReadConn::Writer(guard) => guard,
        }
    }
}

pub fn read_conn(state: &AppState) -> ReadConn<'_> {
    for reader in &state.readers {
        if let Ok(mut slot) = reader.try_lock() {
            if slot.is_none() {
                for _ in 0..3 {
                    if let Ok(storage) = Storage::open_readonly(&state.db_path) {
                        *slot = Some(storage);
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(40));
                }
            }
            if slot.is_some() {
                return ReadConn::Pooled(slot);
            }
        }
    }
    for _ in 0..3 {
        if let Ok(s) = Storage::open_readonly(&state.db_path) {
            return ReadConn::Ro(Box::new(s));
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
    let mut settings: AppSettings = raw
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default();
    // 规整费用缓存 TTL：旧版派生 Default 会持久化 0（非法，合法范围 1–365）；
    // 0 或越界回落默认值；保留合法的用户设置。
    let ttl = settings.pricing.online_cache_ttl_days;
    if !(crate::price_refresh::MIN_TTL_DAYS..=crate::price_refresh::MAX_TTL_DAYS).contains(&ttl) {
        settings.pricing.online_cache_ttl_days = crate::price_refresh::DEFAULT_TTL_DAYS;
    }
    settings
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

#[cfg(test)]
mod pricing_settings_tests {
    use super::*;
    use llm_usage_core::storage::Storage;

    #[test]
    fn default_cache_ttl_is_three_days() {
        assert_eq!(
            PricingSettings::default().online_cache_ttl_days,
            crate::price_refresh::DEFAULT_TTL_DAYS
        );
        assert_eq!(
            AppSettings::default().pricing.online_cache_ttl_days,
            crate::price_refresh::DEFAULT_TTL_DAYS
        );
    }

    #[test]
    fn load_settings_normalizes_invalid_cache_ttl() {
        let storage = Storage::open_in_memory().unwrap();
        // 模拟旧版派生 Default 持久化的非法值 0：加载时应回落默认 3，保留其他字段。
        let mut s = AppSettings::default();
        s.pricing.enabled = true;
        s.pricing.online_cache_ttl_days = 0;
        save_settings(&storage, &s).unwrap();
        let loaded = load_settings(&storage);
        assert_eq!(
            loaded.pricing.online_cache_ttl_days,
            crate::price_refresh::DEFAULT_TTL_DAYS
        );
        assert!(loaded.pricing.enabled);
    }

    #[test]
    fn load_settings_keeps_valid_cache_ttl() {
        let storage = Storage::open_in_memory().unwrap();
        let mut s = AppSettings::default();
        s.pricing.online_cache_ttl_days = 30;
        save_settings(&storage, &s).unwrap();
        assert_eq!(load_settings(&storage).pricing.online_cache_ttl_days, 30);
    }

    #[test]
    fn missing_pricing_and_ttl_fields_use_default_without_enabling_network() {
        let storage = Storage::open_in_memory().unwrap();
        for mode in 0..3 {
            let mut value = serde_json::to_value(AppSettings::default()).unwrap();
            if mode == 0 {
                value.as_object_mut().unwrap().remove("pricing");
            } else {
                value["pricing"]
                    .as_object_mut()
                    .unwrap()
                    .remove("online_cache_ttl_days");
                value["pricing"]["enabled"] = serde_json::json!(mode == 2);
            }
            storage.conn().execute("INSERT OR REPLACE INTO settings(key,value,schema_version,updated_at_ms) VALUES(?1,?2,1,0)",rusqlite::params![SETTINGS_KEY,value.to_string()]).unwrap();
            let loaded = load_settings(&storage);
            assert_eq!(loaded.pricing.online_cache_ttl_days, 3);
            assert!(!loaded.pricing.online_refresh_enabled);
            assert_eq!(loaded.pricing.enabled, mode == 2);
        }
    }

    #[test]
    fn cache_ttl_boundaries_are_normalized_without_changing_other_settings() {
        let storage = Storage::open_in_memory().unwrap();
        for (ttl, expected) in [(0, 3), (1, 1), (365, 365), (366, 3), (u32::MAX, 3)] {
            let mut settings = AppSettings::default();
            settings.pricing.online_cache_ttl_days = ttl;
            settings
                .pricing
                .provider_defaults
                .push(ProviderPricingDefault {
                    provider_id: "custom".into(),
                    region: "cn".into(),
                    channel: "api".into(),
                    cache_ttl_minutes: None,
                });
            save_settings(&storage, &settings).unwrap();
            let loaded = load_settings(&storage);
            assert_eq!(loaded.pricing.online_cache_ttl_days, expected);
            assert_eq!(
                loaded.pricing.provider_defaults,
                settings.pricing.provider_defaults
            );
        }
    }
}

#[cfg(test)]
mod partition_tests {
    #[test]
    fn pooled_readers_release_snapshots_and_observe_external_commits() {
        use llm_usage_core::calendar::{ymd, WeekStart};
        use llm_usage_core::query::{query_summary, Filters, Granularity, SummaryRequest};
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/query-readers")
            .join(format!("{}-{}", std::process::id(), super::now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(dir.clone());
        let state = super::AppState::init(dir.join("reader.sqlite"), "test", false).unwrap();
        let request = SummaryRequest {
            timezone: "UTC".into(),
            week_start: WeekStart::Monday,
            first_day: ymd(2026, 10, 1),
            last_day: ymd(2026, 10, 1),
            today: ymd(2026, 10, 1),
            granularity: Granularity::Day,
            filters: Filters::default(),
            retention_cutoff: None,
        };
        let first = super::read_conn(&state);
        let second = super::read_conn(&state);
        let overflow = super::read_conn(&state);
        assert!(matches!(first, super::ReadConn::Pooled(_)));
        assert!(matches!(second, super::ReadConn::Pooled(_)));
        assert!(matches!(overflow, super::ReadConn::Ro(_)));
        for reader in [&first, &second, &overflow] {
            for _ in 0..2 {
                assert_eq!(
                    query_summary(reader, &request).unwrap().totals.call_count,
                    0
                );
                assert!(
                    reader.conn().is_autocommit(),
                    "queries retain no WAL snapshot"
                );
            }
        }
        {
            let writer = state.storage.lock().unwrap();
            let at = "2026-10-01T12:00:00Z"
                .parse::<jiff::Timestamp>()
                .unwrap()
                .as_millisecond();
            for key in ["one", "two", "three"] {
                writer.conn().execute(
                    "INSERT INTO usage_events(event_id,source_instance_id,source_record_key,record_kind,schema_version,parser_version,agent,occurred_at_ms,time_basis,quality_json,quality_bucket,lifecycle,content_hash,created_at_ms,updated_at_ms)
                     VALUES (?1,'source',?1,'model_call','1','test','example',?2,'source_completion','{}','unknown','final',?1,?2,?2)",
                    rusqlite::params![key, at],
                ).unwrap();
            }
            llm_usage_core::ingest::recompute_days_in_tz(&writer, "UTC", at, at, at).unwrap();
        }
        for reader in [&first, &second] {
            assert_eq!(
                query_summary(reader, &request).unwrap().totals.call_count,
                3
            );
            assert!(reader.conn().is_autocommit());
        }
        drop(first);
        let reused = super::read_conn(&state);
        assert!(matches!(reused, super::ReadConn::Pooled(_)));
        assert_eq!(
            query_summary(&reused, &request).unwrap().totals.call_count,
            3
        );
        assert!(reused.conn().is_autocommit());
    }

    #[test]
    fn returning_to_a_previous_timezone_rebuilds_new_details_and_hours() {
        let storage = llm_usage_core::storage::Storage::open_in_memory().unwrap();
        let at = "2026-09-27T20:00:00Z"
            .parse::<jiff::Timestamp>()
            .unwrap()
            .as_millisecond();
        let insert = |id: &str| {
            storage.conn().execute(
            "INSERT INTO usage_events(event_id,source_instance_id,source_record_key,record_kind,schema_version,parser_version,agent,occurred_at_ms,time_basis,quality_json,quality_bucket,lifecycle,content_hash,created_at_ms,updated_at_ms)
             VALUES (?1,'source',?1,'model_call','1','test','codex',?2,'source_completion','{}','unknown','final',?1,?2,?2)",
            rusqlite::params![id,at]).unwrap()
        };
        insert("first");
        super::repair_tz_partitions(&storage, "UTC", false).unwrap();
        super::repair_tz_partitions(&storage, "Asia/Shanghai", true).unwrap();
        insert("second");
        super::repair_tz_partitions(&storage, "UTC", true).unwrap();
        for table in ["daily_usage", "hourly_usage"] {
            let count: i64 = storage
                .conn()
                .query_row(
                    &format!("SELECT SUM(call_count) FROM {table} WHERE tz_version='UTC'"),
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(
                count, 2,
                "{table} must include events collected under the other timezone"
            );
        }
    }
}
