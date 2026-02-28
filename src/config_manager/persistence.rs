//! Atomic `.env` persistence for runtime config changes.

use std::collections::BTreeMap;
use std::path::PathBuf;

use tracing::{error, info};

/// Serialize a JSON value to a flat string for `.env` storage.
pub fn serialize_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Bool(b) => if *b { "true" } else { "false" }.to_owned(),
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect::<Vec<_>>()
            .join(","),
        other => other.to_string().trim_matches('"').to_owned(),
    }
}

/// Persist key=value pairs to `~/.bobe/.env` via atomic temp-write + rename.
///
/// Returns `true` on success.
pub fn persist(changes: &BTreeMap<String, String>) -> bool {
    for (key, value) in changes {
        if value.contains('\n') || value.contains('\r') {
            error!(key = key.as_str(), "config_persistence.newline_in_value");
            return false;
        }
        if value.len() > 10_000 {
            error!(
                key = key.as_str(),
                length = value.len(),
                "config_persistence.value_too_long"
            );
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

    let mut existing = read_env_file(&env_path);
    for (k, v) in changes {
        existing.insert(k.clone(), v.clone());
    }

    let content: String = existing
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";

    if let Err(e) = std::fs::write(&tmp_path, &content) {
        error!(error = %e, "config_persistence.write_failed");
        return false;
    }
    if let Err(e) = std::fs::rename(&tmp_path, &env_path) {
        error!(error = %e, "config_persistence.rename_failed");
        return false;
    }

    info!(keys = ?changes.keys().collect::<Vec<_>>(), "config_persistence.persisted");
    true
}

fn bobe_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/tmp".into())))
        .join(".bobe")
}

fn read_env_file(path: &PathBuf) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    if let Ok(content) = std::fs::read_to_string(path) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                map.insert(k.trim().to_owned(), v.trim().to_owned());
            }
        }
    }
    map
}
