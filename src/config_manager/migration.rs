//! One-time migration from legacy `.env` format to `config.toml`.

use super::fields::normalize_key_pub;

/// Migrate a `.env` file to nested TOML format.
pub(crate) fn migrate_env_to_toml(
    env_path: &std::path::Path,
    config_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(env_path)?;
    let mut doc = toml_edit::DocumentMut::new();

    // Parse .env key=value pairs and map to nested TOML structure
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_lowercase();
        let value = value.trim();

        // Strip BOBE_ prefix if present
        let key = key.strip_prefix("bobe_").unwrap_or(&key);

        map_flat_key_to_toml(&mut doc, key, value);
    }

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Write config.toml
    let toml_content = format!(
        "# BoBe configuration - migrated from .env\n# See docs for all options\nconfig_version = 1\n\n{}",
        doc
    );
    std::fs::write(config_path, toml_content)?;

    // Rename .env to .env.migrated
    let migrated = env_path.with_extension("env.migrated");
    std::fs::rename(env_path, migrated)?;

    tracing::info!("config.migrated_env_to_toml");
    Ok(())
}

/// Map a flat BOBE_* key to the correct nested TOML path.
///
/// Uses the shared `normalize_key` mapping from `fields.rs` as the single
/// source of truth for flat→dotted key conversion.
fn map_flat_key_to_toml(doc: &mut toml_edit::DocumentMut, key: &str, value: &str) {
    let dotted = normalize_key_pub(key);

    // Split "section.field" into parts
    let Some((section, field)) = dotted.split_once('.') else {
        // Top-level field (e.g. "soul_file", "setup_completed")
        if let Some(v) = parse_toml_value(value) {
            doc[key] = v;
        }
        return;
    };

    // Ensure the section table exists
    if doc.get(section).is_none() {
        doc[section] = toml_edit::Item::Table(toml_edit::Table::new());
    }

    // Handle special array fields (comma-separated → TOML arrays)
    let is_array_field = matches!(
        (section, field),
        ("server", "cors_origins")
            | ("checkin", "times")
            | ("mcp", "blocked_commands" | "dangerous_env_keys")
            | ("tools", "allowed_file_dirs")
    );

    if is_array_field {
        let mut arr = toml_edit::Array::new();
        for item in value.split(',') {
            let trimmed = item.trim();
            if !trimmed.is_empty() {
                arr.push(trimmed);
            }
        }
        doc[section][field] = toml_edit::value(arr);
    } else if let Some(v) = parse_toml_value(value) {
        doc[section][field] = v;
    }
}

/// Try to parse a string value into the most appropriate TOML type.
fn parse_toml_value(value: &str) -> Option<toml_edit::Item> {
    if value == "true" {
        return Some(toml_edit::value(true));
    }
    if value == "false" {
        return Some(toml_edit::value(false));
    }
    if let Ok(n) = value.parse::<i64>() {
        return Some(toml_edit::value(n));
    }
    if let Ok(f) = value.parse::<f64>() {
        return Some(toml_edit::value(f));
    }
    Some(toml_edit::value(value))
}
