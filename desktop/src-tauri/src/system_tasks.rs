//! Persisted consent and native Task Scheduler reconciliation. Status reads never
//! register a task; failures retain the desired state and expose the actual state.
use llm_usage_core::storage::Storage;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::path::Path;

const STATE_KEY: &str = "background_task";
const GLOBAL_KEY: &str = "background_global_deadline";

#[derive(Default, Serialize, Deserialize)]
struct TaskIntent {
    enabled: bool,
    error: Option<String>,
}

fn intent(storage: &Storage) -> Result<TaskIntent, String> {
    let value: Option<String> = storage
        .conn()
        .query_row(
            "SELECT value FROM settings WHERE key=?1",
            [STATE_KEY],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    value
        .map(|v| serde_json::from_str(&v).map_err(|_| "invalid_background_task_state".into()))
        .unwrap_or_else(|| Ok(TaskIntent::default()))
}

#[cfg(any(windows, test))]
fn write_intent(storage: &Storage, value: &TaskIntent) -> Result<(), String> {
    storage
        .conn()
        .execute(
            "INSERT INTO settings(key,value,schema_version,updated_at_ms) VALUES(?1,?2,1,?3)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value,updated_at_ms=excluded.updated_at_ms",
            rusqlite::params![
                STATE_KEY,
                serde_json::to_string(value).map_err(|e| e.to_string())?,
                crate::scanner::now_ms()
            ],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn desired(storage: &Storage) -> Result<bool, String> {
    Ok(intent(storage)?.enabled)
}

pub fn global_due(storage: &Storage, now: i64, interval: u64) -> Result<bool, String> {
    let value: Option<String> = storage
        .conn()
        .query_row(
            "SELECT value FROM settings WHERE key=?1",
            [GLOBAL_KEY],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let previous: Option<(u64, i64)> = value
        .map(|v| serde_json::from_str(&v))
        .transpose()
        .map_err(|_| "invalid_background_deadline")?;
    Ok(interval > 0 && previous.is_none_or(|(seconds, due)| seconds != interval || now >= due))
}

pub fn mark_global_run(storage: &Storage, now: i64, interval: u64) -> Result<(), String> {
    let delay = i64::try_from(interval)
        .unwrap_or(i64::MAX)
        .saturating_mul(1000);
    storage
        .conn()
        .execute(
            "INSERT INTO settings(key,value,schema_version,updated_at_ms) VALUES(?1,?2,1,?3)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value,updated_at_ms=excluded.updated_at_ms",
            rusqlite::params![
                GLOBAL_KEY,
                serde_json::to_string(&(interval, now.saturating_add(delay)))
                    .map_err(|e| e.to_string())?,
                now
            ],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// The losing process uses a plain connection, without recovering the owner's jobs.
pub fn request_if_enabled(path: &Path, now: i64) -> Result<bool, String> {
    let conn =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)
            .map_err(|e| e.to_string())?;
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key=?1",
            [STATE_KEY],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let enabled = value
        .map(|v| serde_json::from_str::<TaskIntent>(&v))
        .transpose()
        .map_err(|_| "invalid_background_task_state")?
        .is_some_and(|v| v.enabled);
    if enabled {
        crate::process_guard::request_background_refresh(path, now).map_err(|e| e.to_string())?;
    }
    Ok(enabled)
}

#[cfg(windows)]
pub mod native {
    use super::*;
    use windows::core::{Interface, BSTR};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_MULTITHREADED,
    };
    use windows::Win32::System::TaskScheduler::*;
    use windows::Win32::System::Variant::VARIANT;

    #[derive(Default)]
    pub struct Actual {
        pub exists: bool,
        pub enabled: bool,
        pub matches: bool,
    }

    fn name(path: &Path) -> String {
        let identity =
            llm_usage_core::identity::content_hash(&path.to_string_lossy().to_lowercase());
        format!(
            "LLMUsageDataRefresh-{}",
            identity.trim_start_matches("fnv1a64:")
        )
    }

    fn canonical_task_path(path: &Path) -> Result<std::path::PathBuf, String> {
        let parent = path
            .parent()
            .ok_or("missing_data_directory")?
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let text = parent.to_string_lossy();
        let directory = if let Some(unc) = text.strip_prefix("\\\\?\\UNC\\") {
            format!("\\\\{unc}")
        } else {
            text.strip_prefix("\\\\?\\").unwrap_or(&text).to_string()
        };
        Ok(Path::new(&directory).join(path.file_name().ok_or("missing_database_name")?))
    }

    fn arguments(path: &Path) -> String {
        format!(
            "--headless --data-dir \"{}\"",
            path.parent().unwrap().display()
        )
    }

    // Each invocation owns its COM apartment, independently of Tauri's threads.
    fn com<T: Send + 'static>(
        f: impl FnOnce(ITaskService, ITaskFolder) -> windows::core::Result<T> + Send + 'static,
    ) -> Result<T, String> {
        std::thread::spawn(move || unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
            struct Apartment;
            impl Drop for Apartment {
                fn drop(&mut self) {
                    unsafe { CoUninitialize() };
                }
            }
            let _apartment = Apartment;
            let service: ITaskService =
                CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)?;
            let empty = VARIANT::default();
            service.Connect(&empty, &empty, &empty, &empty)?;
            let folder = service.GetFolder(&BSTR::from("\\"))?;
            f(service, folder)
        })
        .join()
        .map_err(|_| "task_scheduler_thread_failed".to_string())?
        .map_err(|e| format!("task_scheduler: {}", e.code()))
    }

    unsafe fn find(
        folder: &ITaskFolder,
        path: &Path,
    ) -> windows::core::Result<Option<IRegisteredTask>> {
        match unsafe { folder.GetTask(&BSTR::from(name(path))) } {
            Ok(task) => Ok(Some(task)),
            Err(error) if error.code().0 as u32 == 0x80070002 => Ok(None),
            Err(error) => Err(error),
        }
    }

    unsafe fn owns(task: &IRegisteredTask, path: &Path) -> windows::core::Result<bool> {
        let mut description = BSTR::new();
        unsafe {
            task.Definition()?
                .RegistrationInfo()?
                .Description(&mut description)?;
        }
        Ok(description == format!("llm-usage:{}", name(path)).as_str())
    }

    pub fn status(path: &Path) -> Result<Actual, String> {
        let path = canonical_task_path(path)?;
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        com(move |_, folder| unsafe {
            let Some(task) = find(&folder, &path)? else {
                return Ok(Actual::default());
            };
            let definition = task.Definition()?;
            let actions = definition.Actions()?;
            let mut count = 0;
            actions.Count(&mut count)?;
            let mut logon = TASK_LOGON_NONE;
            let mut run_level = TASK_RUNLEVEL_LUA;
            definition.Principal()?.LogonType(&mut logon)?;
            definition.Principal()?.RunLevel(&mut run_level)?;
            let settings = definition.Settings()?;
            let mut wake = false.into();
            let mut available = false.into();
            let mut policy = TASK_INSTANCES_IGNORE_NEW;
            let mut execution_limit = BSTR::new();
            settings.WakeToRun(&mut wake)?;
            settings.StartWhenAvailable(&mut available)?;
            settings.MultipleInstances(&mut policy)?;
            settings.ExecutionTimeLimit(&mut execution_limit)?;
            let triggers = definition.Triggers()?;
            let mut trigger_count = 0;
            triggers.Count(&mut trigger_count)?;
            let mut cadence = BSTR::new();
            let mut trigger_enabled = false.into();
            if trigger_count == 1 {
                let trigger = triggers.get_Item(1)?;
                trigger.Repetition()?.Interval(&mut cadence)?;
                trigger.Enabled(&mut trigger_enabled)?;
            }
            let matches = if owns(&task, &path)? && count == 1 {
                let action: IExecAction = actions.get_Item(1)?.cast()?;
                let mut executable_path = BSTR::new();
                let mut args = BSTR::new();
                action.Path(&mut executable_path)?;
                action.Arguments(&mut args)?;
                executable_path
                    .to_string()
                    .eq_ignore_ascii_case(&executable.to_string_lossy())
                    && args == arguments(&path).as_str()
                    && logon == TASK_LOGON_INTERACTIVE_TOKEN
                    && run_level == TASK_RUNLEVEL_LUA
                    && !wake.as_bool()
                    && available.as_bool()
                    && policy == TASK_INSTANCES_IGNORE_NEW
                    && execution_limit == "PT5M"
                    && trigger_count == 1
                    && trigger_enabled.as_bool()
                    && cadence == "PT1M"
            } else {
                false
            };
            Ok(Actual {
                exists: true,
                enabled: task.Enabled()?.as_bool(),
                matches,
            })
        })
    }

    pub fn apply(path: &Path, enabled: bool) -> Result<(), String> {
        let path = canonical_task_path(path)?;
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        com(move |service, folder| unsafe {
            if let Some(task) = find(&folder, &path)? {
                if !owns(&task, &path)? {
                    return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                        0x80070005u32 as i32,
                    )));
                }
                if !enabled {
                    folder.DeleteTask(&BSTR::from(name(&path)), 0)?;
                    return Ok(());
                }
            } else if !enabled {
                return Ok(());
            }
            let definition = service.NewTask(0)?;
            definition
                .RegistrationInfo()?
                .SetDescription(&BSTR::from(format!("llm-usage:{}", name(&path))))?;
            let principal = definition.Principal()?;
            principal.SetLogonType(TASK_LOGON_INTERACTIVE_TOKEN)?;
            principal.SetRunLevel(TASK_RUNLEVEL_LUA)?;
            let settings = definition.Settings()?;
            settings.SetEnabled(true.into())?;
            settings.SetWakeToRun(false.into())?;
            settings.SetStartWhenAvailable(true.into())?;
            settings.SetDisallowStartIfOnBatteries(false.into())?;
            settings.SetStopIfGoingOnBatteries(false.into())?;
            settings.SetMultipleInstances(TASK_INSTANCES_IGNORE_NEW)?;
            settings.SetExecutionTimeLimit(&BSTR::from("PT5M"))?;
            let trigger = definition.Triggers()?.Create(TASK_TRIGGER_TIME)?;
            // Explicit UTC boundary avoids ambiguous local time on the registration day.
            let start = jiff::Timestamp::now()
                .checked_add(jiff::Span::new().minutes(1))
                .map_err(|_| {
                    windows::core::Error::from_hresult(windows::core::HRESULT(0x80070057u32 as i32))
                })?;
            trigger.SetStartBoundary(&BSTR::from(start.to_string()))?;
            trigger.Repetition()?.SetInterval(&BSTR::from("PT1M"))?;
            let action: IExecAction = definition.Actions()?.Create(TASK_ACTION_EXEC)?.cast()?;
            action.SetPath(&BSTR::from(executable.to_string_lossy().as_ref()))?;
            action.SetArguments(&BSTR::from(arguments(&path)))?;
            let empty = VARIANT::default();
            folder.RegisterTaskDefinition(
                &BSTR::from(name(&path)),
                &definition,
                TASK_CREATE_OR_UPDATE.0,
                &empty,
                &empty,
                TASK_LOGON_INTERACTIVE_TOKEN,
                &empty,
            )?;
            Ok(())
        })
    }

    /// Enumerate only the task root used by apply. Foreign names, users, executables
    /// or altered arguments remain untouched. Hold database owner locks until done.
    pub fn remove_installation_tasks() -> Result<(), String> {
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let user_sid = crate::installation::current_user_sid()?;
        let candidates = com(move |_, folder| unsafe {
            let collection = folder.GetTasks(TASK_ENUM_HIDDEN.0)?;
            let mut candidates = Vec::new();
            for index in 1..=collection.Count()? {
                let task = collection.get_Item(&VARIANT::from(index))?;
                let name = task.Name()?.to_string();
                if !name.starts_with("LLMUsageDataRefresh-") {
                    continue;
                }
                let definition = task.Definition()?;
                let actions = definition.Actions()?;
                let mut count = 0;
                actions.Count(&mut count)?;
                if count != 1 {
                    continue;
                }
                let Ok(action) = actions.get_Item(1)?.cast::<IExecAction>() else {
                    continue;
                };
                let mut path = BSTR::new();
                let mut args = BSTR::new();
                let mut description = BSTR::new();
                action.Path(&mut path)?;
                action.Arguments(&mut args)?;
                definition
                    .RegistrationInfo()?
                    .Description(&mut description)?;
                let principal = definition.Principal()?;
                let mut sid = BSTR::new();
                let mut logon = TASK_LOGON_NONE;
                let mut level = TASK_RUNLEVEL_LUA;
                principal.UserId(&mut sid)?;
                principal.LogonType(&mut logon)?;
                principal.RunLevel(&mut level)?;
                if logon != TASK_LOGON_INTERACTIVE_TOKEN || level != TASK_RUNLEVEL_LUA {
                    continue;
                }
                if let Some(database) = crate::installation::task_database(
                    &name,
                    &description.to_string(),
                    &path.to_string(),
                    &args.to_string(),
                    &executable.to_string_lossy(),
                ) {
                    if crate::installation::principal_matches_sid(&sid.to_string(), &user_sid)? {
                        candidates.push((name, database, task.Xml()?.to_string()));
                    }
                }
            }
            Ok(candidates)
        })?;
        let mut owners = Vec::new();
        for (_, database, _) in &candidates {
            owners.push(crate::installation::disable_existing_intent(database)?);
        }
        com(move |_, folder| unsafe {
            // Validate every definition again before any deletion; changed ownership
            // must abort uninstall rather than remove a replacement task.
            for (name, _, xml) in &candidates {
                match folder.GetTask(&BSTR::from(name)) {
                    Ok(task) if task.Xml()? == xml.as_str() => {}
                    Err(error) if error.code().0 as u32 == 0x80070002 => {}
                    _ => {
                        return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                            0x80070005u32 as i32,
                        )))
                    }
                }
            }
            for (name, _, xml) in &candidates {
                match folder.GetTask(&BSTR::from(name)) {
                    Ok(task) if task.Xml()? == xml.as_str() => {
                        folder.DeleteTask(&BSTR::from(name), 0)?;
                    }
                    Ok(_) => {
                        return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                            0x80070005u32 as i32,
                        )))
                    }
                    Err(error) if error.code().0 as u32 == 0x80070002 => {}
                    Err(error) => return Err(error),
                }
                match folder.GetTask(&BSTR::from(name)) {
                    Err(error) if error.code().0 as u32 == 0x80070002 => {}
                    Err(error) => return Err(error),
                    Ok(_) => {
                        return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                            0x80070005u32 as i32,
                        )))
                    }
                }
            }
            Ok(())
        })
    }

    #[cfg(test)]
    pub(super) fn change_test_cadence(path: &Path) -> Result<(), String> {
        let path = canonical_task_path(path)?;
        com(move |_, folder| unsafe {
            let task = find(&folder, &path)?.unwrap();
            let definition = task.Definition()?;
            definition
                .Triggers()?
                .get_Item(1)?
                .Repetition()?
                .SetInterval(&BSTR::from("PT1H"))?;
            let empty = VARIANT::default();
            folder.RegisterTaskDefinition(
                &BSTR::from(name(&path)),
                &definition,
                TASK_CREATE_OR_UPDATE.0,
                &empty,
                &empty,
                TASK_LOGON_INTERACTIVE_TOKEN,
                &empty,
            )?;
            Ok(())
        })
    }
}

