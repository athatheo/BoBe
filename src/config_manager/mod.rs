//! Runtime configuration manager.
//!
//! Coordinates hot-swappable settings across application components:
//! - Config is an immutable snapshot behind `ArcSwap`
//! - Changes create a new `Config` by merging, then swap atomically
//! - LLM/embedding providers are rebuilt when backend/model fields change
//! - Non-secret values are persisted to `~/.bobe/config.toml`

mod fields;
pub(crate) mod persistence;

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
// Keys use dotted notation matching the nested Config structure.

static STATIC_FIELDS: &[&str] = &[
    "server.host",
    "server.port",
    "database.url",
    "embedding.model",
    "embedding.dimension",
    "llm.llama_url",
    "ollama.url",
    "ollama.binary_path",
    "server.mdns_enabled",
    "logging.file",
];

static LLM_FIELDS: &[&str] = &[
    "llm.backend",
    "ollama.model",
    "llm.openai_model",
    "llm.azure_openai_endpoint",
    "llm.azure_openai_deployment",
];

static LLM_KEY_FIELDS: &[&str] = &[
    "llm.openai_api_key",
    "llm.azure_openai_api_key",
    "llm.anthropic_api_key",
];

static HOT_SWAP_FIELDS: &[&str] = &[
    "capture.enabled",
    "capture.interval_seconds",
    "decision.cooldown_minutes",
    "decision.extended_cooldown_minutes",
    "decision.min_context",
    "decision.semantic_search_limit",
    "decision.recent_ai_messages_limit",
    "decision.max_response_tokens",
    "decision.response_temperature",
    "checkin.enabled",
    "checkin.times",
    "checkin.jitter_minutes",
    "goals.check_interval_seconds",
    "conversation.inactivity_timeout_seconds",
    "conversation.auto_close_minutes",
    "conversation.summary_enabled",
    "tools.enabled",
    "tools.max_iterations",
    "tools.timeout_seconds",
    "tools.preselector_enabled",
    "tools.allowed_file_dirs",
    "ollama.auto_start",
    "ollama.auto_pull",
    "logging.level",
    "logging.json",
    "server.cors_origins",
    "learning.enabled",
    "learning.interval_minutes",
    "learning.min_context_items",
    "learning.max_context_per_cycle",
    "learning.max_memories_per_cycle",
    "learning.max_goals_per_cycle",
    "learning.max_memories_per_consolidation",
    "learning.daily_consolidation_hour",
    "learning.daily_consolidation_enabled",
    "similarity.deduplication_threshold",
    "similarity.search_recall_threshold",
    "similarity.clustering_threshold",
    "memory.raw_context_retention_days",
    "memory.short_term_retention_days",
    "memory.long_term_retention_days",
    "memory.goal_retention_days",
    "memory.pruning_enabled",
    "vision.backend",
    "vision.ollama_model",
    "vision.openai_model",
    "vision.azure_openai_deployment",
    "mcp.enabled",
    "mcp.config_file",
    "mcp.blocked_commands",
    "mcp.dangerous_env_keys",
    "goals.max_active",
    "goals.sync_on_startup",
    "goals.sync_interval_minutes",
    "coding_agent.enabled",
    "coding_agent.profiles",
    "coding_agent.output_dir",
    "coding_agent.poll_interval_seconds",
    "coding_agent.max_concurrent",
    "coding_agent.max_runtime_seconds",
    "goal_worker.enabled",
    "goal_worker.max_concurrent",
    "goal_worker.poll_interval_seconds",
    "goal_worker.plan_max_steps",
    "goal_worker.step_max_turns",
    "goal_worker.autonomous",
    "goal_worker.ask_user_timeout_seconds",
    "goal_worker.approval_timeout_minutes",
    "goal_worker.max_failure_retries",
    "goal_worker.claude_model",
    "goal_worker.projects_dir",
    "checkin.interval_minutes",
    "goals.file",
    "soul_file",
    "seed_default_documents",
    "setup_completed",
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
    ///
    /// Accepts both flat legacy keys and dotted keys.
    pub fn update(&self, changes: &HashMap<String, serde_json::Value>) -> UpdateResult {
        let mut result = UpdateResult {
            applied_fields: Vec::new(),
            restart_required_fields: Vec::new(),
            persist_failed: false,
        };

        let static_set: HashSet<&str> = STATIC_FIELDS.iter().copied().collect();
        let llm_set: HashSet<&str> = LLM_FIELDS.iter().copied().collect();
        let key_set: HashSet<&str> = LLM_KEY_FIELDS.iter().copied().collect();
        let hot_set: HashSet<&str> = HOT_SWAP_FIELDS.iter().copied().collect();

        let mut toml_changes = BTreeMap::new();
        let mut has_llm_changes = false;
        let mut has_config_changes = false;

        for (key, value) in changes {
            // Normalize to dotted key for classification and persistence
            let dotted = fields::normalize_key_pub(key);
            let k = dotted.as_str();

            if static_set.contains(k) {
                result.restart_required_fields.push(key.clone());
                toml_changes.insert(dotted, value.clone());
            } else if key_set.contains(k) || llm_set.contains(k) {
                has_llm_changes = true;
                has_config_changes = true;
                // Secret fields go to Keychain, not config.toml
                if !crate::secrets::is_secret_field(k) {
                    toml_changes.insert(dotted, value.clone());
                }
                result.applied_fields.push(key.clone());
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

        // Store secret fields in macOS Keychain
        for (key, value) in changes {
            let dotted = fields::normalize_key_pub(key);
            if crate::secrets::is_secret_field(&dotted)
                && let Some(s) = value.as_str()
            {
                let account = dotted.split('.').next_back().unwrap_or(&dotted);
                if let Err(e) = crate::secrets::store_secret(account, s) {
                    warn!(field = %dotted, error = %e, "config_manager.keychain_store_failed");
                }
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
            .or_else(|| changes.get("llm.backend"))
            .and_then(|v| v.as_str())
            .and_then(|s| serde_json::from_value(serde_json::Value::String(s.to_owned())).ok())
            .unwrap_or(config.llm.backend);

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
        // Check both the raw key and normalized dotted form
        let dotted = fields::normalize_key_pub(key);
        if fields.contains(key.as_str()) || fields.contains(dotted.as_str()) {
            result.applied_fields.retain(|f| f != key);
            if !result.restart_required_fields.contains(key) {
                result.restart_required_fields.push(key.clone());
            }
        }
    }
}
