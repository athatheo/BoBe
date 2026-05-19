//! Secret storage. macOS: Data Protection Keychain (app-private, no prompt
//! for the owning signed app). Linux/Unix: `~/.bobe/secrets.json`, 0600.
//!
//! Windows is not a target. Adding it would need DPAPI or a different
//! file-mode strategy than the Unix `chmod 0600` used here.

#[cfg(not(any(target_os = "macos", unix)))]
compile_error!("BoBe daemon supports only macOS and Unix-family OSes (Linux, *BSD).");

use std::sync::{Arc, OnceLock};

#[derive(Debug, thiserror::Error)]
pub(crate) enum SecretError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialize(#[from] serde_json::Error),

    #[error("Lock poisoned: {0}")]
    Lock(String),

    /// Catch-all for platform-specific failures (Keychain OSStatus, etc.)
    /// that don't map onto the structured variants.
    #[error("{0}")]
    Backend(String),
}

/// Stable API for handler/service code; backend swaps per platform.
pub(crate) trait SecretStore: Send + Sync {
    fn read(&self, account: &str) -> Option<String>;
    fn store(&self, account: &str, value: &str) -> Result<(), SecretError>;
    fn delete(&self, account: &str) -> Result<(), SecretError>;
}

/// Process-wide singleton so boot-phase code (`mcp/config.rs`) and
/// handler-phase code (`mcp_config_service`) dispatch through the same
/// `Arc`. Without this the read path and write path could silently
/// disagree even though they're nominally the same backend.
static DEFAULT_STORE: OnceLock<Arc<dyn SecretStore>> = OnceLock::new();

pub(crate) fn default_secret_store() -> Arc<dyn SecretStore> {
    Arc::clone(DEFAULT_STORE.get_or_init(|| {
        #[cfg(target_os = "macos")]
        {
            Arc::new(macos::KeychainSecretStore) as Arc<dyn SecretStore>
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            Arc::new(file::FileSecretStore::new()) as Arc<dyn SecretStore>
        }
    }))
}

/// Boot-phase read for `mcp/config.rs` deep materialization, before
/// `AppState` exists. Dispatches through the same singleton that AppState
/// later receives, so the boot read and the handler-time `Arc<dyn SecretStore>`
/// hit the identical backend instance.
pub(crate) fn read_secret(account: &str) -> Option<String> {
    default_secret_store().read(account)
}

#[cfg(target_os = "macos")]
mod macos {
    #![allow(unsafe_code)]

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
    use tracing::{info, warn};

    // SAFETY: well-known Security framework symbol (macOS 10.15+); not always
    // exported by `security_framework_sys`, so we declare it locally.
    unsafe extern "C" {
        static kSecUseDataProtectionKeychain: core_foundation_sys::string::CFStringRef;
    }

    const SERVICE_NAME: &str = "com.bobe.app";

    fn base_query(account: &str) -> CFMutableDictionary {
        let mut query = CFMutableDictionary::new();
        // SAFETY: Security framework constants + CFString values live long enough for the dict.
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
            // Use Data Protection Keychain: app-private, code-signing-gated,
            // no prompt for the owning signed app.
            query.set(
                kSecUseDataProtectionKeychain.cast(),
                CFBoolean::true_value().as_CFTypeRef(),
            );
        }
        query
    }

    pub(super) fn store_secret(account: &str, value: &str) -> Result<(), super::SecretError> {
        if value.is_empty() {
            let _ignored = delete_secret(account);
            return Ok(());
        }

        let _ignored = delete_secret(account);

        let mut query = base_query(account);
        let value_data = CFData::from_buffer(value.as_bytes());
        // SAFETY: kSecValueData is a valid Security framework key; CFData lives long enough.
        unsafe {
            query.set(kSecValueData.cast(), value_data.as_CFTypeRef());
        }
        // SAFETY: valid query dict; null second arg = don't return a persistent ref.
        let status = unsafe { SecItemAdd(query.as_concrete_TypeRef(), std::ptr::null_mut()) };

        if status == 0 {
            info!(account, "secrets.stored");
            Ok(())
        } else {
            warn!(account, status, "secrets.store_failed");
            Err(super::SecretError::Backend(format!(
                "Failed to store secret '{account}': OSStatus {status}"
            )))
        }
    }

    pub(super) fn read_secret(account: &str) -> Option<String> {
        let mut query = base_query(account);
        // SAFETY: kSecReturnData is a valid Security framework key; CFBoolean is a singleton.
        unsafe {
            query.set(
                kSecReturnData.cast(),
                CFBoolean::true_value().as_CFTypeRef(),
            );
        }

        let mut result: core_foundation::base::CFTypeRef = std::ptr::null();
        // SAFETY: valid query + pointer to local CFTypeRef that will receive the match.
        let status = unsafe { SecItemCopyMatching(query.as_concrete_TypeRef(), &raw mut result) };

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
        // SAFETY: success + non-null → Create Rule → we own and must release.
        let data = unsafe { CFData::wrap_under_create_rule(result.cast()) };
        String::from_utf8(data.bytes().to_vec()).ok()
    }

    pub(super) fn delete_secret(account: &str) -> Result<(), super::SecretError> {
        let query = base_query(account);
        // SAFETY: valid query dict from base_query.
        let status = unsafe { SecItemDelete(query.as_concrete_TypeRef()) };

        if status == 0 || status == -25300 {
            Ok(())
        } else {
            Err(super::SecretError::Backend(format!(
                "Failed to delete secret '{account}': OSStatus {status}"
            )))
        }
    }

    pub(crate) struct KeychainSecretStore;

    impl super::SecretStore for KeychainSecretStore {
        fn read(&self, account: &str) -> Option<String> {
            read_secret(account)
        }
        fn store(&self, account: &str, value: &str) -> Result<(), super::SecretError> {
            store_secret(account, value)
        }
        fn delete(&self, account: &str) -> Result<(), super::SecretError> {
            delete_secret(account)
        }
    }
}

