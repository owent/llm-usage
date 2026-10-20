use sha2::{Digest, Sha256};
use std::path::Path;
use windows::core::{HSTRING, PCWSTR};
use windows::Data::Xml::Dom::XmlDocument;
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows::Win32::System::Registry::*;
use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};
use windows::UI::Notifications::{
    NotificationSetting, ToastNotification, ToastNotificationManager,
};

const OWNER: &str = "LLMUsageExecutable";
const ACTIVATOR: &str = "{98E4068B-DBF8-42AD-9CC9-DA0CD5790B91}";
const TAG: &str = "available";
const GROUP: &str = "updates";

struct Apartment;
impl Apartment {
    fn new() -> Result<Self, String> {
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }
            .map_err(|_| "notification_runtime_failed")?;
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { RoUninitialize() };
    }
}

struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

fn identity() -> Result<(String, String), String> {
    let exe = std::env::current_exe().map_err(|_| "notification_identity_failed")?;
    let exe = exe
        .to_str()
        .ok_or("notification_identity_failed")?
        .trim_start_matches(r"\\?\")
        .replace('/', "\\")
        .to_lowercase();
    let hash: String = Sha256::digest(exe.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok((format!("com.llmusage.updates.{}", &hash[..32]), exe))
}

fn subkey(id: &str) -> HSTRING {
    HSTRING::from(format!(r"Software\Classes\AppUserModelId\{id}"))
}

fn value(id: &str, name: &str) -> Result<Option<String>, String> {
    let mut size = 0;
    unsafe {
        let status = RegGetValueW(
            HKEY_CURRENT_USER,
            &subkey(id),
            &HSTRING::from(name),
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut size),
        );
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        status
            .ok()
            .map_err(|_| "notification_registration_read_failed")?;
        if size > 65536 {
            return Err("notification_registration_invalid".into());
        }
        let mut data = vec![0u16; (size as usize).div_ceil(2).max(1)];
        RegGetValueW(
            HKEY_CURRENT_USER,
            &subkey(id),
            &HSTRING::from(name),
            RRF_RT_REG_SZ,
            None,
            Some(data.as_mut_ptr().cast()),
            Some(&mut size),
        )
        .ok()
        .map_err(|_| "notification_registration_read_failed")?;
        let end = data.iter().position(|v| *v == 0).unwrap_or(data.len());
        String::from_utf16(&data[..end])
            .map(Some)
            .map_err(|_| "notification_registration_invalid".into())
    }
}

fn register(id: &str, owner: &str, cache: &Path) -> Result<(), String> {
    std::fs::create_dir_all(cache).map_err(|_| "notification_icon_failed")?;
    let icon = cache.join("notification-icon.png");
    let icon_bytes = include_bytes!("../../icons/128x128.png");
    if std::fs::read(&icon).ok().as_deref() != Some(icon_bytes.as_slice()) {
        std::fs::write(&icon, icon_bytes).map_err(|_| "notification_icon_failed")?;
    }
    let mut key = HKEY::default();
    let mut disposition = REG_CREATE_KEY_DISPOSITION::default();
    unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            &subkey(id),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            Some(&mut disposition),
        )
        .ok()
        .map_err(|_| "notification_registration_failed")?;
    }
    let key = Key(key);
    if disposition != REG_CREATED_NEW_KEY && value(id, OWNER)?.as_deref() != Some(owner) {
        return Err("notification_identity_conflict".into());
    }
    // Protocol activation uses a stub CLSID; it never launches an installer or a COM server.
    for (name, value) in [
        (OWNER, owner),
        ("DisplayName", "LLM Usage"),
        ("IconUri", icon.to_str().ok_or("notification_icon_failed")?),
        ("CustomActivator", ACTIVATOR),
    ] {
        let bytes: Vec<u8> = value
            .encode_utf16()
            .chain(Some(0))
            .flat_map(u16::to_le_bytes)
            .collect();
        unsafe { RegSetValueExW(key.0, &HSTRING::from(name), None, REG_SZ, Some(&bytes)) }
            .ok()
            .map_err(|_| "notification_registration_failed")?;
    }
    Ok(())
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn show(id: &str, version: &str, title: &str, body: &str) -> Result<(), String> {
    crate::updates::stable_version(version)?;
    let result = (|| -> windows::core::Result<()> {
        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(id))?;
        // An identity-less app's first Show registers it with the notification service.
        // Setting returns ERROR_NOT_FOUND before that call; other errors remain failures.
        match notifier.Setting() {
            Ok(NotificationSetting::Enabled) => {}
            Err(e) if e.code().0 == 0x80070490u32 as i32 => {}
            Err(e) => return Err(e),
            _ => {
                return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                    0x80070005u32 as i32,
                )))
            }
        }
        let document = XmlDocument::new()?;
        document.LoadXml(&HSTRING::from(format!(
            "<toast activationType=\"protocol\" launch=\"https://github.com/owent/llm-usage/releases/tag/v{}\"><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>",
            escape(version), escape(title), escape(body))))?;
        let notification = ToastNotification::CreateToastNotification(&document)?;
        notification.SetTag(&HSTRING::from(TAG))?;
        notification.SetGroup(&HSTRING::from(GROUP))?;
        notifier.Show(&notification)?;
        if notifier.Setting()? != NotificationSetting::Enabled {
            return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                0x80070005u32 as i32,
            )));
        }
        Ok(())
    })();
    result.map_err(|e| {
        format!(
            "notification_delivery_failed_or_disabled:{:08x}",
            e.code().0
        )
    })
}

