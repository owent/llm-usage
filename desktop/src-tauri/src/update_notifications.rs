use crate::app_state::AppState;
use crate::updates::stable_version;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::cleanup;

const RETRY_MS: i64 = 60 * 60 * 1000;
const MAX_ATTEMPTS: u8 = 3;
const ATTEMPT_LIMIT: &str = "notification_attempt_limit";

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct History {
    delivered: Option<String>,
    attempts: BTreeMap<String, Attempt>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Attempt {
    count: u8,
    at_ms: i64,
    error: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyHistory {
    delivered: Option<String>,
    attempt: Option<LegacyAttempt>,
    error: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyAttempt {
    version: String,
    at_ms: i64,
}

impl From<LegacyHistory> for History {
    fn from(old: LegacyHistory) -> Self {
        let mut history = Self {
            delivered: old.delivered,
            ..Self::default()
        };
        if let Some(attempt) = old.attempt {
            // The old format did not count retries; it cannot establish remaining attempts.
            history.attempts.insert(
                attempt.version,
                Attempt {
                    count: MAX_ATTEMPTS,
                    at_ms: attempt.at_ms,
                    error: old.error,
                },
            );
        }
        history
    }
}

fn history_key(cache: &Path) -> String {
    format!(
        "update_notification:{}",
        cache.file_name().unwrap_or_default().to_string_lossy()
    )
}

fn load(conn: &Connection, key: &str) -> Result<History, String> {
    let raw: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key=?1", [key], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|_| "notification_history_read_failed")?;
    let history: History = match raw {
        Some(raw) => serde_json::from_str(&raw)
            .or_else(|_| serde_json::from_str::<LegacyHistory>(&raw).map(History::from))
            .map_err(|_| "notification_history_invalid")?,
        None => History::default(),
    };
    for version in history.delivered.iter().chain(history.attempts.keys()) {
        stable_version(version).map_err(|_| "notification_history_invalid")?;
    }
    if history
        .attempts
        .values()
        .any(|a| a.count == 0 || a.count > MAX_ATTEMPTS)
    {
        return Err("notification_history_invalid".into());
    }
    Ok(history)
}

fn save(conn: &Connection, key: &str, history: &History, now: i64) -> Result<(), String> {
    conn.execute(
        "INSERT INTO settings(key,value,schema_version,updated_at_ms) VALUES(?1,?2,1,?3)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value,updated_at_ms=excluded.updated_at_ms",
        rusqlite::params![
            key,
            serde_json::to_string(history).map_err(|_| "notification_history_invalid")?,
            now
        ],
    )
    .map_err(|_| "notification_history_write_failed")?;
    Ok(())
}

fn deliver(
    history: &mut History,
    version: &str,
    now: i64,
    mut persist: impl FnMut(&History) -> Result<(), String>,
    send: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let next = stable_version(version)?;
    if next <= stable_version(env!("CARGO_PKG_VERSION"))?
        || history
            .delivered
            .as_ref()
            .is_some_and(|v| stable_version(v).is_ok_and(|v| v >= next))
    {
        return Ok(());
    }
    let previous = history.attempts.get(version);
    if previous.is_some_and(|a| a.count >= MAX_ATTEMPTS) {
        return Err(ATTEMPT_LIMIT.into());
    }
    if let Some(previous) = previous.filter(|a| now.saturating_sub(a.at_ms) < RETRY_MS) {
        return Err(previous
            .error
            .clone()
            .unwrap_or_else(|| "notification_retry_pending".into()));
    }
    let count = previous.map_or(1, |a| a.count + 1);
    history.attempts.insert(
        version.into(),
        Attempt {
            count,
            at_ms: now,
            error: None,
        },
    );
    // Persist before touching the OS; a crash must not cause repeated startup notifications.
    persist(history)?;
    let result = send();
    match &result {
        Ok(()) => {
            history.delivered = Some(version.into());
            history
                .attempts
                .retain(|v, _| stable_version(v).is_ok_and(|v| v > next));
        }
        Err(error) => history.attempts.get_mut(version).unwrap().error = Some(error.clone()),
    }
    persist(history)?;
    if result.is_err() && count == MAX_ATTEMPTS {
        Err(ATTEMPT_LIMIT.into())
    } else {
        result
    }
}

#[derive(Default)]
struct Queue {
    pending: Option<String>,
    running: bool,
}

#[derive(Default)]
pub(crate) struct Service {
    queue: Arc<Mutex<Queue>>,
}

impl Service {
    fn enqueue(
        &self,
        version: String,
        mut work: impl FnMut(String) + Send + 'static,
    ) -> Result<(), String> {
        let mut queue = self.queue.lock().unwrap();
        queue.pending = Some(version);
        if queue.running {
            return Ok(());
        }
        queue.running = true;
        let shared = Arc::clone(&self.queue);
        if std::thread::Builder::new()
            .name("update-notification".into())
            .spawn(move || loop {
                let next = {
                    let mut queue = shared.lock().unwrap();
                    match queue.pending.take() {
                        Some(next) => next,
                        None => {
                            queue.running = false;
                            return;
                        }
                    }
                };
                work(next);
            })
            .is_err()
        {
            queue.running = false;
            return Err("notification_worker_failed".into());
        }
        Ok(())
    }

    pub(crate) fn notify_available(
        &self,
        app: tauri::AppHandle,
        state: Arc<AppState>,
        cache: PathBuf,
        version: String,
    ) {
        let worker_app = app.clone();
        let pending_version = version.clone();
        let result = self.enqueue(version, move |version| {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let settings = state.settings.lock().unwrap().clone();
                if !settings.updates.system_notifications {
                    return Ok(());
                }
                let key = history_key(&cache);
                let mut history = load(state.storage.lock().unwrap().conn(), &key)?;
                let now = crate::scanner::now_ms();
                let (title, body) = message(&settings.language, &version);
                deliver(
                    &mut history,
                    &version,
                    now,
                    |history| save(state.storage.lock().unwrap().conn(), &key, history, now),
                    || native_send(&cache, &version, title, &body),
                )
            }))
            .unwrap_or_else(|_| Err("notification_worker_failed".into()));
            crate::updates::notification_result(&worker_app, &version, result);
        });
        if result.is_err() {
            crate::updates::notification_result(&app, &pending_version, result);
        }
    }
}