#[cfg(unix)]
#[cfg_attr(
    target_os = "macos",
    allow(dead_code, reason = "Linux production path + cross-platform test seam")
)]
pub(crate) mod file {
    use std::collections::HashMap;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    use tracing::{info, warn};

    /// JSON file backend; mode 0600. Suitable for single-user daemons.
    /// On macOS we still default to Keychain — this module is the Linux
    /// production path and a test seam everywhere else.
    pub(crate) struct FileSecretStore {
        path: PathBuf,
        cache: Mutex<HashMap<String, String>>,
    }

    impl FileSecretStore {
        pub(crate) fn new() -> Self {
            let path = crate::util::paths::bobe_data_dir().join("secrets.json");
            Self::at_path(path)
        }

        pub(crate) fn at_path(path: PathBuf) -> Self {
            let cache = load(&path).unwrap_or_default();
            Self {
                path,
                cache: Mutex::new(cache),
            }
        }

        fn flush(&self, map: &HashMap<String, String>) -> Result<(), super::SecretError> {
            write_atomic(&self.path, map)
        }
    }

    fn write_atomic(path: &Path, map: &HashMap<String, String>) -> Result<(), super::SecretError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(map)?;
        let tmp = path.with_extension("json.tmp");
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .mode(0o600)
                .open(&tmp)?;
            f.write_all(&bytes)?;
        }
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    fn load(path: &Path) -> Option<HashMap<String, String>> {
        serde_json::from_slice(&std::fs::read(path).ok()?).ok()
    }

    impl super::SecretStore for FileSecretStore {
        fn read(&self, account: &str) -> Option<String> {
            load(&self.path).and_then(|m| m.get(account).cloned())
        }

        fn store(&self, account: &str, value: &str) -> Result<(), super::SecretError> {
            let mut guard = self
                .cache
                .lock()
                .map_err(|e| super::SecretError::Lock(e.to_string()))?;
            if value.is_empty() {
                guard.remove(account);
            } else {
                guard.insert(account.to_owned(), value.to_owned());
            }
            self.flush(&guard)?;
            info!(account, "secrets.stored");
            Ok(())
        }

        fn delete(&self, account: &str) -> Result<(), super::SecretError> {
            let mut guard = self
                .cache
                .lock()
                .map_err(|e| super::SecretError::Lock(e.to_string()))?;
            let removed = guard.remove(account).is_some();
            self.flush(&guard)?;
            if !removed {
                warn!(account, "secrets.delete_missing");
            }
            Ok(())
        }
    }

    #[cfg(test)]
    #[allow(clippy::unwrap_used, clippy::expect_used)]
    mod tests {
        use super::*;
        use crate::secrets::SecretStore;

        fn tmpfile() -> PathBuf {
            std::env::temp_dir().join(format!("bobe-secrets-{}.json", uuid::Uuid::new_v4()))
        }

        #[test]
        fn store_then_read_back() {
            let path = tmpfile();
            let store = FileSecretStore::at_path(path.clone());
            store.store("acct", "hunter2").unwrap();
            let map = load(&path).unwrap();
            assert_eq!(map.get("acct").map(String::as_str), Some("hunter2"));
            drop(std::fs::remove_file(&path));
        }

        #[test]
        fn empty_value_deletes() {
            let path = tmpfile();
            let store = FileSecretStore::at_path(path.clone());
            store.store("acct", "x").unwrap();
            store.store("acct", "").unwrap();
            let map = load(&path).unwrap_or_default();
            assert!(!map.contains_key("acct"), "empty store should drop the key");
            drop(std::fs::remove_file(&path));
        }

        #[test]
        fn delete_clears_entry() {
            let path = tmpfile();
            let store = FileSecretStore::at_path(path.clone());
            store.store("acct", "x").unwrap();
            store.delete("acct").unwrap();
            let map = load(&path).unwrap_or_default();
            assert!(!map.contains_key("acct"));
            drop(std::fs::remove_file(&path));
        }

        #[test]
        fn file_mode_is_0600() {
            use std::os::unix::fs::PermissionsExt;
            let path = tmpfile();
            let store = FileSecretStore::at_path(path.clone());
            store.store("acct", "x").unwrap();
            let perms = std::fs::metadata(&path).unwrap().permissions();
            assert_eq!(perms.mode() & 0o777, 0o600);
            drop(std::fs::remove_file(&path));
        }

        #[test]
        fn missing_file_loads_empty() {
            let path = tmpfile();
            // Constructor must not panic on a missing file.
            let store = FileSecretStore::at_path(path.clone());
            assert!(store.read("anything").is_none());
        }

        #[test]
        fn read_through_trait_returns_stored_value() {
            // Closes Finding 3 from the post-commit review: reads and writes
            // now both flow through `SecretStore` so a future test stub
            // intercepts both halves consistently.
            let path = tmpfile();
            let store: Box<dyn SecretStore> = Box::new(FileSecretStore::at_path(path.clone()));
            store.store("acct", "hunter2").unwrap();
            assert_eq!(store.read("acct").as_deref(), Some("hunter2"));
            assert!(store.read("nope").is_none());
            drop(std::fs::remove_file(&path));
        }
    }
}
