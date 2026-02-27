//! Mechanical field-name → Config-field mapping for runtime updates.
//!
//! Separated from the `ConfigManager` orchestration because this is
//! purely data-driven boilerplate that grows 1:1 with `Config` fields.

use std::collections::HashMap;

use tracing::warn;

use crate::config::Config;

use super::persistence::serialize_value;

/// Apply a set of key→JSON changes to a mutable `Config`.
///
/// Unknown keys are silently ignored (already warned during classification).
pub fn apply(config: &mut Config, changes: &HashMap<String, serde_json::Value>) {
    macro_rules! set_parsed {
        ($field:expr, $raw:expr, $key:expr) => {
            match $raw.parse() {
                Ok(v) => $field = v,
                Err(_) => warn!(field = $key, value = %$raw, "config_manager.parse_failed"),
            }
        };
    }

    for (key, value) in changes {
        let s = serialize_value(value);
        match key.as_str() {
            // ── Server (static) ────────────────────────────────────────
            "host" => config.host = s,
            "port" => set_parsed!(config.port, s, key),
            "database_url" => config.database_url = s,
            "mdns_enabled" => set_parsed!(config.mdns_enabled, s, key),

            // ── LLM ───────────────────────────────────────────────────
            "llm_backend" => try_deserialize(&mut config.llm_backend, value, key),
            "llama_url" => config.llama_url = s,
            "ollama_url" => config.ollama_url = s,
            "ollama_model" => config.ollama_model = s,
            "openai_model" => config.openai_model = s,
            "openai_api_key" => config.openai_api_key = s,
            "azure_openai_api_key" => config.azure_openai_api_key = s,
            "azure_openai_endpoint" => config.azure_openai_endpoint = s,
            "azure_openai_deployment" => config.azure_openai_deployment = s,
            "ollama_auto_start" => set_parsed!(config.ollama_auto_start, s, key),
            "ollama_auto_pull" => set_parsed!(config.ollama_auto_pull, s, key),

            // ── Vision ─────────────────────────────────────────────────
            "vision_backend" => try_deserialize(&mut config.vision_backend, value, key),
            "vision_ollama_model" => config.vision_ollama_model = s,
            "vision_openai_model" => config.vision_openai_model = s,
            "vision_azure_openai_deployment" => config.vision_azure_openai_deployment = s,

            // ── Embedding (static) ─────────────────────────────────────
            "embedding_model" => config.embedding_model = s,
            "embedding_dimension" => set_parsed!(config.embedding_dimension, s, key),

            // ── Runtime orchestration ──────────────────────────────────
            "capture_enabled" => set_parsed!(config.capture_enabled, s, key),
            "capture_interval_seconds" => set_parsed!(config.capture_interval_seconds, s, key),
            "checkin_enabled" => set_parsed!(config.checkin_enabled, s, key),
            "checkin_times" => config.checkin_times = s,
            "checkin_jitter_minutes" => set_parsed!(config.checkin_jitter_minutes, s, key),
            "goal_check_interval_seconds" => {
                set_parsed!(config.goal_check_interval_seconds, s, key)
            }
            "conversation_inactivity_timeout_seconds" => {
                set_parsed!(config.conversation_inactivity_timeout_seconds, s, key)
            }
            "conversation_auto_close_minutes" => {
                set_parsed!(config.conversation_auto_close_minutes, s, key)
            }
            "conversation_summary_enabled" => {
                set_parsed!(config.conversation_summary_enabled, s, key)
            }
            "tools_enabled" => set_parsed!(config.tools_enabled, s, key),
            "tools_max_iterations" => set_parsed!(config.tools_max_iterations, s, key),
            "tools_timeout_seconds" => set_parsed!(config.tools_timeout_seconds, s, key),
            "tools_preselector_enabled" => set_parsed!(config.tools_preselector_enabled, s, key),
            "tools_allowed_file_dirs" => config.tools_allowed_file_dirs = s,
            "decision_cooldown_minutes" => set_parsed!(config.decision_cooldown_minutes, s, key),
            "decision_extended_cooldown_minutes" => {
                set_parsed!(config.decision_extended_cooldown_minutes, s, key)
            }
            "min_context_for_decision" => set_parsed!(config.min_context_for_decision, s, key),
            "semantic_search_limit" => set_parsed!(config.semantic_search_limit, s, key),
            "recent_ai_messages_limit" => set_parsed!(config.recent_ai_messages_limit, s, key),
            "max_response_tokens" => set_parsed!(config.max_response_tokens, s, key),
            "response_temperature" => set_parsed!(config.response_temperature, s, key),

            // ── MCP ────────────────────────────────────────────────────
            "mcp_enabled" => set_parsed!(config.mcp_enabled, s, key),
            "mcp_config_file" => config.mcp_config_file = (!s.is_empty()).then_some(s),
            "mcp_blocked_commands" => config.mcp_blocked_commands = s,
            "mcp_dangerous_env_keys" => config.mcp_dangerous_env_keys = s,

            // ── Goals ──────────────────────────────────────────────────
            "goals_max_active" => set_parsed!(config.goals_max_active, s, key),
            "goals_sync_on_startup" => set_parsed!(config.goals_sync_on_startup, s, key),
            "goals_sync_interval_minutes" => {
                set_parsed!(config.goals_sync_interval_minutes, s, key)
            }

            // ── Learning ───────────────────────────────────────────────
            "learning_enabled" => set_parsed!(config.learning_enabled, s, key),
            "learning_interval_minutes" => set_parsed!(config.learning_interval_minutes, s, key),
            "learning_min_context_items" => set_parsed!(config.learning_min_context_items, s, key),
            "learning_max_context_per_cycle" => {
                set_parsed!(config.learning_max_context_per_cycle, s, key)
            }
            "learning_max_memories_per_cycle" => {
                set_parsed!(config.learning_max_memories_per_cycle, s, key)
            }
            "learning_max_goals_per_cycle" => {
                set_parsed!(config.learning_max_goals_per_cycle, s, key)
            }
            "learning_max_memories_per_consolidation" => {
                set_parsed!(config.learning_max_memories_per_consolidation, s, key)
            }
            "daily_consolidation_enabled" => {
                set_parsed!(config.daily_consolidation_enabled, s, key)
            }
            "daily_consolidation_hour" => set_parsed!(config.daily_consolidation_hour, s, key),

            // ── Similarity thresholds ──────────────────────────────────
            "similarity_deduplication_threshold" => {
                set_parsed!(config.similarity_deduplication_threshold, s, key)
            }
            "similarity_search_recall_threshold" => {
                set_parsed!(config.similarity_search_recall_threshold, s, key)
            }
            "similarity_clustering_threshold" => {
                set_parsed!(config.similarity_clustering_threshold, s, key)
            }

            // ── Retention ──────────────────────────────────────────────
            "memory_raw_context_retention_days" => {
                set_parsed!(config.memory_raw_context_retention_days, s, key)
            }
            "memory_short_term_retention_days" => {
                set_parsed!(config.memory_short_term_retention_days, s, key)
            }
            "memory_long_term_retention_days" => {
                set_parsed!(config.memory_long_term_retention_days, s, key)
            }
            "goal_retention_days" => set_parsed!(config.goal_retention_days, s, key),
            "memory_pruning_enabled" => set_parsed!(config.memory_pruning_enabled, s, key),

            // ── Coding agents ──────────────────────────────────────────
            "coding_agents_enabled" => set_parsed!(config.coding_agents_enabled, s, key),
            "coding_agent_profiles" => config.coding_agent_profiles = s,
            "coding_agent_output_dir" => config.coding_agent_output_dir = s,
            "coding_agent_poll_interval_seconds" => {
                set_parsed!(config.coding_agent_poll_interval_seconds, s, key)
            }
            "coding_agent_max_concurrent" => {
                set_parsed!(config.coding_agent_max_concurrent, s, key)
            }
            "coding_agent_max_runtime_seconds" => {
                set_parsed!(config.coding_agent_max_runtime_seconds, s, key)
            }

            // ── Goal worker ────────────────────────────────────────────
            "goal_worker_enabled" => set_parsed!(config.goal_worker_enabled, s, key),
            "goal_worker_max_concurrent" => set_parsed!(config.goal_worker_max_concurrent, s, key),
            "goal_worker_poll_interval_seconds" => {
                set_parsed!(config.goal_worker_poll_interval_seconds, s, key)
            }
            "goal_worker_plan_max_steps" => set_parsed!(config.goal_worker_plan_max_steps, s, key),
            "goal_worker_step_max_turns" => set_parsed!(config.goal_worker_step_max_turns, s, key),
            "goal_worker_autonomous" => set_parsed!(config.goal_worker_autonomous, s, key),
            "goal_worker_ask_user_timeout_seconds" => {
                set_parsed!(config.goal_worker_ask_user_timeout_seconds, s, key)
            }
            "goal_worker_approval_timeout_minutes" => {
                set_parsed!(config.goal_worker_approval_timeout_minutes, s, key)
            }
            "goal_worker_max_failure_retries" => {
                set_parsed!(config.goal_worker_max_failure_retries, s, key)
            }
            "goal_worker_claude_model" => config.goal_worker_claude_model = s,
            "anthropic_api_key" => config.anthropic_api_key = s,
            "projects_dir" => config.projects_dir = s,

            // ── Misc ───────────────────────────────────────────────────
            "log_level" => config.log_level = s,
            "log_json" => set_parsed!(config.log_json, s, key),
            "cors_origins" => config.cors_origins = s,

            _ => {} // Unknown — already warned during classification
        }
    }
}

/// Try to deserialize a JSON value into a target field; warn on failure.
fn try_deserialize<T: serde::de::DeserializeOwned>(
    field: &mut T,
    value: &serde_json::Value,
    key: &str,
) {
    match serde_json::from_value(value.clone()) {
        Ok(v) => *field = v,
        Err(_) => warn!(field = key, "config_manager.parse_failed"),
    }
}
