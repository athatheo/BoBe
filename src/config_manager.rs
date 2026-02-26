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

use crate::config::Config;
use crate::llm::EmbeddingProvider;
use crate::llm::LlmProvider;
use crate::llm::factory::LlmProviderFactory;

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

/// API key fields — hot-swapped and persisted to ~/.bobe/.env.
static LLM_API_KEY_FIELDS: &[&str] = &[
    "openai_api_key",
    "azure_openai_api_key",
    "anthropic_api_key",
];

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
    "tools_max_iterations",
    "tools_timeout_seconds",
    "tools_preselector_enabled",
    "tools_allowed_file_dirs",
    "ollama_auto_start",
    "ollama_auto_pull",
    "log_level",
    "log_json",
    "cors_origins",
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
    "daily_consolidation_enabled",
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

/// Vision LLM fields.
static VISION_FIELDS: &[&str] = &[
    "vision_backend",
    "vision_ollama_model",
    "vision_openai_model",
    "vision_azure_openai_deployment",
];

/// MCP fields.
static MCP_FIELDS: &[&str] = &[
    "mcp_enabled",
    "mcp_config_file",
    "mcp_blocked_commands",
    "mcp_dangerous_env_keys",
];

/// Goals fields.
static GOALS_FIELDS: &[&str] = &[
    "goals_max_active",
    "goals_sync_on_startup",
    "goals_sync_interval_minutes",
];

/// Coding agent fields.
static CODING_AGENT_FIELDS: &[&str] = &[
    "coding_agents_enabled",
    "coding_agent_profiles",
    "coding_agent_output_dir",
    "coding_agent_poll_interval_seconds",
    "coding_agent_max_concurrent",
    "coding_agent_max_runtime_seconds",
];

/// Goal worker fields.
static GOAL_WORKER_FIELDS: &[&str] = &[
    "goal_worker_enabled",
    "goal_worker_max_concurrent",
    "goal_worker_poll_interval_seconds",
    "goal_worker_plan_max_steps",
    "goal_worker_step_max_turns",
    "goal_worker_autonomous",
    "goal_worker_ask_user_timeout_seconds",
    "goal_worker_approval_timeout_minutes",
    "goal_worker_max_failure_retries",
    "goal_worker_claude_model",
    "projects_dir",
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
    embedding_provider: Arc<ArcSwap<Arc<dyn EmbeddingProvider>>>,
    llm_factory: Option<Arc<LlmProviderFactory>>,
}

impl ConfigManager {
    pub fn new(
        config: Arc<ArcSwap<Config>>,
        llm_provider: Arc<ArcSwap<Arc<dyn LlmProvider>>>,
        embedding_provider: Arc<ArcSwap<Arc<dyn EmbeddingProvider>>>,
        llm_factory: Option<Arc<LlmProviderFactory>>,
    ) -> Self {
        Self {
            config,
            llm_provider,
            embedding_provider,
            llm_factory,
        }
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
        let vision_set: HashSet<&str> = VISION_FIELDS.iter().copied().collect();
        let mcp_set: HashSet<&str> = MCP_FIELDS.iter().copied().collect();
        let goals_set: HashSet<&str> = GOALS_FIELDS.iter().copied().collect();
        let coding_agent_set: HashSet<&str> = CODING_AGENT_FIELDS.iter().copied().collect();
        let goal_worker_set: HashSet<&str> = GOAL_WORKER_FIELDS.iter().copied().collect();

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
            } else if api_key_set.contains(k) || llm_set.contains(k) {
                has_llm_changes = true;
                has_config_changes = true;
                env_vars.insert(format!("BOBE_{}", key.to_uppercase()), str_val);
                result.applied_fields.push(key.clone());
            } else if orch_set.contains(k)
                || learning_set.contains(k)
                || similarity_set.contains(k)
                || retention_set.contains(k)
                || vision_set.contains(k)
                || mcp_set.contains(k)
                || goals_set.contains(k)
                || coding_agent_set.contains(k)
                || goal_worker_set.contains(k)
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

