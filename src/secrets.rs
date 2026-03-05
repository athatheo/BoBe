//! macOS Data Protection Keychain integration for API key storage.
//!
//! Uses code-signing identity auth (macOS 10.15+, signed binaries, same team ID).

use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::data::CFData;
use core_foundation::dictionary::CFMutableDictionary;
use core_foundation::string::CFString;
use security_framework_sys::item::{
    kSecAttrAccount, kSecAttrService, kSecClass, kSecClassGenericPassword, kSecReturnData,
    kSecValueData,
};
use security_framework_sys::keychain_item::{SecItemAdd, SecItemCopyMatching, SecItemDelete};

// Not always in security_framework_sys — declare it ourselves.
unsafe extern "C" {
    static kSecUseDataProtectionKeychain: core_foundation_sys::string::CFStringRef;
}
use tracing::{info, warn};

const SERVICE_NAME: &str = "com.bobe.app";

/// Dotted config keys that map to keychain accounts.
pub(crate) static SECRET_FIELDS: &[&str] = &[
    "llm.openai_api_key",
    "llm.azure_openai_api_key",
    "llm.anthropic_api_key",
];

fn base_query(account: &str) -> CFMutableDictionary {
    let mut query = CFMutableDictionary::new();

    unsafe {
        query.set(kSecClass.cast(), kSecClassGenericPassword.cast());
        query.set(
            kSecAttrService.cast(),
            CFString::new(SERVICE_NAME).as_CFTypeRef(),
        );
        query.set(
            kSecAttrAccount.cast(),
            CFString::new(account).as_CFTypeRef(),
        );
        query.set(
            kSecUseDataProtectionKeychain.cast(),
            CFBoolean::true_value().as_CFTypeRef(),
        );
    }

    query
}

/// Store a secret, replacing any existing entry.
pub(crate) fn store_secret(account: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        let _ = delete_secret(account);
        return Ok(());
    }

    let _ = delete_secret(account);

    let mut query = base_query(account);
    let value_data = CFData::from_buffer(value.as_bytes());

    unsafe {
        query.set(kSecValueData.cast(), value_data.as_CFTypeRef());
    }

    let status = unsafe { SecItemAdd(query.as_concrete_TypeRef(), std::ptr::null_mut()) };

    if status == 0 {
        info!(account, "secrets.stored");
        Ok(())
    } else {
        let msg = format!("Failed to store secret '{account}': OSStatus {status}");
        warn!(account, status, "secrets.store_failed");
        Err(msg)
    }
}

pub(crate) fn read_secret(account: &str) -> Option<String> {
    let mut query = base_query(account);

    unsafe {
        query.set(
            kSecReturnData.cast(),
            CFBoolean::true_value().as_CFTypeRef(),
        );
    }

    let mut result: core_foundation::base::CFTypeRef = std::ptr::null();
    let status =
        unsafe { SecItemCopyMatching(query.as_concrete_TypeRef(), std::ptr::addr_of_mut!(result)) };

    if status == -25300 {
        return None;
    }

    if status != 0 {
        warn!(account, status, "secrets.read_failed");
        return None;
    }

    if result.is_null() {
        return None;
    }

    let data = unsafe { CFData::wrap_under_create_rule(result.cast()) };
    String::from_utf8(data.bytes().to_vec()).ok()
}

pub(crate) fn delete_secret(account: &str) -> Result<(), String> {
    let query = base_query(account);

    let status = unsafe { SecItemDelete(query.as_concrete_TypeRef()) };

    if status == 0 || status == -25300 {
        Ok(())
    } else {
        Err(format!(
            "Failed to delete secret '{account}': OSStatus {status}"
        ))
    }
}

/// Returns dotted-key → value map of all secrets from Keychain.
pub(crate) fn load_secrets() -> std::collections::HashMap<String, String> {
    let mut secrets = std::collections::HashMap::new();

    for &field in SECRET_FIELDS {
        let account = keychain_account(field);
        if let Some(value) = read_secret(&account)
            && !value.is_empty()
        {
            secrets.insert(field.to_string(), value);
        }
    }

    if !secrets.is_empty() {
        info!(count = secrets.len(), "secrets.loaded_from_keychain");
    }

    secrets
}

pub(crate) fn is_secret_field(dotted_key: &str) -> bool {
    SECRET_FIELDS.contains(&dotted_key)
}

fn keychain_account(dotted_key: &str) -> String {
    dotted_key
        .split('.')
        .next_back()
        .unwrap_or(dotted_key)
        .to_string()
}