#[cfg(windows)]
pub fn set_enabled(
    state: &std::sync::Arc<crate::app_state::AppState>,
    enabled: bool,
) -> Result<(), String> {
    set_enabled_with(state, enabled, native::apply, native::status)
}

#[cfg(windows)]
fn set_enabled_with(
    state: &std::sync::Arc<crate::app_state::AppState>,
    enabled: bool,
    apply: impl FnOnce(&Path, bool) -> Result<(), String>,
    inspect: impl FnOnce(&Path) -> Result<native::Actual, String>,
) -> Result<(), String> {
    static MUTATION: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _mutation = MUTATION.lock().map_err(|_| "task_state_busy")?;
    {
        let storage = state.storage.lock().unwrap();
        // Commit disabling before calling the OS: an undeleted trigger must not scan.
        write_intent(
            &storage,
            &TaskIntent {
                enabled,
                error: None,
            },
        )?;
    }
    let result = apply(&state.db_path, enabled).and_then(|_| {
        let actual = inspect(&state.db_path)?;
        if (enabled && actual.enabled && actual.matches) || (!enabled && !actual.exists) {
            Ok(())
        } else {
            Err("background_task_not_applied".into())
        }
    });
    let storage = state.storage.lock().unwrap();
    write_intent(
        &storage,
        &TaskIntent {
            enabled,
            error: result.as_ref().err().cloned(),
        },
    )?;
    result
}