fn message(locale: &str, version: &str) -> (&'static str, String) {
    let (title, body) = match locale {
        "zh-CN" => (
            "LLM Usage 有可用更新",
            "版本 {version} 已发布。打开 LLM Usage 查看更新。",
        ),
        "zh-TW" => (
            "LLM Usage 有可用更新",
            "版本 {version} 已發佈。開啟 LLM Usage 查看更新。",
        ),
        "ja" => (
            "LLM Usage の更新",
            "バージョン {version} が公開されました。LLM Usage で更新を確認してください。",
        ),
        "ko" => (
            "LLM Usage 업데이트",
            "버전 {version}이 출시되었습니다. LLM Usage에서 업데이트를 확인하세요.",
        ),
        "es" => (
            "Actualización de LLM Usage",
            "La versión {version} está disponible. Abre LLM Usage para ver la actualización.",
        ),
        "fr" => (
            "Mise à jour de LLM Usage",
            "La version {version} est disponible. Ouvrez LLM Usage pour consulter la mise à jour.",
        ),
        "de" => (
            "LLM Usage-Update verfügbar",
            "Version {version} ist verfügbar. Öffnen Sie LLM Usage, um das Update anzusehen.",
        ),
        "pt-BR" => (
            "Atualização do LLM Usage",
            "A versão {version} está disponível. Abra o LLM Usage para ver a atualização.",
        ),
        "ru" => (
            "Обновление LLM Usage",
            "Доступна версия {version}. Откройте LLM Usage, чтобы посмотреть обновление.",
        ),
        _ => (
            "LLM Usage update available",
            "Version {version} is available. Open LLM Usage to view the update.",
        ),
    };
    (title, body.replace("{version}", version))
}

