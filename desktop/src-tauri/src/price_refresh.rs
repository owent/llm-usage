//! F2 在线刷新（models.dev 社区目录）：抓取、原始响应缓存、失败回退与幂等导入。
//!
//! 合同（docs/design/desktop-usage/pricing.md · 在线刷新设计）：
//! - 默认关闭；启用后仅 HTTPS GET models.dev api.json，请求不携带任何本地用量、
//!   主机/来源身份、会话内容或账户密钥；
//! - 原始响应缓存于 `<数据库目录>/price-cache/`，TTL 默认 3 天（1–365 可配），
//!   新鲜期内不发网络请求；
//! - 下载或校验失败回退到上一次成功下载的缓存；无缓存时报错并保留既有快照（A9）；
//! - 校验失败的响应不覆盖缓存；导入幂等，不触发既有估算重算。

use llm_usage_core::models_dev::{
    content_hash, snapshot_from_models_dev, MODELS_DEV_API_URL, MODELS_DEV_MAX_BYTES,
};
use llm_usage_core::storage::Storage;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 缓存 TTL 默认值（天）与合法范围。
pub const DEFAULT_TTL_DAYS: u32 = 3;
pub const MIN_TTL_DAYS: u32 = 1;
pub const MAX_TTL_DAYS: u32 = 365;

/// 缓存元数据（sidecar JSON；随文件系统保留，数据库重建不影响）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMeta {
    pub url: String,
    pub fetched_at_ms: i64,
    /// FNV-1a 内容哈希十六进制（快照 ID 组成部分）。
    pub content_hash: String,
    pub bytes: u64,
}

/// 缓存状态（界面展示新鲜度）。
#[derive(Debug, Clone, Serialize)]
pub struct PriceCacheInfo {
    pub fetched_at_ms: i64,
    pub bytes: u64,
    pub content_hash: String,
    pub age_secs: i64,
}

/// 一次刷新的结果（UI 状态与操作日志）。
#[derive(Debug, Clone, Default, Serialize)]
pub struct PriceRefreshOutcome {
    /// fetched / cache_fresh / fetch_failed_used_cache / fetch_failed_no_cache。
    pub status: String,
    pub snapshot_id: Option<String>,
    pub inserted_rows: usize,
    pub already_present: bool,
    pub cache: Option<PriceCacheInfo>,
    pub error: Option<String>,
}

/// 可观察的刷新状态（AppState 持有；UI 轮询）。
#[derive(Debug, Default, Serialize)]
pub struct PriceRefreshState {
    pub running: bool,
    pub last_outcome: Option<PriceRefreshOutcome>,
}

fn cache_paths(db_path: &Path) -> (PathBuf, PathBuf) {
    let dir = db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("price-cache");
    (
        dir.join("models-dev-api.json"),
        dir.join("models-dev-api.meta.json"),
    )
}

