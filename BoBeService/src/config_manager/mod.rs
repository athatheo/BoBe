mod fields;
pub(crate) mod persistence;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use arc_swap::ArcSwap;
use tracing::{info, warn};

use crate::config::Config;

/// Require full daemon restart to apply.
static STATIC_FIELDS: &[&str] = &[
    "server.host",
    "server.port",
    "database.url",
    "server.mdns_enabled",
    "logging.file",
];

/// `engine.*` changes fire the listener wired to `WorkerRegistry::reload()`.
static HOT_SWAP_FIELDS: &[&str] = &[
    "capture.enabled",
    "capture.interval_seconds",
    "decision.cooldown_minutes",
    "decision.extended_cooldown_minutes",
    "decision.recent_ai_messages_limit",
    "checkin.enabled",
    "checkin.times",
    "checkin.interval_minutes",
    "checkin.jitter_minutes",
    "goals.check_interval_seconds",
    "conversation.inactivity_timeout_seconds",
    "conversation.auto_close_minutes",
    "logging.level",
    "logging.json",
    "server.cors_origins",
    "mcp.enabled",
    "mcp.config_file",
    "mcp.blocked_commands",
    "mcp.dangerous_env_keys",
    "engine.engine",
    "engine.provider_base_url",
    "engine.provider_chat_model",
    "engine.provider_batch_model",
    "engine.provider_vision_model",
    "engine.provider_offline",
    "voice.enabled",
    "voice.persona",
    "voice.speed",
    "seed_default_documents",
];

const ENGINE_FIELD_PREFIX: &str = "engine.";

#[derive(Debug)]
pub(crate) struct UpdateResult {
    pub(crate) applied_fields: Vec<String>,
    pub(crate) restart_required_fields: Vec<String>,
    pub(crate) persist_failed: bool,
}

type EngineChangeListener = Box<dyn Fn() + Send + Sync>;

pub(crate) struct ConfigManager {
    config: Arc<ArcSwap<Config>>,
    on_engine_change: std::sync::Mutex<Option<EngineChangeListener>>,
}

impl ConfigManager {
    pub(crate) fn new(config: Arc<ArcSwap<Config>>) -> Self {
        Self {
            config,
            on_engine_change: std::sync::Mutex::new(None),
        }
    }

    pub(crate) fn set_engine_change_listener<F>(&self, listener: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        if let Ok(mut guard) = self.on_engine_change.lock() {
            *guard = Some(Box::new(listener));
        }
    }

    pub(crate) fn update(&self, changes: &HashMap<String, serde_json::Value>) -> UpdateResult {
        let mut result = UpdateResult {
            applied_fields: Vec::new(),
            restart_required_fields: Vec::new(),
            persist_failed: false,
        };

        let static_set: HashSet<&str> = STATIC_FIELDS.iter().copied().collect();
        let hot_set: HashSet<&str> = HOT_SWAP_FIELDS.iter().copied().collect();

        let mut toml_changes = BTreeMap::new();
        let mut has_config_changes = false;
        let mut engine_changed = false;

        for (key, value) in changes {
            let dotted = fields::normalize_key_pub(key);
            let k = dotted.as_str();

            if static_set.contains(k) {
                result.restart_required_fields.push(key.clone());
                toml_changes.insert(dotted, value.clone());
            } else if hot_set.contains(k) {
                has_config_changes = true;
                if k.starts_with(ENGINE_FIELD_PREFIX) {
                    engine_changed = true;
                }
                toml_changes.insert(dotted, value.clone());
                result.applied_fields.push(key.clone());
            } else {
                warn!(field = k, "config_manager.unknown_field");
                toml_changes.insert(dotted, value.clone());
                result.applied_fields.push(key.clone());
            }
        }

        if !toml_changes.is_empty() && !persistence::persist(&toml_changes) {
            result.persist_failed = true;
        }

        if has_config_changes {
            let current = self.config.load();
            let mut new_config = (**current).clone();
            fields::apply(&mut new_config, changes);
            self.config.store(Arc::new(new_config));
            info!("config_manager.config_swapped");
        }

        if engine_changed
            && let Ok(guard) = self.on_engine_change.lock()
            && let Some(listener) = guard.as_ref()
        {
            info!("config_manager.engine_change_notify");
            listener();
        }

        info!(
            applied = ?result.applied_fields,
            restart_required = ?result.restart_required_fields,
            "config_manager.update_complete"
        );
        result
    }
}