        // 4. Rebuild LLM/embedding providers if LLM fields changed
        if has_llm_changes {
            self.rebuild_llm(changes, &mut result);
            self.rebuild_embedding(changes, &mut result);
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
                    if LLM_FIELDS.contains(&key.as_str())
                        || LLM_API_KEY_FIELDS.contains(&key.as_str())
                    {
                        result.applied_fields.retain(|f| f != key);
                        if !result.restart_required_fields.contains(key) {
                            result.restart_required_fields.push(key.clone());
                        }
                    }
                }
            }
        }
    }

    /// Rebuild embedding provider from current config and push to consumers.
    fn rebuild_embedding(
        &self,
        changes: &HashMap<String, serde_json::Value>,
        result: &mut UpdateResult,
    ) {
        let Some(ref factory) = self.llm_factory else {
            warn!("config_manager.no_llm_factory_for_embedding");
            return;
        };

        match factory.create_embedding() {
            Ok(new_provider) => {
                self.embedding_provider.store(Arc::new(new_provider));
                info!("config_manager.embedding_rebuilt");
            }
            Err(e) => {
                error!(error = %e, "config_manager.embedding_rebuild_failed");
                for key in changes.keys() {
                    if LLM_FIELDS.contains(&key.as_str())
                        || LLM_API_KEY_FIELDS.contains(&key.as_str())
                    {
                        result.applied_fields.retain(|f| f != key);
                        if !result.restart_required_fields.contains(key) {
                            result.restart_required_fields.push(key.clone());
                        }
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
    /// Parse `str_val` into the target field's type. Logs a warning on failure.
    macro_rules! parse_and_set {
        ($field:expr, $str_val:expr, $key:expr) => {
            match $str_val.parse() {
                Ok(v) => $field = v,
                Err(_) => warn!(field = $key, value = %$str_val, "config_manager.parse_failed"),
            }
        };
    }

    for (key, value) in changes {
        let str_val = serialize_json_value(value);
        match key.as_str() {
            // Server (static, but still apply for completeness)
            "host" => config.host = str_val,
            "port" => parse_and_set!(config.port, str_val, key),
            "database_url" => config.database_url = str_val,
            "mdns_enabled" => parse_and_set!(config.mdns_enabled, str_val, key),
            // LLM
            "llm_backend" => {
                if let Ok(v) = serde_json::from_value(value.clone()) {
                    config.llm_backend = v;
                } else {
                    warn!(field = key, value = %str_val, "config_manager.parse_failed");
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
            // Ollama
            "ollama_auto_start" => parse_and_set!(config.ollama_auto_start, str_val, key),
            "ollama_auto_pull" => parse_and_set!(config.ollama_auto_pull, str_val, key),
            // Vision LLM
            "vision_backend" => {
                if let Ok(v) = serde_json::from_value(value.clone()) {
                    config.vision_backend = v;
                } else {
                    warn!(field = key, value = %str_val, "config_manager.parse_failed");
                }
            }
            "vision_ollama_model" => config.vision_ollama_model = str_val,
            "vision_openai_model" => config.vision_openai_model = str_val,
            "vision_azure_openai_deployment" => config.vision_azure_openai_deployment = str_val,
            // Embedding (static)
            "embedding_model" => config.embedding_model = str_val,
            "embedding_dimension" => parse_and_set!(config.embedding_dimension, str_val, key),
            // Orchestrator
            "capture_enabled" => parse_and_set!(config.capture_enabled, str_val, key),
            "capture_interval_seconds" => {
                parse_and_set!(config.capture_interval_seconds, str_val, key)
            }
            "checkin_enabled" => parse_and_set!(config.checkin_enabled, str_val, key),
            "checkin_times" => config.checkin_times = str_val,
            "checkin_jitter_minutes" => parse_and_set!(config.checkin_jitter_minutes, str_val, key),
            "goal_check_interval_seconds" => {
                parse_and_set!(config.goal_check_interval_seconds, str_val, key)
            }
            "conversation_inactivity_timeout_seconds" => {
                parse_and_set!(config.conversation_inactivity_timeout_seconds, str_val, key)
            }
            "conversation_auto_close_minutes" => {
                parse_and_set!(config.conversation_auto_close_minutes, str_val, key)
            }
            "conversation_summary_enabled" => {
                parse_and_set!(config.conversation_summary_enabled, str_val, key)
            }
            "tools_enabled" => parse_and_set!(config.tools_enabled, str_val, key),
            "tools_max_iterations" => parse_and_set!(config.tools_max_iterations, str_val, key),
            "tools_timeout_seconds" => parse_and_set!(config.tools_timeout_seconds, str_val, key),
            "tools_preselector_enabled" => {
                parse_and_set!(config.tools_preselector_enabled, str_val, key)
            }
            "tools_allowed_file_dirs" => config.tools_allowed_file_dirs = str_val,
            "decision_cooldown_minutes" => {
                parse_and_set!(config.decision_cooldown_minutes, str_val, key)
            }
            "decision_extended_cooldown_minutes" => {
                parse_and_set!(config.decision_extended_cooldown_minutes, str_val, key)
            }
            "min_context_for_decision" => {
                parse_and_set!(config.min_context_for_decision, str_val, key)
            }
            "semantic_search_limit" => parse_and_set!(config.semantic_search_limit, str_val, key),
            "recent_ai_messages_limit" => {
                parse_and_set!(config.recent_ai_messages_limit, str_val, key)
            }
            "max_response_tokens" => parse_and_set!(config.max_response_tokens, str_val, key),
            "response_temperature" => parse_and_set!(config.response_temperature, str_val, key),
            // MCP
            "mcp_enabled" => parse_and_set!(config.mcp_enabled, str_val, key),
            "mcp_config_file" => {
                config.mcp_config_file = if str_val.is_empty() {
                    None
                } else {
                    Some(str_val)
                };
            }
            "mcp_blocked_commands" => config.mcp_blocked_commands = str_val,
            "mcp_dangerous_env_keys" => config.mcp_dangerous_env_keys = str_val,
            // Goals
            "goals_max_active" => parse_and_set!(config.goals_max_active, str_val, key),
            "goals_sync_on_startup" => parse_and_set!(config.goals_sync_on_startup, str_val, key),
            "goals_sync_interval_minutes" => {
                parse_and_set!(config.goals_sync_interval_minutes, str_val, key)
            }
            // Learning
            "learning_enabled" => parse_and_set!(config.learning_enabled, str_val, key),
            "learning_interval_minutes" => {
                parse_and_set!(config.learning_interval_minutes, str_val, key)
            }
            "learning_min_context_items" => {
                parse_and_set!(config.learning_min_context_items, str_val, key)
            }
            "learning_max_context_per_cycle" => {
                parse_and_set!(config.learning_max_context_per_cycle, str_val, key)
            }
            "learning_max_memories_per_cycle" => {
                parse_and_set!(config.learning_max_memories_per_cycle, str_val, key)
            }
            "learning_max_goals_per_cycle" => {
                parse_and_set!(config.learning_max_goals_per_cycle, str_val, key)
            }
            "learning_max_memories_per_consolidation" => {
                parse_and_set!(config.learning_max_memories_per_consolidation, str_val, key)
            }
            "daily_consolidation_enabled" => {
                parse_and_set!(config.daily_consolidation_enabled, str_val, key)
            }
            "daily_consolidation_hour" => {
                parse_and_set!(config.daily_consolidation_hour, str_val, key)
            }
            // Similarity thresholds
            "similarity_deduplication_threshold" => {
                parse_and_set!(config.similarity_deduplication_threshold, str_val, key)
            }
            "similarity_search_recall_threshold" => {
                parse_and_set!(config.similarity_search_recall_threshold, str_val, key)
            }
            "similarity_clustering_threshold" => {
                parse_and_set!(config.similarity_clustering_threshold, str_val, key)
            }
            // Retention
            "memory_raw_context_retention_days" => {
                parse_and_set!(config.memory_raw_context_retention_days, str_val, key)
            }
            "memory_short_term_retention_days" => {
                parse_and_set!(config.memory_short_term_retention_days, str_val, key)
            }
            "memory_long_term_retention_days" => {
                parse_and_set!(config.memory_long_term_retention_days, str_val, key)
            }
            "goal_retention_days" => parse_and_set!(config.goal_retention_days, str_val, key),
            "memory_pruning_enabled" => parse_and_set!(config.memory_pruning_enabled, str_val, key),
            // Coding agents
            "coding_agents_enabled" => parse_and_set!(config.coding_agents_enabled, str_val, key),
            "coding_agent_profiles" => config.coding_agent_profiles = str_val,
            "coding_agent_output_dir" => config.coding_agent_output_dir = str_val,
            "coding_agent_poll_interval_seconds" => {
                parse_and_set!(config.coding_agent_poll_interval_seconds, str_val, key)
            }
            "coding_agent_max_concurrent" => {
                parse_and_set!(config.coding_agent_max_concurrent, str_val, key)
            }
            "coding_agent_max_runtime_seconds" => {
                parse_and_set!(config.coding_agent_max_runtime_seconds, str_val, key)
            }
            // Goal worker
            "goal_worker_enabled" => parse_and_set!(config.goal_worker_enabled, str_val, key),
            "goal_worker_max_concurrent" => {
                parse_and_set!(config.goal_worker_max_concurrent, str_val, key)
            }
            "goal_worker_poll_interval_seconds" => {
                parse_and_set!(config.goal_worker_poll_interval_seconds, str_val, key)
            }
            "goal_worker_plan_max_steps" => {
                parse_and_set!(config.goal_worker_plan_max_steps, str_val, key)
            }
            "goal_worker_step_max_turns" => {
                parse_and_set!(config.goal_worker_step_max_turns, str_val, key)
            }
            "goal_worker_autonomous" => parse_and_set!(config.goal_worker_autonomous, str_val, key),
            "goal_worker_ask_user_timeout_seconds" => {
                parse_and_set!(config.goal_worker_ask_user_timeout_seconds, str_val, key)
            }
            "goal_worker_approval_timeout_minutes" => {
                parse_and_set!(config.goal_worker_approval_timeout_minutes, str_val, key)
            }
            "goal_worker_max_failure_retries" => {
                parse_and_set!(config.goal_worker_max_failure_retries, str_val, key)
            }
            "goal_worker_claude_model" => config.goal_worker_claude_model = str_val,
            "anthropic_api_key" => config.anthropic_api_key = str_val,
            "projects_dir" => config.projects_dir = str_val,
            // Log
            "log_level" => config.log_level = str_val,
            "log_json" => parse_and_set!(config.log_json, str_val, key),
            // CORS
            "cors_origins" => config.cors_origins = str_val,
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
