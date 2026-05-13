//! Data Protection Keychain (macOS 10.15+); auth via code-signing identity.

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

// SAFETY: well-known Security framework symbol (macOS 10.15+);
// some `security_framework_sys` versions don't export it, so we declare here.
unsafe extern "C" {
    static kSecUseDataProtectionKeychain: core_foundation_sys::string::CFStringRef;
}
use tracing::{info, warn};

const SERVICE_NAME: &str = "com.bobe.app";

fn base_query(account: &str) -> CFMutableDictionary {
    let mut query = CFMutableDictionary::new();

    // SAFETY: kSecClass and kSecClassGenericPassword are valid CoreFoundation constants
    // provided by the Security framework; casting to CFTypeRef is required by CFMutableDictionary.
    unsafe {
        query.set(kSecClass.cast(), kSecClassGenericPassword.cast());
    }
    // SAFETY: kSecAttrService is a valid Security framework key; the CFString value
    // lives long enough for the dictionary to retain it.
    unsafe {
        query.set(
            kSecAttrService.cast(),
            CFString::new(SERVICE_NAME).as_CFTypeRef(),
        );
    }
    // SAFETY: kSecAttrAccount is a valid Security framework key; the CFString value
    // lives long enough for the dictionary to retain it.
    unsafe {
        query.set(
            kSecAttrAccount.cast(),
            CFString::new(account).as_CFTypeRef(),
        );
    }
    // SAFETY: kSecUseDataProtectionKeychain is declared as an extern static CFStringRef
    // and is available on macOS 10.15+; casting to CFTypeRef is required by the dictionary API.
    unsafe {
        query.set(
            kSecUseDataProtectionKeychain.cast(),
            CFBoolean::true_value().as_CFTypeRef(),
        );
    }

    query
}

/// Replaces any existing entry; empty value = delete.
pub(crate) fn store_secret(account: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        let _ignored = delete_secret(account);
        return Ok(());
    }

    let _ignored = delete_secret(account);

    let mut query = base_query(account);
    let value_data = CFData::from_buffer(value.as_bytes());

    // SAFETY: kSecValueData is a valid Security framework key; the CFData value
    // lives long enough for the dictionary to retain it.
    unsafe {
        query.set(kSecValueData.cast(), value_data.as_CFTypeRef());
    }

    // SAFETY: SecItemAdd is called with a valid query dictionary; the null second
    // parameter indicates we do not need the persistent reference back.
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

    // SAFETY: kSecReturnData is a valid Security framework key; the CFBoolean value
    // is a singleton that outlives the dictionary.
    unsafe {
        query.set(
            kSecReturnData.cast(),
            CFBoolean::true_value().as_CFTypeRef(),
        );
    }

    let mut result: core_foundation::base::CFTypeRef = std::ptr::null();
    // SAFETY: SecItemCopyMatching is called with a valid query dictionary and a pointer
    // to a local CFTypeRef that will receive the matched data.
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

    // SAFETY: success + non-null result → Create Rule → we own and must release.
    // Cast to CFDataRef valid because we requested kSecReturnData above.
    let data = unsafe { CFData::wrap_under_create_rule(result.cast()) };
    String::from_utf8(data.bytes().to_vec()).ok()
}

pub(crate) fn delete_secret(account: &str) -> Result<(), String> {
    let query = base_query(account);

    // SAFETY: SecItemDelete is called with a valid query dictionary built by base_query.
    let status = unsafe { SecItemDelete(query.as_concrete_TypeRef()) };

    if status == 0 || status == -25300 {
        Ok(())
    } else {
        Err(format!(
            "Failed to delete secret '{account}': OSStatus {status}"
        ))
    }
}
