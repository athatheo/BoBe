use std::collections::BTreeMap;

use tracing::{error, info};

pub(crate) fn persist(
    data_root: &std::path::Path,
    changes: &BTreeMap<String, serde_json::Value>,
) -> bool {
    let dir = data_root;
    if let Err(e) = std::fs::create_dir_all(dir) {
        error!(error = %e, "config_persistence.mkdir_failed");
        return false;
    }

    let config_path = dir.join("config.toml");

    let existing = if config_path.exists() {
        match std::fs::read_to_string(&config_path) {
            Ok(s) => s,
            Err(e) => {
                error!(error = %e, "config_persistence.read_failed");
                return false;
            }
        }
    } else {
        "# BoBe configuration\n".to_string()
    };

    let mut doc: toml_edit::DocumentMut = match existing.parse() {
        Ok(d) => d,
        Err(e) => {
            error!(error = %e, "config_persistence.parse_failed");
            return false;
        }
    };
    doc.remove("config_version");

    for (dotted_key, value) in changes {
        set_toml_value(&mut doc, dotted_key, value);
    }

    let content = doc.to_string();

    if let Err(e) = crate::util::durable_fs::atomic_write_sync(&config_path, content.as_bytes()) {
        error!(error = %e, "config_persistence.write_failed");
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
        serde_json::Value::Number(n) if let Some(i) = n.as_i64() => toml_edit::value(i),
        serde_json::Value::Number(n) if let Some(f) = n.as_f64() => toml_edit::value(f),
        serde_json::Value::Number(n) => toml_edit::value(n.to_string()),
        serde_json::Value::String(s) => toml_edit::value(s.as_str()),
        serde_json::Value::Array(arr) => {
            let mut toml_arr = toml_edit::Array::new();
            for item in arr {
                match item {
                    serde_json::Value::String(s) => toml_arr.push(s.as_str()),
                    serde_json::Value::Number(n) if let Some(i) = n.as_i64() => toml_arr.push(i),
                    serde_json::Value::Number(n) if let Some(f) = n.as_f64() => toml_arr.push(f),
                    serde_json::Value::Bool(b) => toml_arr.push(*b),
                    _ => {}
                }
            }
            toml_edit::value(toml_arr)
        }
        serde_json::Value::Null | serde_json::Value::Object(_) => toml_edit::value(""),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "tests panic on fixture failures")]
mod tests {
    use std::collections::BTreeMap;

    use super::persist;

    #[test]
    fn persistence_removes_unused_legacy_config_version() {
        let root =
            std::env::temp_dir().join(format!("bobe-config-version-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create test config directory");
        std::fs::write(
            root.join("config.toml"),
            "config_version = 1\n[voice]\nenabled = true\n",
        )
        .expect("write test config");

        assert!(persist(&root, &BTreeMap::new()));
        let saved = std::fs::read_to_string(root.join("config.toml")).expect("read saved config");
        assert!(!saved.contains("config_version"));
        assert!(saved.contains("[voice]"));
        assert!(std::fs::remove_dir_all(root).is_ok());
    }
}
