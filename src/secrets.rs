//! macOS Data Protection Keychain integration for API key storage.
//!
//! Uses the Data Protection Keychain (`kSecUseDataProtectionKeychain = true`)
//! which authenticates via code-signing identity instead of password prompts.
//! Requires: macOS 10.15+, signed binaries, same team ID.
//!
//! Wraps the raw Security framework C APIs via `security_framework_sys`.

use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::data::CFData;
use core_foundation::dictionary::CFMutableDictionary;
use core_foundation::string::CFString;
use security_framework_sys::item::*;
use security_framework_sys::keychain_item::{SecItemAdd, SecItemCopyMatching, SecItemDelete};

// kSecUseDataProtectionKeychain is available since macOS 10.15 but may not be
// in all versions of security_framework_sys. Declare it ourselves.
unsafe extern "C" {
    static kSecUseDataProtectionKeychain: core_foundation_sys::string::CFStringRef;
}
use tracing::{info, warn};

const SERVICE_NAME: &str = "com.bobe.app";

/// Secret field names (dotted config keys) that map to keychain accounts.
pub static SECRET_FIELDS: &[&str] = &[
    "llm.openai_api_key",
    "llm.azure_openai_api_key",
    "llm.anthropic_api_key",
];

/// Build the base query dictionary for a generic password item.
fn base_query(account: &str) -> CFMutableDictionary {
    let mut query = CFMutableDictionary::new();

    unsafe {
        // kSecClass = kSecClassGenericPassword
        query.set(kSecClass.cast(), kSecClassGenericPassword.cast());
        // kSecAttrService
        query.set(
            kSecAttrService.cast(),
            CFString::new(SERVICE_NAME).as_CFTypeRef(),
        );
        // kSecAttrAccount
        query.set(
            kSecAttrAccount.cast(),
            CFString::new(account).as_CFTypeRef(),
        );
        // kSecUseDataProtectionKeychain = true
        query.set(
            kSecUseDataProtectionKeychain.cast(),
            CFBoolean::true_value().as_CFTypeRef(),
        );
    }

    query
}

/// Store a secret in the Data Protection Keychain.
///
/// If a secret with the same account already exists, it is deleted first.
pub fn store_secret(account: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        let _ = delete_secret(account);
        return Ok(());
    }

    // Delete existing entry first (keychain doesn't support upsert)
    let _ = delete_secret(account);

    let mut query = base_query(account);
    let value_data = CFData::from_buffer(value.as_bytes());

    unsafe {
        // kSecValueData
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

/// Read a secret from the Data Protection Keychain.
///
/// Returns `None` if not found.
pub fn read_secret(account: &str) -> Option<String> {
    let mut query = base_query(account);

    unsafe {
        // kSecReturnData = true
        query.set(
            kSecReturnData.cast(),
            CFBoolean::true_value().as_CFTypeRef(),
        );
    }

    let mut result: core_foundation::base::CFTypeRef = std::ptr::null();
    let status =
        unsafe { SecItemCopyMatching(query.as_concrete_TypeRef(), std::ptr::addr_of_mut!(result)) };

    if status == -25300 {
        // errSecItemNotFound — expected when key doesn't exist
        return None;
    }

    if status != 0 {
        warn!(account, status, "secrets.read_failed");
        return None;
    }

    if result.is_null() {
        return None;
    }

    // Result is CFData when kSecReturnData is true
    let data = unsafe { CFData::wrap_under_create_rule(result.cast()) };
    String::from_utf8(data.bytes().to_vec()).ok()
}

/// Delete a secret from the Data Protection Keychain.
pub fn delete_secret(account: &str) -> Result<(), String> {
    let query = base_query(account);

    let status = unsafe { SecItemDelete(query.as_concrete_TypeRef()) };

    if status == 0 || status == -25300 {
        // success or not found
        Ok(())
    } else {
        Err(format!(
            "Failed to delete secret '{account}': OSStatus {status}"
        ))
    }
}

/// Load all secrets from Keychain into a map.
///
/// For each secret field, attempts to read from Keychain.
/// Returns a map of dotted-key → value for secrets that were found.
pub fn load_secrets() -> std::collections::HashMap<String, String> {
    let mut secrets = std::collections::HashMap::new();

    for &field in SECRET_FIELDS {
        let account = keychain_account(field);
        if let Some(value) = read_secret(&account) {
            if !value.is_empty() {
                secrets.insert(field.to_string(), value);
            }
        }
    }

    if !secrets.is_empty() {
        info!(count = secrets.len(), "secrets.loaded_from_keychain");
    }

    secrets
}

/// Check if a dotted config key is a secret field.
pub fn is_secret_field(dotted_key: &str) -> bool {
    SECRET_FIELDS.contains(&dotted_key)
}

/// Convert a dotted config key to a keychain account name.
///
/// E.g. `"llm.openai_api_key"` → `"openai_api_key"`
fn keychain_account(dotted_key: &str) -> String {
    dotted_key
        .split('.')
        .next_back()
        .unwrap_or(dotted_key)
        .to_string()
}

/// Migrate secrets from config values to Keychain.
///
/// Stores any non-empty API keys found in the config into the Keychain,
/// then returns the keys that were migrated (so they can be removed from config.toml).
pub fn migrate_secrets_to_keychain(config: &crate::config::Config) -> Vec<String> {
    let mut migrated = Vec::new();

    let key_map: &[(&str, &str)] = &[
        ("llm.openai_api_key", &config.llm.openai_api_key),
        ("llm.azure_openai_api_key", &config.llm.azure_openai_api_key),
        ("llm.anthropic_api_key", &config.llm.anthropic_api_key),
    ];

    for (field, value) in key_map {
        if !value.is_empty() {
            let account = keychain_account(field);
            match store_secret(&account, value) {
                Ok(()) => {
                    info!(field, "secrets.migrated_to_keychain");
                    migrated.push(field.to_string());
                }
                Err(e) => {
                    warn!(field, error = %e, "secrets.migration_failed");
                }
            }
        }
    }

    migrated
}
