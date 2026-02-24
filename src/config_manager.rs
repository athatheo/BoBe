//! Runtime configuration manager.
//!
//! Coordinates settings changes across application and infrastructure components.
//!
//! Design:
//! - Config is an immutable snapshot behind ArcSwap
//! - Changes create a new Config by merging, then swap atomically
//! - LLM provider is rebuilt when backend/model fields change
//! - Persisted to ~/.bobe/.env for survival across restarts

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use arc_swap::ArcSwap;
use tracing::{error, info, warn};

use crate::llm::factory::LlmProviderFactory;
use crate::config::Config;
use crate::llm::LlmProvider;

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
    #[allow(dead_code)]
    pub fn current(&self) -> arc_swap::Guard<Arc<Config>> {
        self.config.load()
    }

    /// Get the current LLM provider.
    #[allow(dead_code)]
    pub fn current_llm(&self) -> arc_swap::Guard<Arc<Arc<dyn LlmProvider>>> {
        self.llm_provider.load()
    }

    /// Apply settings changes: classify, persist non-secret fields, patch config in-memory, rebuild LLM if needed.
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
                // API keys: applied in-memory only, never persisted to .env
                has_llm_changes = true;
                has_config_changes = true;
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

        // 2. Persist non-secret fields to ~/.bobe/.env
        if !env_vars.is_empty() && !persist_config(&env_vars) {
            result.persist_failed = true;
        }

        // 3. Clone current config and apply all changes in-memory
        if has_config_changes {
            let current = self.config.load();
            let mut new_config = (**current).clone();
            apply_changes(&mut new_config, changes);
            self.config.store(Arc::new(new_config));
            info!("config_manager.config_swapped");
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

/// Apply a set of changes to a Config struct by matching known field names.
///
/// Fields are matched by their Config struct name (e.g. "openai_api_key", "llm_backend").
/// Unknown fields are silently ignored (already logged as warnings during classification).
fn apply_changes(config: &mut Config, changes: &HashMap<String, serde_json::Value>) {
    for (key, value) in changes {
        let str_val = serialize_json_value(value);
        match key.as_str() {
            // Server (static, but still apply for completeness)
            "host" => config.host = str_val,
            "port" => {
                if let Ok(v) = str_val.parse() {
                    config.port = v;
                }
            }
            "database_url" => config.database_url = str_val,
            "mdns_enabled" => {
                if let Ok(v) = str_val.parse() {
                    config.mdns_enabled = v;
                }
            }
            // LLM
            "llm_backend" => {
                if let Ok(v) = serde_json::from_value(value.clone()) {
                    config.llm_backend = v;
                }
            }
            "llama_url" => config.llama_url = str_val,
            "ollama_url" => config.ollama_url = str_val,
            "ollama_model" => config.ollama_model = str_val,
            "openai_model" => config.openai_model = str_val,
            "openai_api_key" => config.openai_api_key = str_val,
            "azure_openai_api_key" => config.azure_openai_api_key = str_val,
            "azure_openai_endpoint" => config.azure_openai_endpoint = str_val,
            "azure_openai_deployment" => config.azure_openai_deployment = str_val,
            // Embedding (static)
            "embedding_model" => config.embedding_model = str_val,
            "embedding_dimension" => {
                if let Ok(v) = str_val.parse() {
                    config.embedding_dimension = v;
                }
            }
            // Orchestrator
            "capture_enabled" => {
                if let Ok(v) = str_val.parse() {
                    config.capture_enabled = v;
                }
            }
            "capture_interval_seconds" => {
                if let Ok(v) = str_val.parse() {
                    config.capture_interval_seconds = v;
                }
            }
            "checkin_enabled" => {
                if let Ok(v) = str_val.parse() {
                    config.checkin_enabled = v;
                }
            }
            "checkin_times" => config.checkin_times = str_val,
            "checkin_jitter_minutes" => {
                if let Ok(v) = str_val.parse() {
                    config.checkin_jitter_minutes = v;
                }
            }
            "goal_check_interval_seconds" => {
                if let Ok(v) = str_val.parse() {
                    config.goal_check_interval_seconds = v;
                }
            }
            "conversation_inactivity_timeout_seconds" => {
                if let Ok(v) = str_val.parse() {
                    config.conversation_inactivity_timeout_seconds = v;
                }
            }
            "conversation_auto_close_minutes" => {
                if let Ok(v) = str_val.parse() {
                    config.conversation_auto_close_minutes = v;
                }
            }
            "conversation_summary_enabled" => {
                if let Ok(v) = str_val.parse() {
                    config.conversation_summary_enabled = v;
                }
            }
            "tools_enabled" => {
                if let Ok(v) = str_val.parse() {
                    config.tools_enabled = v;
                }
            }
            "decision_cooldown_minutes" => {
                if let Ok(v) = str_val.parse() {
                    config.decision_cooldown_minutes = v;
                }
            }
            "decision_extended_cooldown_minutes" => {
                if let Ok(v) = str_val.parse() {
                    config.decision_extended_cooldown_minutes = v;
                }
            }
            "min_context_for_decision" => {
                if let Ok(v) = str_val.parse() {
                    config.min_context_for_decision = v;
                }
            }
            "semantic_search_limit" => {
                if let Ok(v) = str_val.parse() {
                    config.semantic_search_limit = v;
                }
            }
            "recent_ai_messages_limit" => {
                if let Ok(v) = str_val.parse() {
                    config.recent_ai_messages_limit = v;
                }
            }
            "max_response_tokens" => {
                if let Ok(v) = str_val.parse() {
                    config.max_response_tokens = v;
                }
            }
            "response_temperature" => {
                if let Ok(v) = str_val.parse() {
                    config.response_temperature = v;
                }
            }
            // Learning
            "learning_enabled" => {
                if let Ok(v) = str_val.parse() {
                    config.learning_enabled = v;
                }
            }
            "learning_interval_minutes" => {
                if let Ok(v) = str_val.parse() {
                    config.learning_interval_minutes = v;
                }
            }
            "learning_min_context_items" => {
                if let Ok(v) = str_val.parse() {
                    config.learning_min_context_items = v;
                }
            }
            "learning_max_context_per_cycle" => {
                if let Ok(v) = str_val.parse() {
                    config.learning_max_context_per_cycle = v;
                }
            }
            "learning_max_memories_per_cycle" => {
                if let Ok(v) = str_val.parse() {
                    config.learning_max_memories_per_cycle = v;
                }
            }
            "learning_max_goals_per_cycle" => {
                if let Ok(v) = str_val.parse() {
                    config.learning_max_goals_per_cycle = v;
                }
            }
            "learning_max_memories_per_consolidation" => {
                if let Ok(v) = str_val.parse() {
                    config.learning_max_memories_per_consolidation = v;
                }
            }
            "daily_consolidation_hour" => {
                if let Ok(v) = str_val.parse() {
                    config.daily_consolidation_hour = v;
                }
            }
            // Similarity thresholds
            "similarity_deduplication_threshold" => {
                if let Ok(v) = str_val.parse() {
                    config.similarity_deduplication_threshold = v;
                }
            }
            "similarity_search_recall_threshold" => {
                if let Ok(v) = str_val.parse() {
                    config.similarity_search_recall_threshold = v;
                }
            }
            "similarity_clustering_threshold" => {
                if let Ok(v) = str_val.parse() {
                    config.similarity_clustering_threshold = v;
                }
            }
            // Retention
            "memory_raw_context_retention_days" => {
                if let Ok(v) = str_val.parse() {
                    config.memory_raw_context_retention_days = v;
                }
            }
            "memory_short_term_retention_days" => {
                if let Ok(v) = str_val.parse() {
                    config.memory_short_term_retention_days = v;
                }
            }
            "memory_long_term_retention_days" => {
                if let Ok(v) = str_val.parse() {
                    config.memory_long_term_retention_days = v;
                }
            }
            "goal_retention_days" => {
                if let Ok(v) = str_val.parse() {
                    config.goal_retention_days = v;
                }
            }
            "memory_pruning_enabled" => {
                if let Ok(v) = str_val.parse() {
                    config.memory_pruning_enabled = v;
                }
            }
            _ => {} // Unknown fields ignored
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

// ── Config persistence ─────────────────────────────────────────────────
//
// Persists config key=value pairs to ~/.bobe/.env for survival across restarts.

/// Get the BoBe configuration directory (~/.bobe).
fn bobe_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".bobe")
}

/// Persist config key=value pairs to ~/.bobe/.env.
///
/// Performs atomic write (write temp -> rename) to prevent corruption.
/// API keys must NOT be written here -- use OS keychain instead.
///
/// Returns true on success, false on failure.
fn persist_config(changes: &BTreeMap<String, String>) -> bool {
    // Sanitize values
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
    let lines: Vec<String> = existing.iter().map(|(k, v)| format!("{k}={v}")).collect();
    let content = lines.join("\n") + "\n";

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