/// 读取缓存（meta 缺失/损坏时以文件 mtime 重建元数据——下载失败时仍能回退）。
fn read_cache(db_path: &Path) -> Option<(Vec<u8>, CacheMeta)> {
    let (json_path, meta_path) = cache_paths(db_path);
    let bytes = std::fs::read(&json_path).ok()?;
    if bytes.is_empty() {
        return None;
    }
    if let Ok(text) = std::fs::read_to_string(&meta_path) {
        if let Ok(meta) = serde_json::from_str::<CacheMeta>(&text) {
            return Some((bytes, meta));
        }
    }
    let mtime_ms = std::fs::metadata(&json_path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    Some((
        bytes.clone(),
        CacheMeta {
            url: MODELS_DEV_API_URL.to_string(),
            fetched_at_ms: mtime_ms,
            content_hash: format!("{:016x}", content_hash(&bytes)),
            bytes: bytes.len() as u64,
        },
    ))
}

/// 原子写缓存（tmp + rename；仅在内容通过校验后调用）。
fn write_cache(db_path: &Path, bytes: &[u8], meta: &CacheMeta) -> Result<(), String> {
    let (json_path, meta_path) = cache_paths(db_path);
    let dir = json_path.parent().expect("cache path has parent");
    std::fs::create_dir_all(dir).map_err(|e| format!("create price-cache dir: {e}"))?;
    let tmp_json = dir.join("models-dev-api.json.tmp");
    let tmp_meta = dir.join("models-dev-api.meta.json.tmp");
    std::fs::write(&tmp_json, bytes).map_err(|e| format!("write cache tmp: {e}"))?;
    let meta_json =
        serde_json::to_string_pretty(meta).map_err(|e| format!("serialize cache meta: {e}"))?;
    std::fs::write(&tmp_meta, meta_json).map_err(|e| format!("write cache meta tmp: {e}"))?;
    std::fs::rename(&tmp_json, &json_path).map_err(|e| format!("rename cache: {e}"))?;
    std::fs::rename(&tmp_meta, &meta_path).map_err(|e| format!("rename cache meta: {e}"))?;
    Ok(())
}

/// HTTPS GET models.dev api.json（ureq/rustls；全局超时；响应体有界）。
pub fn http_fetch(url: &str) -> Result<Vec<u8>, String> {
    let agent = ureq::Agent::config_builder()
        .https_only(true)
        .timeout_connect(Some(std::time::Duration::from_secs(15)))
        .timeout_global(Some(std::time::Duration::from_secs(120)))
        .max_redirects(3)
        .user_agent(concat!("llm-usage/", env!("CARGO_PKG_VERSION")))
        .build()
        .new_agent();
    let mut response = agent
        .get(url)
        .call()
        .map_err(|e| format!("GET {url}: {e}"))?;
    if response.status() != 200 {
        return Err(format!("GET {url}: HTTP {}", response.status()));
    }
    response
        .body_mut()
        .with_config()
        .limit(MODELS_DEV_MAX_BYTES)
        .read_to_vec()
        .map_err(|e| format!("read {url} body: {e}"))
}

fn cache_info(meta: &CacheMeta, now_ms: i64) -> PriceCacheInfo {
    PriceCacheInfo {
        fetched_at_ms: meta.fetched_at_ms,
        bytes: meta.bytes,
        content_hash: meta.content_hash.clone(),
        age_secs: (now_ms - meta.fetched_at_ms).max(0) / 1000,
    }
}

/// 从给定内容导入快照（幂等）；成功返回 (snapshot_id, inserted, already_present)。
fn import_cached_content(
    storage: &Storage,
    bytes: &[u8],
    meta: &CacheMeta,
    now_ms: i64,
) -> Result<(String, usize, bool), String> {
    let text =
        String::from_utf8(bytes.to_vec()).map_err(|e| format!("cache content not UTF-8: {e}"))?;
    let hash = u64::from_str_radix(&meta.content_hash, 16).unwrap_or_else(|_| content_hash(bytes));
    let snapshot = snapshot_from_models_dev(&text, meta.fetched_at_ms, hash)
        .map_err(|e| format!("convert cached models.dev content: {e}"))?;
    let outcome = storage
        .import_price_snapshot(&snapshot, now_ms)
        .map_err(|e| format!("import snapshot: {e}"))?;
    Ok((
        outcome.snapshot_id,
        outcome.inserted_rows,
        outcome.already_present,
    ))
}

/// 在线刷新主流程（可注入抓取函数以便测试）。
///
/// `force` 绕过 TTL（手动「立即刷新」）。任何失败都保留既有快照与缓存语义：
/// 抓取/校验失败时回退到上次成功下载的内容导入；无缓存时报错。
pub fn refresh_prices_with(
    storage: &Storage,
    db_path: &Path,
    now_ms: i64,
    ttl_days: u32,
    force: bool,
    fetch: &dyn Fn() -> Result<Vec<u8>, String>,
) -> PriceRefreshOutcome {
    let cached = read_cache(db_path);
    // 1) 缓存新鲜且非强制：不发网络请求，幂等导入缓存内容。
    if !force {
        if let Some((bytes, meta)) = &cached {
            let ttl_ms = (ttl_days.max(MIN_TTL_DAYS) as i64) * 86_400_000;
            if now_ms - meta.fetched_at_ms < ttl_ms {
                return match import_cached_content(storage, bytes, meta, now_ms) {
                    Ok((id, inserted, already)) => PriceRefreshOutcome {
                        status: "cache_fresh".to_string(),
                        snapshot_id: Some(id),
                        inserted_rows: inserted,
                        already_present: already,
                        cache: Some(cache_info(meta, now_ms)),
                        error: None,
                    },
                    Err(e) => PriceRefreshOutcome {
                        status: "fetch_failed_no_cache".to_string(),
                        cache: Some(cache_info(meta, now_ms)),
                        error: Some(e),
                        ..Default::default()
                    },
                };
            }
        }
    }
    // 2) 抓取 + 校验 + 写缓存 + 导入。
    match fetch().and_then(|bytes| {
        let text =
            String::from_utf8(bytes.clone()).map_err(|e| format!("response not UTF-8: {e}"))?;
        let hash = content_hash(&bytes);
        // 先校验转换，通过后才覆盖缓存文件。
        let snapshot = snapshot_from_models_dev(&text, now_ms, hash)
            .map_err(|e| format!("validate models.dev content: {e}"))?;
        let meta = CacheMeta {
            url: MODELS_DEV_API_URL.to_string(),
            fetched_at_ms: now_ms,
            content_hash: format!("{hash:016x}"),
            bytes: bytes.len() as u64,
        };
        write_cache(db_path, &bytes, &meta)?;
        let outcome = storage
            .import_price_snapshot(&snapshot, now_ms)
            .map_err(|e| format!("import snapshot: {e}"))?;
        Ok((meta, outcome))
    }) {
        Ok((meta, outcome)) => PriceRefreshOutcome {
            status: "fetched".to_string(),
            snapshot_id: Some(outcome.snapshot_id),
            inserted_rows: outcome.inserted_rows,
            already_present: outcome.already_present,
            cache: Some(cache_info(&meta, now_ms)),
            error: None,
        },
        Err(e) => {
            // 3) 失败回退：上一次成功下载的缓存（A9）。
            match cached {
                Some((bytes, meta)) => {
                    match import_cached_content(storage, &bytes, &meta, now_ms) {
                        Ok((id, inserted, already)) => PriceRefreshOutcome {
                            status: "fetch_failed_used_cache".to_string(),
                            snapshot_id: Some(id),
                            inserted_rows: inserted,
                            already_present: already,
                            cache: Some(cache_info(&meta, now_ms)),
                            error: Some(e),
                        },
                        Err(import_err) => PriceRefreshOutcome {
                            status: "fetch_failed_no_cache".to_string(),
                            cache: Some(cache_info(&meta, now_ms)),
                            error: Some(format!("{e}; cached content unusable: {import_err}")),
                            ..Default::default()
                        },
                    }
                }
                None => PriceRefreshOutcome {
                    status: "fetch_failed_no_cache".to_string(),
                    error: Some(e),
                    ..Default::default()
                },
            }
        }
    }
}

/// 状态查询用缓存信息（无缓存时为 None）。
pub fn cache_info_for_status(db_path: &Path, now_ms: i64) -> Option<PriceCacheInfo> {
    read_cache(db_path).map(|(_, meta)| cache_info(&meta, now_ms))
}

/// 命令/调度入口：按当前设置执行一次刷新（后台线程调用；写操作日志）。
pub fn run_price_refresh(state: &crate::app_state::AppState, force: bool) -> PriceRefreshOutcome {
    let (ttl_days, now) = (
        state
            .settings
            .lock()
            .unwrap()
            .pricing
            .online_cache_ttl_days
            .clamp(MIN_TTL_DAYS, MAX_TTL_DAYS),
        crate::scanner::now_ms(),
    );
    let outcome = {
        let storage = state.storage.lock().unwrap();
        refresh_prices_with(&storage, &state.db_path, now, ttl_days, force, &|| {
            http_fetch(MODELS_DEV_API_URL)
        })
    };
    {
        let storage = state.storage.lock().unwrap();
        crate::commands::log_operation(
            &storage,
            "price_online_refresh",
            &format!(
                "status={} snapshot={:?} rows={} error={:?}",
                outcome.status, outcome.snapshot_id, outcome.inserted_rows, outcome.error
            ),
        );
    }
    outcome
}

/// 采集结束后的自动刷新检查：启用且缓存过期（或缺失）时后台刷新一次。
/// 并发重入由 AppState.price_refresh 的 running 标记挡下；失败只留痕不阻塞。
pub fn maybe_auto_refresh(state: &std::sync::Arc<crate::app_state::AppState>, force: bool) -> bool {
    {
        let settings = state.settings.lock().unwrap();
        if !settings.pricing.enabled || !settings.pricing.online_refresh_enabled {
            return false;
        }
    }
    if !force {
        let ttl_days = state
            .settings
            .lock()
            .unwrap()
            .pricing
            .online_cache_ttl_days
            .clamp(MIN_TTL_DAYS, MAX_TTL_DAYS);
        let stale = match read_cache(&state.db_path) {
            Some((_, meta)) => {
                let ttl_ms = (ttl_days as i64) * 86_400_000;
                crate::scanner::now_ms() - meta.fetched_at_ms >= ttl_ms
            }
            None => true,
        };
        if !stale {
            return false;
        }
    }
    {
        let mut refresh = state.price_refresh.lock().unwrap();
        if refresh.running {
            return false;
        }
        refresh.running = true;
    }
    let state = state.clone();
    std::thread::spawn(move || {
        let outcome = run_price_refresh(&state, force);
        let mut refresh = state.price_refresh.lock().unwrap();
        refresh.running = false;
        refresh.last_outcome = Some(outcome);
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_usage_core::storage::Storage;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// 最小合法 api.json：官方 vendorA（canonical 前缀）一个按量价模型。
    const MINI: &str = r#"{"vendorA":{"id":"vendorA","models":{"m-one":{"id":"m-one","canonical_model_id":"vendorA/m-one","cost":{"input":10,"output":50,"cache_read":1}}}}}"#;

    /// 真实 HTTPS 抓取冒烟：ureq/rustls 链路 + 响应体上限 + 实载转换。
    /// 需网络；CI 不运行（`cargo test -p llm-usage-desktop http_fetch_live -- --ignored`）。
    #[test]
    #[ignore = "hits the live models.dev endpoint; maintainer-run smoke"]
    fn http_fetch_live_models_dev_smoke() {
        let bytes = http_fetch(MODELS_DEV_API_URL).expect("live fetch");
        assert!(bytes.len() > 1_000_000, "api.json is multi-MB");
        let text = String::from_utf8(bytes.clone()).unwrap();
        let snapshot =
            snapshot_from_models_dev(&text, crate::scanner::now_ms(), content_hash(&bytes))
                .expect("live payload converts");
        assert!(!snapshot.rows.is_empty());
    }

    fn temp_db_path(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "llm-usage-price-refresh-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("llm-usage.sqlite")
    }

    fn seed_cache(db_path: &Path, content: &str, fetched_at_ms: i64) {
        let bytes = content.as_bytes();
        let meta = CacheMeta {
            url: MODELS_DEV_API_URL.to_string(),
            fetched_at_ms,
            content_hash: format!("{:016x}", content_hash(bytes)),
            bytes: bytes.len() as u64,
        };
        write_cache(db_path, bytes, &meta).unwrap();
    }

    fn priced_rows(storage: &Storage) -> usize {
        storage.load_price_book().unwrap().rows.len()
    }

    const NOW: i64 = 1_790_000_000_000; // 2026-09-30 UTC 附近
    const DAY: i64 = 86_400_000;

    /// A9 主路径：下载失败 ⇒ 回退上一次成功下载的缓存继续导入。
    #[test]
    fn failed_fetch_falls_back_to_last_successful_cache() {
        let db = temp_db_path("a9-cache");
        let storage = Storage::open_in_memory().unwrap();
        seed_cache(&db, MINI, NOW - 30 * DAY); // 陈旧缓存（30 天前）
        let calls = AtomicUsize::new(0);
        let outcome = refresh_prices_with(&storage, &db, NOW, DEFAULT_TTL_DAYS, false, &|| {
            calls.fetch_add(1, Ordering::SeqCst);
            Err("network down".to_string())
        });
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(outcome.status, "fetch_failed_used_cache");
        assert!(outcome.error.unwrap().contains("network down"));
        assert!(outcome.snapshot_id.is_some());
        assert_eq!(priced_rows(&storage), 1);
        let _ = std::fs::remove_dir_all(db.parent().unwrap());
    }

    /// A9 无缓存：下载失败 ⇒ 报错且既有快照不受影响。
    #[test]
    fn failed_fetch_without_cache_keeps_existing_snapshots() {
        let db = temp_db_path("a9-none");
        let storage = Storage::open_in_memory().unwrap();
        storage.ensure_seed_price_snapshot(NOW).unwrap();
        let before = priced_rows(&storage);
        let outcome = refresh_prices_with(&storage, &db, NOW, DEFAULT_TTL_DAYS, false, &|| {
            Err("dns failed".to_string())
        });
        assert_eq!(outcome.status, "fetch_failed_no_cache");
        assert!(outcome.snapshot_id.is_none());
        assert_eq!(priced_rows(&storage), before); // 既有快照原样保留
        let _ = std::fs::remove_dir_all(db.parent().unwrap());
    }

    /// TTL 内不发网络请求（缓存时间足够长，减少重复下载）。
    #[test]
    fn fresh_cache_skips_network() {
        let db = temp_db_path("fresh");
        let storage = Storage::open_in_memory().unwrap();
        seed_cache(&db, MINI, NOW - DAY); // 1 天前 < 默认 3 天 TTL
        let outcome = refresh_prices_with(&storage, &db, NOW, DEFAULT_TTL_DAYS, false, &|| {
            panic!("network must not be called within TTL")
        });
        assert_eq!(outcome.status, "cache_fresh");
        assert!(outcome.already_present || outcome.inserted_rows == 1);
        assert_eq!(priced_rows(&storage), 1);
        // force 绕过 TTL：抓取被调用。
        let calls = AtomicUsize::new(0);
        let outcome = refresh_prices_with(&storage, &db, NOW, DEFAULT_TTL_DAYS, true, &|| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(MINI.as_bytes().to_vec())
        });
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(outcome.status, "fetched");
        let _ = std::fs::remove_dir_all(db.parent().unwrap());
    }

    /// 校验失败的响应不覆盖缓存，并回退到旧缓存（A9 变体）。
    #[test]
    fn invalid_download_does_not_overwrite_cache() {
        let db = temp_db_path("invalid");
        let storage = Storage::open_in_memory().unwrap();
        seed_cache(&db, MINI, NOW - 30 * DAY);
        let outcome = refresh_prices_with(&storage, &db, NOW, DEFAULT_TTL_DAYS, false, &|| {
            Ok(b"garbage-not-json".to_vec())
        });
        assert_eq!(outcome.status, "fetch_failed_used_cache");
        assert!(outcome
            .error
            .unwrap()
            .contains("validate models.dev content"));
        // 缓存内容仍是旧的成功下载。
        let (bytes, meta) = read_cache(&db).unwrap();
        assert_eq!(bytes, MINI.as_bytes());
        assert_eq!(meta.fetched_at_ms, NOW - 30 * DAY);
        assert_eq!(priced_rows(&storage), 1);
        let _ = std::fs::remove_dir_all(db.parent().unwrap());
    }

    /// 成功下载：写缓存 + 导入新快照；同内容重复刷新幂等。
    #[test]
    fn successful_fetch_writes_cache_and_imports_idempotently() {
        let db = temp_db_path("ok");
        let storage = Storage::open_in_memory().unwrap();
        let outcome = refresh_prices_with(&storage, &db, NOW, DEFAULT_TTL_DAYS, false, &|| {
            Ok(MINI.as_bytes().to_vec())
        });
        assert_eq!(outcome.status, "fetched");
        assert_eq!(outcome.inserted_rows, 1);
        assert_eq!(priced_rows(&storage), 1);
        let (_, meta) = read_cache(&db).expect("cache written");
        assert_eq!(meta.fetched_at_ms, NOW);
        // 强制再刷新同内容：快照 ID 相同 ⇒ 幂等跳过。
        let outcome = refresh_prices_with(&storage, &db, NOW, DEFAULT_TTL_DAYS, true, &|| {
            Ok(MINI.as_bytes().to_vec())
        });
        assert_eq!(outcome.status, "fetched");
        assert!(outcome.already_present);
        assert_eq!(priced_rows(&storage), 1);
        let _ = std::fs::remove_dir_all(db.parent().unwrap());
    }
}
