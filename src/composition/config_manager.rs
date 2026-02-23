use std::collections::BTreeMap;
use std::sync::Arc;

use arc_swap::ArcSwap;
use tracing::{info, warn};

use crate::config::Config;
use super::config_persistence;

/// Settings that require a server restart (cannot be hot-swapped).
const STATIC_SETTINGS: &[&str] = &[
    "host",
    "port",
    "database_url",
    "embedding_model",
    "embedding_dimension",
    "llama_url",
    "ollama_url",
    "mdns_enabled",
];

/// Result of a config update operation.
#[derive(Debug)]
pub struct UpdateResult {
    pub applied_fields: Vec<String>,
    pub restart_required_fields: Vec<String>,
    pub persist_failed: bool,
}

/// Coordinates runtime config changes.
///
/// Lives in the composition root because it needs to touch both
/// application components (config push) and infrastructure (LLM rebuild).
pub struct ConfigManager {
    config: Arc<ArcSwap<Config>>,
}

impl ConfigManager {
    pub fn new(config: Arc<ArcSwap<Config>>) -> Self {
        Self { config }
    }

    /// Apply settings changes: validate, build new config, persist.
    pub fn update(&self, changes: &std::collections::HashMap<String, serde_json::Value>) -> UpdateResult {
        let mut result = UpdateResult {
            applied_fields: Vec::new(),
            restart_required_fields: Vec::new(),
            persist_failed: false,
        };

        let mut env_vars = BTreeMap::new();
        let current = self.config.load();

        for (key, _value) in changes {
            if STATIC_SETTINGS.contains(&key.as_str()) {
                result.restart_required_fields.push(key.clone());
                // Still persist for next restart
                if let Some(s) = _value.as_str() {
                    env_vars.insert(format!("BOBE_{}", key.to_uppercase()), s.to_owned());
                } else {
                    env_vars.insert(
                        format!("BOBE_{}", key.to_uppercase()),
                        _value.to_string().trim_matches('"').to_owned(),
                    );
                }
            } else {
                result.applied_fields.push(key.clone());
                if let Some(s) = _value.as_str() {
                    env_vars.insert(format!("BOBE_{}", key.to_uppercase()), s.to_owned());
                } else {
                    env_vars.insert(
                        format!("BOBE_{}", key.to_uppercase()),
                        _value.to_string().trim_matches('"').to_owned(),
                    );
                }
            }
        }

        // Persist to ~/.bobe/.env
        if !env_vars.is_empty() && !config_persistence::persist_config(&env_vars) {
            result.persist_failed = true;
        }

        info!(
            applied = ?result.applied_fields,
            restart_required = ?result.restart_required_fields,
            "config_manager.update_complete"
        );

        result
    }

    /// Get the current config.
    pub fn current(&self) -> arc_swap::Guard<Arc<Config>> {
        self.config.load()
    }
}