#[cfg(windows)]
pub fn status(
    state: &std::sync::Arc<crate::app_state::AppState>,
) -> Result<serde_json::Value, String> {
    let wanted = { intent(&state.storage.lock().unwrap())? };
    let actual = native::status(&state.db_path);
    let mut result = serde_json::json!({"refresh_task_desired": wanted.enabled, "refresh_task_interval": "minute"});
    match actual {
        Ok(actual) => {
            let applied = actual.enabled && actual.matches;
            result["refresh_task"] = applied.into();
            result["refresh_task_exists"] = actual.exists.into();
            result["refresh_task_error"] =
                if wanted.enabled != applied || (!wanted.enabled && actual.exists) {
                    serde_json::json!(wanted
                        .error
                        .unwrap_or_else(|| "background_task_not_applied".into()))
                } else {
                    serde_json::Value::Null
                };
        }
        Err(error) => {
            result["refresh_task"] = false.into();
            result["refresh_task_error"] = error.into();
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn storage(tag: &str) -> Storage {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/plan-completion/tasks");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join(format!(
            "{}-{tag}-{}.sqlite",
            std::process::id(),
            crate::scanner::now_ms()
        ));
        Storage::open(&path).unwrap()
    }

    #[test]
    fn consent_defaults_closed_and_survives_failed_disable() {
        let storage = storage("intent");
        assert!(!desired(&storage).unwrap());
        write_intent(
            &storage,
            &TaskIntent {
                enabled: true,
                error: Some("registration_failed".into()),
            },
        )
        .unwrap();
        assert!(desired(&storage).unwrap());
        write_intent(
            &storage,
            &TaskIntent {
                enabled: false,
                error: Some("delete_failed".into()),
            },
        )
        .unwrap();
        assert!(!desired(&storage).unwrap());
        assert_eq!(
            intent(&storage).unwrap().error.as_deref(),
            Some("delete_failed")
        );
    }

    #[test]
    fn background_deadline_merges_missed_ticks_and_handles_rule_changes() {
        let storage = storage("deadline");
        assert!(!global_due(&storage, 0, 0).unwrap());
        assert!(global_due(&storage, 0, 60).unwrap());
        mark_global_run(&storage, 0, 60).unwrap();
        assert!(!global_due(&storage, 59999, 60).unwrap());
        assert!(global_due(&storage, 60000, 60).unwrap());
        mark_global_run(&storage, 600000, 60).unwrap();
        assert!(!global_due(&storage, 600001, 60).unwrap());
        assert!(!global_due(&storage, 1, 60).unwrap());
        assert!(global_due(&storage, 1, 120).unwrap());
    }

    #[test]
    fn leftover_system_trigger_does_not_queue_after_disable() {
        let storage = storage("queue");
        let path = storage.path();
        assert!(!request_if_enabled(path, 1).unwrap());
        assert!(!crate::process_guard::take_background_refresh_request(&storage).unwrap());
        write_intent(
            &storage,
            &TaskIntent {
                enabled: true,
                error: None,
            },
        )
        .unwrap();
        assert!(request_if_enabled(path, 2).unwrap());
        request_if_enabled(path, 3).unwrap();
        assert!(crate::process_guard::take_background_refresh_request(&storage).unwrap());
        assert!(!crate::process_guard::take_background_refresh_request(&storage).unwrap());
        write_intent(&storage, &TaskIntent::default()).unwrap();
        assert!(!request_if_enabled(path, 4).unwrap());
        assert!(!crate::process_guard::take_background_refresh_request(&storage).unwrap());
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "registers and deletes an isolated current-user task; run explicitly for V24"]
    fn native_task_roundtrip() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/plan-completion/tasks/含空格 task");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("llm-usage.sqlite");
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = native::apply(&self.0, false);
            }
        }
        let _cleanup = Cleanup(path.clone());
        assert!(!native::status(&path).unwrap().exists);
        native::apply(&path, true).unwrap();
        let actual = native::status(&path).unwrap();
        assert!(actual.enabled && actual.matches);
        assert!(
            native::status(
                &path
                    .parent()
                    .unwrap()
                    .canonicalize()
                    .unwrap()
                    .join("llm-usage.sqlite")
            )
            .unwrap()
            .matches
        );
        native::change_test_cadence(&path).unwrap();
        assert!(
            !native::status(&path).unwrap().matches,
            "existence does not prove the minute cadence is applied"
        );
        native::apply(&path, true).unwrap();
        assert!(native::status(&path).unwrap().matches);
        native::apply(&path, false).unwrap();
        assert!(!native::status(&path).unwrap().exists);
        native::apply(&path, false).unwrap();
    }
}
#[cfg(windows)]
#[test]
fn failed_registration_and_deletion_persist_intent_before_os_mutation() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/plan-finalization/task-failures");
    std::fs::create_dir_all(&root).unwrap();
    let state = std::sync::Arc::new(
        crate::app_state::AppState::init(
            root.join(format!(
                "{}-{}.sqlite",
                std::process::id(),
                crate::scanner::now_ms()
            )),
            "test",
            false,
        )
        .unwrap(),
    );
    let result = set_enabled_with(
        &state,
        true,
        |path, enabled| {
            assert!(enabled);
            assert!(desired(&Storage::open_readonly(path).unwrap()).unwrap());
            Err("registration_denied".into())
        },
        |_| panic!("failed registration must not claim verification"),
    );
    assert_eq!(result.unwrap_err(), "registration_denied");
    assert_eq!(
        intent(&state.storage.lock().unwrap())
            .unwrap()
            .error
            .as_deref(),
        Some("registration_denied")
    );
    set_enabled_with(
        &state,
        true,
        |_, _| Ok(()),
        |_| {
            Ok(native::Actual {
                exists: true,
                enabled: true,
                matches: true,
            })
        },
    )
    .unwrap();
    let result = set_enabled_with(
        &state,
        false,
        |path, enabled| {
            assert!(!enabled);
            assert!(!desired(&Storage::open_readonly(path).unwrap()).unwrap());
            Err("deletion_denied".into())
        },
        |_| panic!("failed deletion must not claim removal"),
    );
    assert_eq!(result.unwrap_err(), "deletion_denied");
    assert_eq!(
        intent(&state.storage.lock().unwrap())
            .unwrap()
            .error
            .as_deref(),
        Some("deletion_denied")
    );
    assert!(!request_if_enabled(&state.db_path, 1).unwrap());
    assert!(!crate::scanner::run_background_refresh(&state).unwrap());
    set_enabled_with(
        &state,
        false,
        |_, _| Ok(()),
        |_| Ok(native::Actual::default()),
    )
    .unwrap();
    assert!(intent(&state.storage.lock().unwrap())
        .unwrap()
        .error
        .is_none());
}
