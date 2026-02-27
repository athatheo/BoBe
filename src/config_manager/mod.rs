//! Runtime configuration manager.
//!
//! Coordinates hot-swappable settings across application components:
//! - Config is an immutable snapshot behind `ArcSwap`
//! - Changes create a new `Config` by merging, then swap atomically
//! - LLM/embedding providers are rebuilt when backend/model fields change
//! - Non-secret values are persisted to `~/.bobe/.env`

mod fields;
mod persistence;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use arc_swap::ArcSwap;
use tracing::{error, info, warn};

use crate::config::Config;
use crate::llm::factory::LlmProviderFactory;
use crate::llm::{EmbeddingProvider, LlmProvider};

// ── Field classification tables ────────────────────────────────────────────
//
// Each table determines how the `update` method handles a given key:
// restart-required, hot-swap config only, or hot-swap + provider rebuild.

static STATIC_FIELDS: &[&str] = &[
    "host",
    "port",
    "database_url",
    "embedding_model",
    "embedding_dimension",
    "llama_url",
    "ollama_url",
    "mdns_enabled",
];

static LLM_FIELDS: &[&str] = &[
    "llm_backend",
    "ollama_model",
    "openai_model",
    "azure_openai_endpoint",
    "azure_openai_deployment",
];

static LLM_KEY_FIELDS: &[&str] = &[
    "openai_api_key",
    "azure_openai_api_key",
    "anthropic_api_key",
];

static HOT_SWAP_FIELDS: &[&[&str]] = &[
    &[
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
    ],
    &[
        "learning_enabled",
        "learning_interval_minutes",
        "learning_min_context_items",
        "learning_max_context_per_cycle",
        "learning_max_memories_per_cycle",
        "learning_max_goals_per_cycle",
        "learning_max_memories_per_consolidation",
        "daily_consolidation_hour",
        "daily_consolidation_enabled",
    ],
    &[
        "similarity_deduplication_threshold",
        "similarity_search_recall_threshold",
        "similarity_clustering_threshold",
    ],
    &[
        "memory_raw_context_retention_days",
        "memory_short_term_retention_days",
        "memory_long_term_retention_days",
        "goal_retention_days",
        "memory_pruning_enabled",
    ],
    &[
        "vision_backend",
        "vision_ollama_model",
        "vision_openai_model",
        "vision_azure_openai_deployment",
    ],
    &[
        "mcp_enabled",
        "mcp_config_file",
        "mcp_blocked_commands",
        "mcp_dangerous_env_keys",
    ],
    &[
        "goals_max_active",
        "goals_sync_on_startup",
        "goals_sync_interval_minutes",
    ],
    &[
        "coding_agents_enabled",
        "coding_agent_profiles",
        "coding_agent_output_dir",
        "coding_agent_poll_interval_seconds",
        "coding_agent_max_concurrent",
        "coding_agent_max_runtime_seconds",
    ],
    &[
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
    ],
];

// ── Public types ───────────────────────────────────────────────────────────

/// Result of a runtime config update.
#[derive(Debug)]
pub struct UpdateResult {
    pub applied_fields: Vec<String>,
    pub restart_required_fields: Vec<String>,
    pub persist_failed: bool,
}

/// Coordinates runtime config changes.
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

    /// Classify, persist, apply in-memory, and optionally rebuild providers.
    pub fn update(&self, changes: &HashMap<String, serde_json::Value>) -> UpdateResult {
        let mut result = UpdateResult {
            applied_fields: Vec::new(),
            restart_required_fields: Vec::new(),
            persist_failed: false,
        };

        let static_set: HashSet<&str> = STATIC_FIELDS.iter().copied().collect();
        let llm_set: HashSet<&str> = LLM_FIELDS.iter().copied().collect();
        let key_set: HashSet<&str> = LLM_KEY_FIELDS.iter().copied().collect();
        let hot_set: HashSet<&str> = HOT_SWAP_FIELDS
            .iter()
            .flat_map(|s| s.iter().copied())
            .collect();

        let mut env_vars = BTreeMap::new();
        let mut has_llm_changes = false;
        let mut has_config_changes = false;

        for (key, value) in changes {
            let k = key.as_str();
            let s = persistence::serialize_value(value);

            if static_set.contains(k) {
                result.restart_required_fields.push(key.clone());
                env_vars.insert(format!("BOBE_{}", key.to_uppercase()), s);
            } else if key_set.contains(k) || llm_set.contains(k) {
                has_llm_changes = true;
                has_config_changes = true;
                env_vars.insert(format!("BOBE_{}", key.to_uppercase()), s);
                result.applied_fields.push(key.clone());
            } else if hot_set.contains(k) {
                has_config_changes = true;
                env_vars.insert(format!("BOBE_{}", key.to_uppercase()), s);
                result.applied_fields.push(key.clone());
            } else {
                warn!(field = k, "config_manager.unknown_field");
                env_vars.insert(format!("BOBE_{}", key.to_uppercase()), s);
                result.applied_fields.push(key.clone());
            }
        }

        if !env_vars.is_empty() && !persistence::persist(&env_vars) {
            result.persist_failed = true;
        }

        if has_config_changes {
            let current = self.config.load();
            let mut new_config = (**current).clone();
            fields::apply(&mut new_config, changes);
            self.config.store(Arc::new(new_config));
            info!("config_manager.config_swapped");
        }

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

    fn rebuild_llm(&self, changes: &HashMap<String, serde_json::Value>, result: &mut UpdateResult) {
        let Some(ref factory) = self.llm_factory else {
            warn!("config_manager.no_llm_factory");
            return;
        };

        let config = self.config.load();
        let backend = changes
            .get("llm_backend")
            .and_then(|v| v.as_str())
            .and_then(|s| serde_json::from_value(serde_json::Value::String(s.to_owned())).ok())
            .unwrap_or(config.llm_backend);

        match factory.create(backend) {
            Ok(p) => {
                self.llm_provider.store(Arc::new(p));
                info!(backend = %backend, "config_manager.llm_rebuilt");
            }
            Err(e) => {
                error!(error = %e, backend = %backend, "config_manager.llm_rebuild_failed");
                demote_to_restart(changes, result, &[LLM_FIELDS, LLM_KEY_FIELDS]);
            }
        }
    }

    fn rebuild_embedding(
        &self,
        changes: &HashMap<String, serde_json::Value>,
        result: &mut UpdateResult,
    ) {
        let Some(ref factory) = self.llm_factory else {
            return;
        };

        match factory.create_embedding() {
            Ok(p) => {
                self.embedding_provider.store(Arc::new(p));
                info!("config_manager.embedding_rebuilt");
            }
            Err(e) => {
                error!(error = %e, "config_manager.embedding_rebuild_failed");
                demote_to_restart(changes, result, &[LLM_FIELDS, LLM_KEY_FIELDS]);
            }
        }
    }
}

/// Move changed keys from `applied` to `restart_required` when a rebuild fails.
fn demote_to_restart(
    changes: &HashMap<String, serde_json::Value>,
    result: &mut UpdateResult,
    field_groups: &[&[&str]],
) {
    let fields: HashSet<&str> = field_groups
        .iter()
        .flat_map(|g| g.iter().copied())
        .collect();
    for key in changes.keys() {
        if fields.contains(key.as_str()) {
            result.applied_fields.retain(|f| f != key);
            if !result.restart_required_fields.contains(key) {
                result.restart_required_fields.push(key.clone());
            }
        }
    }
}