pub(super) fn send(cache: &Path, version: &str, title: &str, body: &str) -> Result<(), String> {
    let _apartment = Apartment::new()?;
    let (id, owner) = identity()?;
    register(&id, &owner, cache)?;
    show(&id, version, title, body)
}

fn remove(id: &str, owner: &str) -> Result<(), String> {
    if value(id, OWNER)?.as_deref() != Some(owner) {
        return Ok(());
    }
    // Delete only known values; an unrelated value or child key preserves the registration.
    for name in ["DisplayName", "IconUri", "CustomActivator", OWNER] {
        let matches = match name {
            "DisplayName" => value(id, name)?.as_deref() == Some("LLM Usage"),
            "CustomActivator" => value(id, name)?.as_deref() == Some(ACTIVATOR),
            "IconUri" => value(id, name)?.is_some_and(|v| {
                Path::new(&v)
                    .file_name()
                    .is_some_and(|f| f == "notification-icon.png")
            }),
            _ => true,
        };
        if matches {
            unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, &subkey(id), &HSTRING::from(name)) }
                .ok()
                .map_err(|_| "notification_cleanup_failed")?;
        }
    }
    let mut key = HKEY::default();
    unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            &subkey(id),
            None,
            KEY_QUERY_VALUE,
            &mut key,
        )
        .ok()
        .map_err(|_| "notification_cleanup_failed")?;
    }
    let key = Key(key);
    let (mut children, mut values) = (0, 0);
    unsafe {
        RegQueryInfoKeyW(
            key.0,
            None,
            None,
            None,
            Some(&mut children),
            None,
            None,
            Some(&mut values),
            None,
            None,
            None,
            None,
        )
        .ok()
        .map_err(|_| "notification_cleanup_failed")?;
    }
    drop(key);
    if children == 0 && values == 0 {
        let status = unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, &subkey(id)) };
        if status != ERROR_SUCCESS && status != ERROR_FILE_NOT_FOUND {
            return Err("notification_cleanup_failed".into());
        }
    }
    Ok(())
}

pub(crate) fn cleanup() -> Result<(), String> {
    let (id, owner) = identity()?;
    remove(&id, &owner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_text_is_escaped() {
        assert_eq!(escape("<&>\"'"), "&lt;&amp;&gt;&quot;&apos;");
    }

    #[test]
    #[ignore = "native Windows notification center; creates and removes an isolated test registration"]
    fn native_notification_center_roundtrip() {
        let _apartment = Apartment::new().unwrap();
        let (base, owner) = identity().unwrap();
        let id = format!("{base}.test.{}", std::process::id());
        let cache =
            std::env::temp_dir().join(format!("notification-native-{}", std::process::id()));
        struct Cleanup(String, String, std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let history = ToastNotificationManager::History().unwrap();
                history
                    .RemoveGroupedTagWithId(
                        &HSTRING::from(TAG),
                        &HSTRING::from(GROUP),
                        &HSTRING::from(&self.0),
                    )
                    .unwrap();
                remove(&self.0, &self.1).unwrap();
                let _ = std::fs::remove_file(self.2.join("notification-icon.png"));
                let _ = std::fs::remove_dir(&self.2);
            }
        }
        assert!(value(&id, OWNER).unwrap().is_none());
        let _cleanup = Cleanup(id.clone(), owner.clone(), cache.clone());
        register(&id, &owner, &cache).unwrap();
        eprintln!(
            "native notification setting: {:?}",
            ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(&id))
                .unwrap()
                .Setting()
        );
        assert!(register(&id, "foreign-owner", &cache).is_err());
        remove(&id, "foreign-owner").unwrap();
        assert_eq!(value(&id, OWNER).unwrap().as_deref(), Some(owner.as_str()));
        show(
            &id,
            "999.1.0",
            "LLM Usage 通知测试 / notification test",
            "本地验证通知，测试后自动清理。Local verification; removed after the test.",
        )
        .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let history = ToastNotificationManager::History()
                .unwrap()
                .GetHistoryWithId(&HSTRING::from(&id))
                .unwrap();
            let found = history.into_iter().any(|item| {
                item.Tag().unwrap() == TAG
                    && item
                        .Content()
                        .unwrap()
                        .GetXml()
                        .unwrap()
                        .to_string_lossy()
                        .contains("notification test")
            });
            if found {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "notification was not present in the native notification center"
            );
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        drop(_cleanup);
        assert!(value(&id, OWNER).unwrap().is_none());
    }
}
