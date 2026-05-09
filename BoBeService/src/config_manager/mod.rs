//! Runtime configuration manager — hot-swap of in-memory `Config` via
//! `ArcSwap` plus persistence to `config.toml`.
//!
//! Pre-pivot this module also rebuilt `LlmProvider` and
//! `EmbeddingProvider` instances when their config keys changed
//! (backend / model / API key). Post-pivot Copilot CLI is the engine,
//! so there are no providers to rebuild — the manager just classifies
//! whether a change is hot-applicable or restart-required, persists
//! it, and atomically swaps the in-memory `Config`.

mod fields;
pub(crate) mod persistence;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use arc_swap::ArcSwap;
use tracing::{info, warn};

use crate::config::Config;

/// Fields that require a full daemon restart to apply (network
/// bindings, on-disk DB path, mDNS toggles, etc.).
static STATIC_FIELDS: &[&str] = &[
    "server.host",
    "server.port",
    "database.url",
    "server.mdns_enabled",
    "logging.file",
];

/// Fields safe to swap at runtime via `fields::apply`.
static HOT_SWAP_FIELDS: &[&str] = &[
    "capture.enabled",
    "capture.interval_seconds",
    "decision.cooldown_minutes",
    "decision.extended_cooldown_minutes",
    "decision.min_context",
    "decision.recent_ai_messages_limit",
    "checkin.enabled",
    "checkin.times",
    "checkin.interval_minutes",
    "checkin.jitter_minutes",
    "goals.check_interval_seconds",
    "goals.max_active",
    "conversation.inactivity_timeout_seconds",
    "conversation.auto_close_minutes",
    "conversation.summary_enabled",
    "logging.level",
    "logging.json",
    "server.cors_origins",
    "mcp.enabled",
    "mcp.config_file",
    "mcp.blocked_commands",
    "mcp.dangerous_env_keys",
    "coding_agent.enabled",
    "coding_agent.profiles",
    "coding_agent.output_dir",
    "coding_agent.poll_interval_seconds",
    "coding_agent.max_concurrent",
    "coding_agent.max_runtime_seconds",
    "soul_file",
    "seed_default_documents",
    "setup_completed",
    "locale_override",
];

#[derive(Debug)]
pub(crate) struct UpdateResult {
    pub(crate) applied_fields: Vec<String>,
    pub(crate) restart_required_fields: Vec<String>,
    pub(crate) persist_failed: bool,
}

pub(crate) struct ConfigManager {
    config: Arc<ArcSwap<Config>>,
}

impl ConfigManager {
    pub(crate) fn new(config: Arc<ArcSwap<Config>>) -> Self {
        Self { config }
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

        for (key, value) in changes {
            let dotted = fields::normalize_key_pub(key);
            let k = dotted.as_str();

            if static_set.contains(k) {
                result.restart_required_fields.push(key.clone());
                toml_changes.insert(dotted, value.clone());
            } else if hot_set.contains(k) {
                has_config_changes = true;
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

        info!(
            applied = ?result.applied_fields,
            restart_required = ?result.restart_required_fields,
            "config_manager.update_complete"
        );
        result
    }
}
