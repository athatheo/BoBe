use std::collections::BTreeMap;
use std::path::PathBuf;

use tracing::{info, warn, error};

/// Get the BoBe configuration directory (~/.bobe).
fn bobe_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".bobe")
}

/// Persist config key=value pairs to ~/.bobe/.env AND set on the running process.
///
/// Performs atomic write (write temp → rename) to prevent corruption.
/// API keys must NOT be written here — use OS keychain instead.
///
/// Returns true on success, false on failure.
pub fn persist_config(changes: &BTreeMap<String, String>) -> bool {
    // Sanitize values
    for (key, value) in changes {
        if value.contains('\n') || value.contains('\r') {
            error!(key = key.as_str(), "config_persistence.newline_in_value");
            return false;
        }
        if value.len() > 10_000 {
            error!(key = key.as_str(), length = value.len(), "config_persistence.value_too_long");
            return false;
        }
    }

    let dir = bobe_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        error!(error = %e, "config_persistence.mkdir_failed");
        return false;
    }

    let env_path = dir.join(".env");
    let tmp_path = dir.join(".env.tmp");

    // Read existing .env
    let mut existing = BTreeMap::new();
    if env_path.exists() {
        match std::fs::read_to_string(&env_path) {
            Ok(content) => {
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    if let Some((k, v)) = line.split_once('=') {
                        existing.insert(k.trim().to_owned(), v.trim().to_owned());
                    }
                }
            }
            Err(_) => {
                warn!("config_persistence.env_read_failed");
            }
        }
    }

    // Merge changes
    for (k, v) in changes {
        existing.insert(k.clone(), v.clone());
    }

    // Atomic write
    let lines: Vec<String> = existing
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    let content = lines.join("\n") + "\n";

    if let Err(e) = std::fs::write(&tmp_path, &content) {
        error!(error = %e, "config_persistence.write_failed");
        return false;
    }

    if let Err(e) = std::fs::rename(&tmp_path, &env_path) {
        error!(error = %e, "config_persistence.rename_failed");
        return false;
    }

    // Set on running process
    for (k, v) in changes {
        // SAFETY: called from single-threaded bootstrap before server starts
        unsafe {
            std::env::set_var(k, v);
        }
    }

    info!(keys = ?changes.keys().collect::<Vec<_>>(), "config_persistence.persisted");
    true
}

/// Check if user has ever configured BoBe (has a .env file).
pub fn has_persisted_config() -> bool {
    bobe_dir().join(".env").exists()
}
