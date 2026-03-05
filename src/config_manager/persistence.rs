//! Atomic `config.toml` persistence via temp-write + rename.

use std::collections::BTreeMap;
use std::path::PathBuf;

use tracing::{error, info};

pub(crate) fn persist(changes: &BTreeMap<String, serde_json::Value>) -> bool {
    let dir = bobe_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        error!(error = %e, "config_persistence.mkdir_failed");
        return false;
    }

    let config_path = dir.join("config.toml");
    let tmp_path = dir.join("config.toml.tmp");

    let existing = if config_path.exists() {
        match std::fs::read_to_string(&config_path) {
            Ok(s) => s,
            Err(e) => {
                error!(error = %e, "config_persistence.read_failed");
                return false;
            }
        }
    } else {
        "# BoBe configuration\nconfig_version = 1\n".to_string()
    };

    let mut doc: toml_edit::DocumentMut = match existing.parse() {
        Ok(d) => d,
        Err(e) => {
            error!(error = %e, "config_persistence.parse_failed");
            return false;
        }
    };

    for (dotted_key, value) in changes {
        set_toml_value(&mut doc, dotted_key, value);
    }

    let content = doc.to_string();

    if let Err(e) = std::fs::write(&tmp_path, &content) {
        error!(error = %e, "config_persistence.write_failed");
        return false;
    }
    if let Err(e) = std::fs::rename(&tmp_path, &config_path) {
        error!(error = %e, "config_persistence.rename_failed");
        return false;
    }

    info!(keys = ?changes.keys().collect::<Vec<_>>(), "config_persistence.persisted");
    true
}

fn set_toml_value(doc: &mut toml_edit::DocumentMut, dotted_key: &str, value: &serde_json::Value) {
    let parts: Vec<&str> = dotted_key.split('.').collect();

    match parts.len() {
        1 => {
            doc[parts[0]] = json_to_toml_item(value);
        }
        2 => {
            let section = parts[0];
            let field = parts[1];

            if doc.get(section).is_none() {
                doc[section] = toml_edit::Item::Table(toml_edit::Table::new());
            }
            doc[section][field] = json_to_toml_item(value);
        }
        _ => {
            tracing::warn!(key = dotted_key, "config_persistence.unsupported_nesting");
        }
    }
}

fn json_to_toml_item(value: &serde_json::Value) -> toml_edit::Item {
    match value {
        serde_json::Value::Bool(b) => toml_edit::value(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                toml_edit::value(i)
            } else if let Some(f) = n.as_f64() {
                toml_edit::value(f)
            } else {
                toml_edit::value(n.to_string())
            }
        }
        serde_json::Value::String(s) => toml_edit::value(s.as_str()),
        serde_json::Value::Array(arr) => {
            let mut toml_arr = toml_edit::Array::new();
            for item in arr {
                match item {
                    serde_json::Value::String(s) => toml_arr.push(s.as_str()),
                    serde_json::Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            toml_arr.push(i);
                        } else if let Some(f) = n.as_f64() {
                            toml_arr.push(f);
                        }
                    }
                    serde_json::Value::Bool(b) => toml_arr.push(*b),
                    _ => {}
                }
            }
            toml_edit::value(toml_arr)
        }
        serde_json::Value::Null | serde_json::Value::Object(_) => toml_edit::value(""),
    }
}

fn bobe_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let data_dir = std::env::var("BOBE_DATA_DIR").unwrap_or_else(|_| format!("{home}/.bobe"));
    PathBuf::from(data_dir)
}
