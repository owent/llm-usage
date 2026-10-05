//! Local Keychain generic passwords. Every operation disables synchronization and UI.
use super::{Store, SystemStore};
use core_foundation::{base::TCFType, string::CFString};
use security_framework::passwords::{self, PasswordOptions};
use security_framework_sys::{base::errSecItemNotFound, item::kSecUseAuthenticationUI};

// security-framework-sys exposes the UI key and Skip, but omits Apple's Fail constant.
#[link(name = "Security", kind = "framework")]
extern "C" {
    static kSecUseAuthenticationUIFail: core_foundation::string::CFStringRef;
}

const SERVICE: &str = "org.owent.llm-usage.otel.v1";

fn options(name: &str) -> PasswordOptions {
    let mut options = PasswordOptions::new_generic_password(SERVICE, name);
    options.set_access_synchronized(Some(false));
    // The library has no setter for this SecItem option. Keep the deprecated
    // query-field access confined here, using retained CoreFoundation values.
    #[allow(deprecated)]
    unsafe {
        options.query.push((
            CFString::wrap_under_get_rule(kSecUseAuthenticationUI),
            CFString::wrap_under_get_rule(kSecUseAuthenticationUIFail).into_CFType(),
        ));
    }
    options
}

impl Store for SystemStore {
    fn random(&self, bytes: &mut [u8]) -> Result<(), String> {
        super::system_random(bytes)
    }
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        match passwords::generic_password(options(name)) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.code() == errSecItemNotFound => Ok(None),
            Err(_) => Err("credential_store_unavailable".into()),
        }
    }
    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        passwords::set_generic_password_options(bytes, options(name))
            .map_err(|_| "credential_store_unavailable".into())
    }
    fn delete(&self, name: &str) -> Result<(), String> {
        match passwords::delete_generic_password_options(options(name)) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == errSecItemNotFound => Ok(()),
            Err(_) => Err("credential_store_unavailable".into()),
        }
    }
}
