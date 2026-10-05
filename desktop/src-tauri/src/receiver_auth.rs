//! Source-scoped credentials. No secrets in DTOs, SQLite, logs or ordinary state files.
use serde::{Deserialize, Serialize};
use std::path::Path;

const PREFIX: &str = "llm-usage/otel/v1/";
pub const PLACEHOLDER: &str = "<generated-on-apply>";

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Claude,
    Codex,
    CodeBuddy,
    #[cfg(test)]
    Synthetic,
}
impl Family {
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "claude" => Some(Self::Claude),
            "codex" => Some(Self::Codex),
            "codebuddy" => Some(Self::CodeBuddy),
            _ => None,
        }
    }
    pub fn allows(self, path: &str) -> bool {
        match self {
            Self::Claude | Self::Codex => path == "/v1/logs",
            Self::CodeBuddy => path == "/v1/traces/supplemental",
            #[cfg(test)]
            Self::Synthetic => {
                matches!(path, "/v1/logs" | "/v1/traces" | "/v1/traces/supplemental")
            }
        }
    }
    pub fn accepts_record(self, record: &serde_json::Value) -> bool {
        let name = record["name"].as_str().unwrap_or_default();
        match self {
            Self::Claude => name.starts_with("claude_code."),
            Self::Codex => name.starts_with("codex."),
            // Supplemental spans stay quarantined. User-configurable service.name
            // and span naming do not prove origin; the credential binds the source.
            Self::CodeBuddy => true,
            #[cfg(test)]
            Self::Synthetic => true,
        }
    }
}

// Intentionally no Debug: error reporting must never format a credential.
#[derive(Clone, Serialize, Deserialize)]
pub struct Binding {
    version: u8,
    id: String,
    secret: String,
    app: String,
    config: String,
    family: Family,
}
impl Binding {
    pub fn header(&self) -> String {
        format!("Bearer {}.{}", self.id, self.secret)
    }
}

pub trait Store: Send + Sync {
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, String>;
    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), String>;
    fn delete(&self, name: &str) -> Result<(), String>;
    fn random(&self, bytes: &mut [u8]) -> Result<(), String>;
}

pub struct SystemStore;
pub fn available() -> bool {
    // Read-only capability probe. Missing entries are distinct from an unavailable vault.
    SystemStore
        .read("llm-usage/otel/v1/capability-check")
        .is_ok()
}
fn app_identity(app: &Path) -> Result<String, String> {
    app.canonicalize()
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|_| "credential_store_unavailable".into())
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn valid_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn split_header(header: &str) -> Option<(&str, &str)> {
    let (scheme, value) = header.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("Bearer") {
        return None;
    }
    let (id, secret) = value.split_once('.')?;
    (valid_hex(id, 32) && valid_hex(secret, 64)).then_some((id, secret))
}
fn same_secret(a: &str, b: &str) -> bool {
    // Inputs are fixed-size, validated hexadecimal strings. Do not stop at the first mismatch.
    a.len() == 64
        && b.len() == 64
        && a.bytes().zip(b.bytes()).fold(0u8, |v, (x, y)| v | (x ^ y)) == 0
}

pub fn issue(
    store: &dyn Store,
    app: &Path,
    config: &Path,
    family: Family,
) -> Result<Binding, String> {
    let mut random = [0; 48];
    store.random(&mut random)?;
    let binding = Binding {
        version: 1,
        id: hex(&random[..16]),
        secret: hex(&random[16..]),
        app: app_identity(app)?,
        config: config.to_string_lossy().into_owned(),
        family,
    };
    let name = format!("{PREFIX}{}", binding.id);
    if store.read(&name)?.is_some() {
        return Err("credential_store_unavailable".into());
    }
    let bytes = serde_json::to_vec(&binding).map_err(|_| "credential_store_unavailable")?;
    if let Err(error) = store.write(&name, &bytes) {
        // A service can persist a write before its reply fails. Reclaim only
        // the exact entry we attempted, never a replaced or ambiguous value.
        if store.read(&name)?.as_deref() == Some(bytes.as_slice()) {
            store.delete(&name)?;
        }
        return Err(error);
    }
    // Verify persistence before any exporter is configured. Roll back our entry on failure.
    match store.read(&name) {
        Ok(Some(saved)) if saved == bytes => Ok(binding),
        _ => {
            // A failed verification may mean another writer replaced this
            // target. Re-read and only reclaim our exact payload.
            if store.read(&name)?.as_deref() == Some(bytes.as_slice()) {
                store.delete(&name)?;
            }
            Err("credential_store_unavailable".into())
        }
    }
}
pub fn authenticated(store: &dyn Store, app: &Path, header: &str) -> Option<Binding> {
    let (id, secret) = split_header(header)?;
    let bytes = store.read(&format!("{PREFIX}{id}")).ok()??;
    let binding: Binding = serde_json::from_slice(&bytes).ok()?;
    (binding.version == 1
        && binding.id == id
        && valid_hex(&binding.secret, 64)
        && binding.app == app_identity(app).ok()?
        && same_secret(secret, &binding.secret))
    .then_some(binding)
}
pub fn configured(app: &Path, config: &Path, family: Family, header: &str) -> bool {
    authenticated(&SystemStore, app, header)
        .is_some_and(|b| b.config == config.to_string_lossy() && b.family == family)
}
pub fn authorize(store: &dyn Store, app: &Path, header: &str, path: &str) -> Option<Family> {
    let binding = authenticated(store, app, header)?;
    binding.family.allows(path).then_some(binding.family)
}
pub fn revoke(store: &dyn Store, binding: &Binding) -> Result<(), String> {
    let name = format!("{PREFIX}{}", binding.id);
    // Only remove exactly our credential, even if an external writer changed this target.
    if let Some(bytes) = store.read(&name)? {
        let expected = serde_json::to_vec(binding).map_err(|_| "credential_store_unavailable")?;
        if bytes != expected {
            return Err("credential_store_unavailable".into());
        }
        store.delete(&name)?;
    }
    Ok(())
}

