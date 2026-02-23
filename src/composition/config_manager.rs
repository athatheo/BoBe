//! Runtime configuration manager.
//!
//! Coordinates settings changes across application and infrastructure components.
//! Lives in composition/ because it needs to touch both layers.
//!
//! Design:
//! - Config is an immutable snapshot behind ArcSwap
//! - Changes create a new Config by merging, then swap atomically
//! - LLM provider is rebuilt when backend/model fields change
//! - Persisted to ~/.bobe/.env for survival across restarts

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use arc_swap::ArcSwap;
use tracing::{error, info, warn};

use super::config_persistence;
use crate::adapters::llm::factory::LlmProviderFactory;
use crate::config::Config;
use crate::ports::llm::LlmProvider;

/// Settings that require a server restart (cannot be hot-swapped).
static STATIC_SETTINGS: &[&str] = &[
    "host",
    "port",
    "database_url",
    "embedding_model",
    "embedding_dimension",
    "llama_url",
    "ollama_url",
    "mdns_enabled",
];

/// LLM-related fields that trigger provider rebuild.
static LLM_FIELDS: &[&str] = &[
    "llm_backend",
    "ollama_model",
    "openai_model",
    "azure_openai_endpoint",
    "azure_openai_deployment",
];

/// API key fields — set in env, never written to .env file.
static LLM_API_KEY_FIELDS: &[&str] = &["openai_api_key", "azure_openai_api_key"];

/// Orchestrator config fields (hot-swappable).
static ORCH_FIELDS: &[&str] = &[
    "capture_enabled",
    "capture_interval_seconds",
    "decision_cooldown_minutes",
    "decision_extended_cooldown_minutes",
    "min_context_for_decision",
    "semantic_search_limit",
    "recent_ai_messages_limit",
    "max_response_tokens",
    "response_temperature",
    "checkin_enabled",
    "checkin_times",
    "checkin_jitter_minutes",
    "goal_check_interval_seconds",
    "conversation_inactivity_timeout_seconds",
    "conversation_auto_close_minutes",
    "conversation_summary_enabled",
    "tools_enabled",
];

/// Learning-related fields.
static LEARNING_FIELDS: &[&str] = &[
    "learning_enabled",
    "learning_interval_minutes",
    "learning_min_context_items",
    "learning_max_context_per_cycle",
    "learning_max_memories_per_cycle",
    "learning_max_goals_per_cycle",
    "learning_max_memories_per_consolidation",
    "daily_consolidation_hour",
];

/// Similarity threshold fields.
static SIMILARITY_FIELDS: &[&str] = &[
    "similarity_deduplication_threshold",
    "similarity_search_recall_threshold",
    "similarity_clustering_threshold",
];

/// Retention fields.
static RETENTION_FIELDS: &[&str] = &[
    "memory_raw_context_retention_days",
    "memory_short_term_retention_days",
    "memory_long_term_retention_days",
    "goal_retention_days",
    "memory_pruning_enabled",
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
    llm_provider: Arc<ArcSwap<Arc<dyn LlmProvider>>>,
    llm_factory: Option<Arc<LlmProviderFactory>>,
}

impl ConfigManager {
    pub fn new(
        config: Arc<ArcSwap<Config>>,
        llm_provider: Arc<ArcSwap<Arc<dyn LlmProvider>>>,
        llm_factory: Option<Arc<LlmProviderFactory>>,
    ) -> Self {
        Self {
            config,
            llm_provider,
            llm_factory,
        }
    }

    /// Get the current config.
    pub fn current(&self) -> arc_swap::Guard<Arc<Config>> {
        self.config.load()
    }

    /// Get the current LLM provider.
    pub fn current_llm(&self) -> arc_swap::Guard<Arc<Arc<dyn LlmProvider>>> {
        self.llm_provider.load()
    }

