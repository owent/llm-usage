//! Standalone uninstall removes only integration owned by this executable.
//! Do not initialize Storage, recover jobs, migrate databases or discover sources.
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};

pub(crate) fn same_executable(left: &str, right: &str) -> bool {
    fn normalize(value: &str) -> String {
        let value = value.trim_matches('"');
        if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{unc}")
        } else {
            value.strip_prefix(r"\\?\").unwrap_or(value).to_string()
        }
        .replace('/', "\\")
    }
    normalize(left).eq_ignore_ascii_case(&normalize(right))
}

/// system_tasks::native::apply produces the accepted argument syntax.
/// A prefix/name alone never establishes Task Scheduler entry ownership.
pub(crate) fn task_database(
    name: &str,
    description: &str,
    executable: &str,
    arguments: &str,
    current_executable: &str,
) -> Option<PathBuf> {
    if !same_executable(executable, current_executable) {
        return None;
    }
    let directory = arguments
        .strip_prefix("--headless --data-dir \"")?
        .strip_suffix('"')?;
    if directory.contains('"') || !Path::new(directory).is_absolute() {
        return None;
    }
    let database = Path::new(directory).join("llm-usage.sqlite");
    let identity =
        llm_usage_core::identity::content_hash(&database.to_string_lossy().to_lowercase());
    let expected = format!(
        "LLMUsageDataRefresh-{}",
        identity.trim_start_matches("fnv1a64:")
    );
    (name == expected && description == format!("llm-usage:{expected}")).then_some(database)
}

/// Plain SQLite opens an existing database without changing even a newer schema.
/// The returned owner lock prevents another application from re-enabling consent.
pub(crate) fn disable_existing_intent(path: &Path) -> Result<Option<std::fs::File>, String> {
    if !path.try_exists().map_err(|e| e.to_string())? {
        return Ok(None);
    }
    let owner = crate::process_guard::acquire(path)
        .map_err(|e| e.to_string())?
        .ok_or("uninstall_application_running")?;
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|e| e.to_string())?;
    connection
        .busy_timeout(std::time::Duration::from_secs(3))
        .map_err(|e| e.to_string())?;
    let has_settings: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='settings')",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if has_settings {
        connection
            .execute(
                "UPDATE settings SET value=?1,updated_at_ms=?2 WHERE key='background_task'",
                rusqlite::params![
                    r#"{"enabled":false,"error":null}"#,
                    crate::scanner::now_ms()
                ],
            )
            .map_err(|e| e.to_string())?;
        connection
            .execute(
                "DELETE FROM settings WHERE key='pending_background_refresh'",
                [],
            )
            .map_err(|e| e.to_string())?;
    }
    Ok(Some(owner))
}

pub(crate) fn current_user_sid() -> Result<String, String> {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL};
    use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
            .map_err(|e| e.to_string())?;
        struct Token(HANDLE);
        impl Drop for Token {
            fn drop(&mut self) {
                unsafe {
                    let _ = CloseHandle(self.0);
                }
            }
        }
        let token = Token(token);
        let mut size = 0;
        let _ = GetTokenInformation(token.0, TokenUser, None, 0, &mut size);
        if size == 0 || size > 65536 {
            return Err("invalid_user_token".into());
        }
        let mut buffer = vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
        GetTokenInformation(
            token.0,
            TokenUser,
            Some(buffer.as_mut_ptr().cast()),
            size,
            &mut size,
        )
        .map_err(|e| e.to_string())?;
        let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
        let mut text = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut text).map_err(|e| e.to_string())?;
        let result = text.to_string().map_err(|e| e.to_string());
        let _ = LocalFree(Some(HLOCAL(text.0.cast())));
        result
    }
}