#[cfg(windows)]
fn native_send(cache: &Path, version: &str, title: &str, body: &str) -> Result<(), String> {
    windows::send(cache, version, title, body)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn native_send(_cache: &Path, _version: &str, title: &str, body: &str) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "notification_worker_failed")?;
    runtime.block_on(async {
        #[cfg(target_os = "linux")]
        {
            tokio::time::timeout(
                std::time::Duration::from_secs(5),
                notify_rust::Notification::new()
                    .appname("LLM Usage")
                    .summary(title)
                    .body(body)
                    .show_async(),
            )
            .await
            .map_err(|_| "notification_timeout")?
            .map_err(|_| "notification_service_unavailable")?;
        }
        #[cfg(target_os = "macos")]
        {
            use mac_usernotifications::{AuthorizationStatus, Notification};
            mac_usernotifications::check_bundle().map_err(|_| "notification_bundle_required")?;
            let settings = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                mac_usernotifications::get_notification_settings(),
            )
            .await
            .map_err(|_| "notification_timeout")?
            .map_err(|_| "notification_service_unavailable")?;
            match settings.authorization_status {
                AuthorizationStatus::NotDetermined => {
                    let granted = tokio::time::timeout(
                        std::time::Duration::from_secs(60),
                        mac_usernotifications::request_auth(),
                    )
                    .await
                    .map_err(|_| "notification_timeout")?
                    .map_err(|_| "notification_permission_failed")?;
                    if !granted {
                        return Err("notification_disabled".into());
                    }
                }
                AuthorizationStatus::Authorized
                | AuthorizationStatus::Provisional
                | AuthorizationStatus::Ephemeral => {}
                _ => return Err("notification_disabled".into()),
            }
            tokio::time::timeout(
                std::time::Duration::from_secs(5),
                Notification::new()
                    .title(title)
                    .message(body)
                    .id("llm-usage-update")
                    .send(),
            )
            .await
            .map_err(|_| "notification_timeout")?
            .map_err(|_| "notification_delivery_failed")?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn history_deduplicates_across_reopen_and_rejects_rollback() {
        let path = std::env::temp_dir().join(format!(
            "notification-history-{}.sqlite",
            std::process::id()
        ));
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT,schema_version INTEGER,updated_at_ms INTEGER)").unwrap();
        let key = "update_notification:test";
        let mut history = load(&conn, key).unwrap();
        deliver(
            &mut history,
            "999.1.0",
            1,
            |h| save(&conn, key, h, 1),
            || Ok(()),
        )
        .unwrap();
        drop(conn);
        let conn = Connection::open(&path).unwrap();
        let mut history = load(&conn, key).unwrap();
        for version in ["999.1.0", "999.0.0", env!("CARGO_PKG_VERSION")] {
            deliver(
                &mut history,
                version,
                RETRY_MS * 2,
                |_| panic!("must not write"),
                || panic!("must not notify"),
            )
            .unwrap();
        }
        deliver(
            &mut history,
            "999.2.0",
            2,
            |h| save(&conn, key, h, 2),
            || Ok(()),
        )
        .unwrap();
        assert_eq!(
            load(&conn, key).unwrap().delivered.as_deref(),
            Some("999.2.0")
        );
        conn.execute("UPDATE settings SET value='invalid'", [])
            .unwrap();
        assert!(load(&conn, key).is_err());
        drop(conn);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn failures_retry_after_cooldown_without_resetting_on_clock_rollback() {
        let mut history = History::default();
        assert_eq!(
            deliver(
                &mut history,
                "999.1.0",
                100,
                |_| Ok(()),
                || Err("denied".into())
            ),
            Err("denied".into())
        );
        assert!(history.delivered.is_none());
        assert_eq!(
            deliver(&mut history, "999.1.0", 101, |_| panic!(), || panic!()),
            Err("denied".into())
        );
        deliver(
            &mut history,
            "999.1.0",
            RETRY_MS + 100,
            |_| Ok(()),
            || Ok(()),
        )
        .unwrap();
        assert!(deliver(
            &mut history,
            "999.2.0",
            RETRY_MS + 101,
            |_| Ok(()),
            || Err("denied".into())
        )
        .is_err());
        assert_eq!(
            deliver(&mut history, "999.2.0", 99, |_| panic!(), || panic!()),
            Err("denied".into())
        );
        deliver(
            &mut history,
            "999.2.0",
            RETRY_MS * 2 + 101,
            |_| Ok(()),
            || Ok(()),
        )
        .unwrap();
        deliver(&mut history, "999.3.0", 100, |_| Ok(()), || Ok(())).unwrap();
    }

    #[test]
    fn failed_persistence_and_invalid_versions_never_send() {
        let mut history = History::default();
        assert!(deliver(
            &mut history,
            "999.1.0",
            0,
            |_| Err("disk full".into()),
            || panic!()
        )
        .is_err());
        for version in [
            "invalid",
            "999.1.0-rc1",
            "999.1.0+build",
            "999.1.0\"><script>",
        ] {
            assert!(deliver(
                &mut History::default(),
                version,
                0,
                |_| panic!(),
                || panic!()
            )
            .is_err());
        }
        let mut interrupted = History {
            attempts: BTreeMap::from([(
                "999.1.0".into(),
                Attempt {
                    count: 1,
                    at_ms: 1,
                    error: None,
                },
            )]),
            ..History::default()
        };
        assert!(deliver(&mut interrupted, "999.1.0", 2, |_| panic!(), || panic!()).is_err());
        deliver(
            &mut interrupted,
            "999.1.0",
            RETRY_MS + 1,
            |_| Ok(()),
            || Ok(()),
        )
        .unwrap();
    }

    #[test]
    fn retry_limit_survives_reopen_version_alternation_and_clock_extremes() {
        let path =
            std::env::temp_dir().join(format!("notification-budget-{}.sqlite", std::process::id()));
        let key = "update_notification:budget";
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT,schema_version INTEGER,updated_at_ms INTEGER)").unwrap();
        drop(conn);
        let mut sent = 0;
        for round in 0..MAX_ATTEMPTS {
            for version in ["999.1.0", "999.2.0"] {
                let conn = Connection::open(&path).unwrap();
                let mut history = load(&conn, key).unwrap();
                let now = i64::from(round) * RETRY_MS + 100;
                let result = deliver(
                    &mut history,
                    version,
                    now,
                    |h| save(&conn, key, h, now),
                    || {
                        sent += 1;
                        Err("denied".into())
                    },
                );
                assert_eq!(
                    result,
                    Err(if round + 1 == MAX_ATTEMPTS {
                        ATTEMPT_LIMIT
                    } else {
                        "denied"
                    }
                    .into())
                );
                assert_eq!(load(&conn, key).unwrap().attempts[version].count, round + 1);
                if round + 1 < MAX_ATTEMPTS {
                    assert_eq!(
                        deliver(
                            &mut history,
                            version,
                            now + 1,
                            |_| panic!("must not write"),
                            || panic!("must not send")
                        ),
                        Err("denied".into())
                    );
                }
            }
        }
        assert_eq!(sent, 6);
        let conn = Connection::open(&path).unwrap();
        for now in [i64::MIN, 0, RETRY_MS * 100, i64::MAX] {
            let mut history = load(&conn, key).unwrap();
            for version in ["999.1.0", "999.2.0"] {
                assert_eq!(
                    deliver(
                        &mut history,
                        version,
                        now,
                        |_| panic!("must not write"),
                        || panic!("must not send")
                    ),
                    Err(ATTEMPT_LIMIT.into())
                );
            }
        }
        let mut history = load(&conn, key).unwrap();
        deliver(
            &mut history,
            "999.3.0",
            RETRY_MS * 4,
            |h| save(&conn, key, h, RETRY_MS * 4),
            || {
                sent += 1;
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(sent, 7);
        let mut history = load(&conn, key).unwrap();
        assert!(history.attempts.is_empty());
        for version in ["999.1.0", "999.2.0", "999.3.0"] {
            deliver(&mut history, version, i64::MAX, |_| panic!(), || panic!()).unwrap();
        }
        drop(conn);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn interrupted_or_unrecorded_results_still_consume_a_persisted_attempt() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT,schema_version INTEGER,updated_at_ms INTEGER)").unwrap();
        let key = "update_notification:interrupted";
        let mut sent = 0;
        for round in 0..MAX_ATTEMPTS {
            let now = i64::from(round) * RETRY_MS;
            let mut history = load(&conn, key).unwrap();
            let mut writes = 0;
            assert_eq!(
                deliver(
                    &mut history,
                    "999.1.0",
                    now,
                    |h| {
                        writes += 1;
                        if writes == 1 {
                            save(&conn, key, h, now)
                        } else {
                            Err("disk full".into())
                        }
                    },
                    || {
                        sent += 1;
                        Ok(())
                    }
                ),
                Err("disk full".into())
            );
            let saved = load(&conn, key).unwrap();
            assert_eq!(saved.attempts["999.1.0"].count, round + 1);
            assert!(saved.delivered.is_none());
        }
        assert_eq!(sent, 3);
        let mut history = load(&conn, key).unwrap();
        assert_eq!(
            deliver(&mut history, "999.1.0", i64::MAX, |_| panic!(), || panic!()),
            Err(ATTEMPT_LIMIT.into())
        );
    }

    #[test]
    fn legacy_unknown_retry_history_is_exhausted_and_invalid_budgets_are_rejected() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT,schema_version INTEGER,updated_at_ms INTEGER)").unwrap();
        let key = "update_notification:legacy";
        conn.execute("INSERT INTO settings VALUES(?1,?2,1,0)", rusqlite::params![key,
            r#"{"delivered":"999.0.0","attempt":{"version":"999.1.0","at_ms":1},"error":"denied"}"#]).unwrap();
        let mut history = load(&conn, key).unwrap();
        assert_eq!(history.attempts["999.1.0"].count, MAX_ATTEMPTS);
        assert_eq!(history.attempts["999.1.0"].error.as_deref(), Some("denied"));
        assert_eq!(
            deliver(&mut history, "999.1.0", i64::MAX, |_| panic!(), || panic!()),
            Err(ATTEMPT_LIMIT.into())
        );
        deliver(&mut history, "999.0.0", i64::MAX, |_| panic!(), || panic!()).unwrap();
        assert!(deliver(
            &mut history,
            "999.2.0",
            RETRY_MS,
            |h| save(&conn, key, h, RETRY_MS),
            || Err("denied".into())
        )
        .is_err());
        let history = load(&conn, key).unwrap();
        assert_eq!(history.attempts["999.1.0"].count, MAX_ATTEMPTS);
        assert_eq!(history.attempts["999.2.0"].count, 1);
        for count in [0, 4, 256, -1] {
            let invalid = format!(
                r#"{{"delivered":null,"attempts":{{"999.1.0":{{"count":{count},"at_ms":0,"error":null}}}}}}"#
            );
            conn.execute(
                "UPDATE settings SET value=?1 WHERE key=?2",
                [invalid.as_str(), key],
            )
            .unwrap();
            assert!(load(&conn, key).is_err());
        }
    }

    #[test]
    fn queue_serializes_and_keeps_latest_pending_version() {
        let service = Service::default();
        let (sent, received) = mpsc::channel();
        let (release, wait) = mpsc::channel();
        service
            .enqueue("1".into(), move |version| {
                sent.send(version.clone()).unwrap();
                if version == "1" {
                    wait.recv_timeout(Duration::from_secs(5)).unwrap();
                }
            })
            .unwrap();
        assert_eq!(received.recv_timeout(Duration::from_secs(5)).unwrap(), "1");
        service
            .enqueue("2".into(), |_| panic!("second worker"))
            .unwrap();
        service
            .enqueue("3".into(), |_| panic!("second worker"))
            .unwrap();
        release.send(()).unwrap();
        assert_eq!(received.recv_timeout(Duration::from_secs(5)).unwrap(), "3");
        assert!(received.recv_timeout(Duration::from_secs(1)).is_err());
    }

    #[test]
    fn old_settings_default_to_notifications_and_explicit_opt_out_survives() {
        let settings: crate::updates::UpdateSettings =
            serde_json::from_str(r#"{"schedule":"manual","auto_download":false}"#).unwrap();
        assert!(settings.system_notifications);
        let settings: crate::updates::UpdateSettings =
            serde_json::from_str(r#"{"system_notifications":false}"#).unwrap();
        assert!(!settings.system_notifications);
        for locale in [
            "zh-CN", "zh-TW", "en", "ja", "ko", "es", "fr", "de", "pt-BR", "ru",
        ] {
            let (title, body) = message(locale, "999.1.0");
            assert!(!title.is_empty());
            assert!(body.contains("999.1.0") && !body.contains("{version}"));
        }
    }
}