#[cfg(windows)]
impl Store for SystemStore {
    fn random(&self, bytes: &mut [u8]) -> Result<(), String> {
        use windows::Win32::Security::Cryptography::{
            BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        };
        unsafe { BCryptGenRandom(None, bytes, BCRYPT_USE_SYSTEM_PREFERRED_RNG) }
            .ok()
            .map_err(|_| "credential_store_unavailable".into())
    }
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        use windows::Win32::Foundation::ERROR_NOT_FOUND;
        use windows::Win32::Security::Credentials::*;
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let mut ptr = std::ptr::null_mut();
        match unsafe {
            CredReadW(
                windows::core::PCWSTR(wide.as_ptr()),
                CRED_TYPE_GENERIC,
                None,
                &mut ptr,
            )
        } {
            Err(e) if e.code() == windows::core::HRESULT::from_win32(ERROR_NOT_FOUND.0) => {
                return Ok(None)
            }
            Err(_) => return Err("credential_store_unavailable".into()),
            Ok(()) => {}
        }
        // CredRead returns one allocation; free it even when the payload is invalid.
        struct Owned(*mut CREDENTIALW);
        impl Drop for Owned {
            fn drop(&mut self) {
                unsafe { CredFree(self.0.cast()) }
            }
        }
        if ptr.is_null() {
            return Err("credential_store_unavailable".into());
        }
        let owned = Owned(ptr);
        let credential = unsafe { &*owned.0 };
        if credential.CredentialBlobSize == 0
            || credential.CredentialBlobSize > CRED_MAX_CREDENTIAL_BLOB_SIZE
            || credential.CredentialBlob.is_null()
        {
            return Err("credential_store_unavailable".into());
        }
        Ok(Some(
            unsafe {
                std::slice::from_raw_parts(
                    credential.CredentialBlob,
                    credential.CredentialBlobSize as usize,
                )
            }
            .to_vec(),
        ))
    }
    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        use windows::Win32::Security::Credentials::*;
        if bytes.len() > CRED_MAX_CREDENTIAL_BLOB_SIZE as usize {
            return Err("credential_store_unavailable".into());
        }
        let mut wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: windows::core::PWSTR(wide.as_mut_ptr()),
            CredentialBlobSize: bytes.len() as u32,
            CredentialBlob: bytes.as_ptr().cast_mut(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            ..Default::default()
        };
        unsafe { CredWriteW(&credential, 0) }.map_err(|_| "credential_store_unavailable".into())
    }
    fn delete(&self, name: &str) -> Result<(), String> {
        use windows::Win32::Foundation::ERROR_NOT_FOUND;
        use windows::Win32::Security::Credentials::*;
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        match unsafe {
            CredDeleteW(
                windows::core::PCWSTR(wide.as_ptr()),
                CRED_TYPE_GENERIC,
                None,
            )
        } {
            Ok(()) => Ok(()),
            Err(e) if e.code() == windows::core::HRESULT::from_win32(ERROR_NOT_FOUND.0) => Ok(()),
            Err(_) => Err("credential_store_unavailable".into()),
        }
    }
}
#[cfg(target_os = "linux")]
#[path = "receiver_auth/linux.rs"]
mod linux;
#[cfg(target_os = "macos")]
#[path = "receiver_auth/macos.rs"]
mod macos;

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn system_random(bytes: &mut [u8]) -> Result<(), String> {
    getrandom::fill(bytes).map_err(|_| "credential_store_unavailable".into())
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
impl Store for SystemStore {
    fn random(&self, _: &mut [u8]) -> Result<(), String> {
        Err("credential_store_unavailable".into())
    }
    fn read(&self, _: &str) -> Result<Option<Vec<u8>>, String> {
        Err("credential_store_unavailable".into())
    }
    fn write(&self, _: &str, _: &[u8]) -> Result<(), String> {
        Err("credential_store_unavailable".into())
    }
    fn delete(&self, _: &str) -> Result<(), String> {
        Err("credential_store_unavailable".into())
    }
}

#[cfg(test)]
pub(crate) mod tests;