/// Task Scheduler returns an account name from Principal.UserId even with a SID in XML.
/// Resolve that native identity before comparing current ownership.
pub(crate) fn principal_matches_sid(account: &str, expected: &str) -> windows::core::Result<bool> {
    use windows::core::{HSTRING, PWSTR};
    use windows::Win32::Foundation::{LocalFree, ERROR_NONE_MAPPED, HLOCAL};
    use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows::Win32::Security::{LookupAccountNameW, SidTypeUnknown, PSID};
    if account == expected {
        return Ok(true);
    }
    unsafe {
        let account = HSTRING::from(account);
        let mut sid_size = 0;
        let mut domain_size = 0;
        let mut kind = SidTypeUnknown;
        let first = LookupAccountNameW(
            None,
            &account,
            None,
            &mut sid_size,
            None,
            &mut domain_size,
            &mut kind,
        );
        if let Err(error) = first {
            if error.code() == ERROR_NONE_MAPPED.to_hresult() {
                return Ok(false);
            }
            if sid_size == 0 {
                return Err(error);
            }
        }
        if sid_size > 65536 || domain_size > 65536 || sid_size == 0 {
            return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                0x80070057u32 as i32,
            )));
        }
        let mut sid = vec![0usize; (sid_size as usize).div_ceil(std::mem::size_of::<usize>())];
        let mut domain = vec![0u16; domain_size as usize];
        let sid = PSID(sid.as_mut_ptr().cast());
        LookupAccountNameW(
            None,
            &account,
            Some(sid),
            &mut sid_size,
            Some(PWSTR(domain.as_mut_ptr())),
            &mut domain_size,
            &mut kind,
        )?;
        let mut text = PWSTR::null();
        ConvertSidToStringSidW(sid, &mut text)?;
        let resolved = text.to_string();
        let _ = LocalFree(Some(HLOCAL(text.0.cast())));
        Ok(resolved? == expected)
    }
}

pub fn cleanup() -> Result<(), String> {
    crate::system_tasks::native::remove_installation_tasks()?;
    crate::commands::remove_owned_auto_start()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ownership_requires_full_identity_and_exact_argument_grammar() {
        let executable = r"C:\含空格 install\LLMUsage.exe";
        let database = Path::new(r"C:\data\llm-usage.sqlite");
        let identity =
            llm_usage_core::identity::content_hash(&database.to_string_lossy().to_lowercase());
        let name = format!(
            "LLMUsageDataRefresh-{}",
            identity.trim_start_matches("fnv1a64:")
        );
        let description = format!("llm-usage:{name}");
        let arguments = r#"--headless --data-dir "C:\data""#;
        assert_eq!(
            task_database(&name, &description, executable, arguments, executable).as_deref(),
            Some(database)
        );
        for (name, description, exe, args) in [
            (
                "LLMUsageDataRefresh-foreign",
                description.as_str(),
                executable,
                arguments,
            ),
            (name.as_str(), "foreign", executable, arguments),
            (
                name.as_str(),
                description.as_str(),
                r"C:\other\LLMUsage.exe",
                arguments,
            ),
            (
                name.as_str(),
                description.as_str(),
                executable,
                r#"--headless --data-dir "C:\data" --scan-once"#,
            ),
            (
                name.as_str(),
                description.as_str(),
                executable,
                r#"--headless --data-dir "relative""#,
            ),
        ] {
            assert!(task_database(name, description, exe, args, executable).is_none());
        }
        assert!(same_executable(
            r"\\?\C:\含空格 install\LLMUsage.exe",
            executable
        ));
    }

    #[test]
    fn cleanup_preserves_future_schema_and_missing_database_and_refuses_running_owner() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/install-lifecycle/unit")
            .join(format!(
                "{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(root.parent().unwrap()).unwrap();
        std::fs::create_dir(&root).unwrap();
        let path = root.join("llm-usage.sqlite");
        assert!(disable_existing_intent(&path).unwrap().is_none());
        assert!(!path.exists());
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch("PRAGMA user_version=999; CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT,updated_at_ms INTEGER); INSERT INTO settings VALUES('background_task','{\"enabled\":true}',0),('pending_background_refresh','1',0),('foreign','keep',0);").unwrap();
        let owner = crate::process_guard::acquire(&path).unwrap().unwrap();
        assert!(disable_existing_intent(&path).is_err());
        drop(owner);
        let owner = disable_existing_intent(&path).unwrap().unwrap();
        assert_eq!(
            connection
                .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            999
        );
        assert_eq!(
            connection
                .query_row("SELECT value FROM settings WHERE key='foreign'", [], |r| {
                    r.get::<_, String>(0)
                })
                .unwrap(),
            "keep"
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM settings WHERE key='pending_background_refresh'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        assert!(connection
            .query_row(
                "SELECT value FROM settings WHERE key='background_task'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap()
            .contains("false"));
        drop(owner);
        drop(connection);
        std::fs::remove_dir_all(root).unwrap();
    }
}