    /// Apply settings changes: classify, rebuild config, rebuild LLM if needed, persist.
    #[allow(unsafe_code)]
    pub fn update(&self, changes: &HashMap<String, serde_json::Value>) -> UpdateResult {
        let mut result = UpdateResult {
            applied_fields: Vec::new(),
            restart_required_fields: Vec::new(),
            persist_failed: false,
        };

        let static_set: HashSet<&str> = STATIC_SETTINGS.iter().copied().collect();
        let llm_set: HashSet<&str> = LLM_FIELDS.iter().copied().collect();
        let api_key_set: HashSet<&str> = LLM_API_KEY_FIELDS.iter().copied().collect();
        let orch_set: HashSet<&str> = ORCH_FIELDS.iter().copied().collect();
        let learning_set: HashSet<&str> = LEARNING_FIELDS.iter().copied().collect();
        let similarity_set: HashSet<&str> = SIMILARITY_FIELDS.iter().copied().collect();
        let retention_set: HashSet<&str> = RETENTION_FIELDS.iter().copied().collect();

        let mut env_vars = BTreeMap::new();
        let mut has_llm_changes = false;
        let mut has_config_changes = false;

        // 1. Classify changes into buckets
        for (key, value) in changes {
            let k = key.as_str();
            let str_val = serialize_json_value(value);

            if static_set.contains(k) {
                result.restart_required_fields.push(key.clone());
                env_vars.insert(format!("BOBE_{}", key.to_uppercase()), str_val);
            } else if api_key_set.contains(k) {
                // API keys: set in env, never persisted to .env
                has_llm_changes = true;
                // SAFETY: set_var is technically UB in multi-threaded Rust, but macOS/glibc
                // setenv is thread-safe. This is an infrequent config operation.
                unsafe {
                    std::env::set_var(format!("BOBE_{}", key.to_uppercase()), &str_val);
                }
                result.applied_fields.push(key.clone());
            } else if llm_set.contains(k) {
                has_llm_changes = true;
                has_config_changes = true;
                env_vars.insert(format!("BOBE_{}", key.to_uppercase()), str_val);
                result.applied_fields.push(key.clone());
            } else if orch_set.contains(k)
                || learning_set.contains(k)
                || similarity_set.contains(k)
                || retention_set.contains(k)
            {
                has_config_changes = true;
                env_vars.insert(format!("BOBE_{}", key.to_uppercase()), str_val);
                result.applied_fields.push(key.clone());
            } else {
                // Unknown field — persist anyway
                warn!(field = k, "config_manager.unknown_field");
                env_vars.insert(format!("BOBE_{}", key.to_uppercase()), str_val);
                result.applied_fields.push(key.clone());
            }
        }

        // 2. Persist to ~/.bobe/.env
        if !env_vars.is_empty() && !config_persistence::persist_config(&env_vars) {
            result.persist_failed = true;
        }

        // 3. Rebuild Config from env (picks up persisted + env changes)
        if has_config_changes {
            match Config::from_env() {
                Ok(new_config) => {
                    self.config.store(Arc::new(new_config));
                    info!("config_manager.config_swapped");
                }
                Err(e) => {
                    error!(error = %e, "config_manager.config_rebuild_failed");
                }
            }
        }

        // 4. Rebuild LLM provider if LLM fields changed
        if has_llm_changes {
            self.rebuild_llm(changes, &mut result);
        }

        info!(
            applied = ?result.applied_fields,
            restart_required = ?result.restart_required_fields,
            "config_manager.update_complete"
        );

        result
    }

    /// Rebuild LLM provider from current config and push to consumers.
    fn rebuild_llm(&self, changes: &HashMap<String, serde_json::Value>, result: &mut UpdateResult) {
        let Some(ref factory) = self.llm_factory else {
            warn!("config_manager.no_llm_factory");
            return;
        };

        let config = self.config.load();
        let backend = if let Some(v) = changes.get("llm_backend").and_then(|v| v.as_str()) {
            serde_json::from_value::<crate::config::LlmBackend>(serde_json::Value::String(
                v.to_owned(),
            ))
            .unwrap_or(config.llm_backend)
        } else {
            config.llm_backend
        };

        match factory.create(backend) {
            Ok(new_provider) => {
                self.llm_provider.store(Arc::new(new_provider));
                info!(backend = %backend, "config_manager.llm_rebuilt");
            }
            Err(e) => {
                error!(error = %e, backend = %backend, "config_manager.llm_rebuild_failed");
                // Mark LLM fields as requiring restart since rebuild failed
                for key in changes.keys() {
                    if LLM_FIELDS.contains(&key.as_str()) {
                        result.applied_fields.retain(|f| f != key);
                        result.restart_required_fields.push(key.clone());
                    }
                }
            }
        }
    }
}

/// Serialize a JSON value to a string suitable for env var / .env persistence.
fn serialize_json_value(value: &serde_json::Value) -> String {
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
