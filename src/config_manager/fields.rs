//! Field-name → Config-field mapping for runtime updates.
//! Accepts both dotted keys (`"llm.backend"`) and flat legacy keys (`"llm_backend"`).

use std::collections::HashMap;

use tracing::warn;

use crate::config::Config;

pub fn apply(config: &mut Config, changes: &HashMap<String, serde_json::Value>) {
    macro_rules! set_parsed {
        ($field:expr, $value:expr, $key:expr) => {
            match serde_json::from_value($value.clone()) {
                Ok(v) => $field = v,
                Err(_) => warn!(field = $key, "config_manager.parse_failed"),
            }
        };
    }

    for (key, value) in changes {
        let dotted = normalize_key(key);
        let k = dotted.as_str();

        match k {
            // ── Server ────────────────────────────────────────────────
            "server.host" => set_parsed!(config.server.host, value, k),
            "server.port" => set_parsed!(config.server.port, value, k),
            "server.mdns_enabled" => set_parsed!(config.server.mdns_enabled, value, k),
            "server.cors_origins" => set_parsed!(config.server.cors_origins, value, k),

            // ── Database ──────────────────────────────────────────────
            "database.url" => set_parsed!(config.database.url, value, k),

            // ── LLM ───────────────────────────────────────────────────
            "llm.backend" => set_parsed!(config.llm.backend, value, k),
            "llm.llama_url" => set_parsed!(config.llm.llama_url, value, k),
            "llm.openai_api_key" => set_parsed!(config.llm.openai_api_key, value, k),
            "llm.openai_model" => set_parsed!(config.llm.openai_model, value, k),
            "llm.azure_openai_endpoint" => {
                set_parsed!(config.llm.azure_openai_endpoint, value, k);
            }
            "llm.azure_openai_api_key" => set_parsed!(config.llm.azure_openai_api_key, value, k),
            "llm.azure_openai_deployment" => {
                set_parsed!(config.llm.azure_openai_deployment, value, k);
            }
            "llm.anthropic_api_key" => set_parsed!(config.llm.anthropic_api_key, value, k),
            "llm.context_window" => set_parsed!(config.llm.context_window, value, k),

            // ── Ollama ────────────────────────────────────────────────
            "ollama.url" => set_parsed!(config.ollama.url, value, k),
            "ollama.model" => set_parsed!(config.ollama.model, value, k),
            "ollama.auto_start" => set_parsed!(config.ollama.auto_start, value, k),
            "ollama.auto_pull" => set_parsed!(config.ollama.auto_pull, value, k),
            "ollama.binary_path" => set_parsed!(config.ollama.binary_path, value, k),

            // ── Vision ────────────────────────────────────────────────
            "vision.backend" => set_parsed!(config.vision.backend, value, k),
            "vision.ollama_model" => set_parsed!(config.vision.ollama_model, value, k),
            "vision.openai_model" => set_parsed!(config.vision.openai_model, value, k),
            "vision.azure_openai_deployment" => {
                set_parsed!(config.vision.azure_openai_deployment, value, k);
            }

            // ── Embedding ─────────────────────────────────────────────
            "embedding.model" => set_parsed!(config.embedding.model, value, k),
            "embedding.dimension" => set_parsed!(config.embedding.dimension, value, k),

            // ── Capture ───────────────────────────────────────────────
            "capture.enabled" => set_parsed!(config.capture.enabled, value, k),
            "capture.interval_seconds" => {
                set_parsed!(config.capture.interval_seconds, value, k);
            }

            // ── Check-in ──────────────────────────────────────────────
            "checkin.enabled" => set_parsed!(config.checkin.enabled, value, k),
            "checkin.times" => set_parsed!(config.checkin.times, value, k),
            "checkin.jitter_minutes" => set_parsed!(config.checkin.jitter_minutes, value, k),
            "checkin.interval_minutes" => {
                set_parsed!(config.checkin.interval_minutes, value, k);
            }

            // ── Conversation ──────────────────────────────────────────
            "conversation.inactivity_timeout_seconds" => {
                set_parsed!(config.conversation.inactivity_timeout_seconds, value, k);
            }
            "conversation.auto_close_minutes" => {
                set_parsed!(config.conversation.auto_close_minutes, value, k);
            }
            "conversation.summary_enabled" => {
                set_parsed!(config.conversation.summary_enabled, value, k);
            }

            // ── Logging ───────────────────────────────────────────────
            "logging.level" => set_parsed!(config.logging.level, value, k),
            "logging.json" => set_parsed!(config.logging.json, value, k),
            "logging.file" => set_parsed!(config.logging.file, value, k),

            // ── Decision ──────────────────────────────────────────────
            "decision.cooldown_minutes" => {
                set_parsed!(config.decision.cooldown_minutes, value, k);
            }
            "decision.extended_cooldown_minutes" => {
                set_parsed!(config.decision.extended_cooldown_minutes, value, k);
            }
            "decision.min_context" => set_parsed!(config.decision.min_context, value, k),
            "decision.semantic_search_limit" => {
                set_parsed!(config.decision.semantic_search_limit, value, k);
            }
            "decision.recent_ai_messages_limit" => {
                set_parsed!(config.decision.recent_ai_messages_limit, value, k);
            }
            "decision.max_response_tokens" => {
                set_parsed!(config.decision.max_response_tokens, value, k);
            }
            "decision.response_temperature" => {
                set_parsed!(config.decision.response_temperature, value, k);
            }

            // ── Tools ─────────────────────────────────────────────────
            "tools.enabled" => set_parsed!(config.tools.enabled, value, k),
            "tools.max_iterations" => set_parsed!(config.tools.max_iterations, value, k),
            "tools.timeout_seconds" => set_parsed!(config.tools.timeout_seconds, value, k),
            "tools.preselector_enabled" => {
                set_parsed!(config.tools.preselector_enabled, value, k);
            }
            "tools.allowed_file_dirs" => set_parsed!(config.tools.allowed_file_dirs, value, k),

            // ── MCP ───────────────────────────────────────────────────
            "mcp.enabled" => set_parsed!(config.mcp.enabled, value, k),
            "mcp.config_file" => set_parsed!(config.mcp.config_file, value, k),
            "mcp.blocked_commands" => set_parsed!(config.mcp.blocked_commands, value, k),
            "mcp.dangerous_env_keys" => set_parsed!(config.mcp.dangerous_env_keys, value, k),

            // ── Learning ──────────────────────────────────────────────
            "learning.enabled" => set_parsed!(config.learning.enabled, value, k),
            "learning.interval_minutes" => {
                set_parsed!(config.learning.interval_minutes, value, k);
            }
            "learning.min_context_items" => {
                set_parsed!(config.learning.min_context_items, value, k);
            }
            "learning.max_memories_per_cycle" => {
                set_parsed!(config.learning.max_memories_per_cycle, value, k);
            }
            "learning.max_goals_per_cycle" => {
                set_parsed!(config.learning.max_goals_per_cycle, value, k);
            }
            "learning.max_context_per_cycle" => {
                set_parsed!(config.learning.max_context_per_cycle, value, k);
            }
            "learning.max_memories_per_consolidation" => {
                set_parsed!(config.learning.max_memories_per_consolidation, value, k);
            }
            "learning.daily_consolidation_enabled" => {
                set_parsed!(config.learning.daily_consolidation_enabled, value, k);
            }
            "learning.daily_consolidation_hour" => {
                set_parsed!(config.learning.daily_consolidation_hour, value, k);
            }

            // ── Similarity ────────────────────────────────────────────
            "similarity.deduplication_threshold" => {
                set_parsed!(config.similarity.deduplication_threshold, value, k);
            }
            "similarity.search_recall_threshold" => {
                set_parsed!(config.similarity.search_recall_threshold, value, k);
            }
            "similarity.clustering_threshold" => {
                set_parsed!(config.similarity.clustering_threshold, value, k);
            }

            // ── Memory ────────────────────────────────────────────────
            "memory.raw_context_retention_days" => {
                set_parsed!(config.memory.raw_context_retention_days, value, k);
            }
            "memory.short_term_retention_days" => {
                set_parsed!(config.memory.short_term_retention_days, value, k);
            }
            "memory.long_term_retention_days" => {
                set_parsed!(config.memory.long_term_retention_days, value, k);
            }
            "memory.pruning_enabled" => set_parsed!(config.memory.pruning_enabled, value, k),
            "memory.goal_retention_days" => {
                set_parsed!(config.memory.goal_retention_days, value, k);
            }

            // ── Goals ─────────────────────────────────────────────────
            "goals.file" => set_parsed!(config.goals.file, value, k),
            "goals.max_active" => set_parsed!(config.goals.max_active, value, k),
            "goals.sync_on_startup" => set_parsed!(config.goals.sync_on_startup, value, k),
            "goals.sync_interval_minutes" => {
                set_parsed!(config.goals.sync_interval_minutes, value, k);
            }
            "goals.check_interval_seconds" => {
                set_parsed!(config.goals.check_interval_seconds, value, k);
            }

            // ── Coding Agent ──────────────────────────────────────────
            "coding_agent.enabled" => set_parsed!(config.coding_agent.enabled, value, k),
            "coding_agent.profiles" => set_parsed!(config.coding_agent.profiles, value, k),
            "coding_agent.output_dir" => set_parsed!(config.coding_agent.output_dir, value, k),
            "coding_agent.poll_interval_seconds" => {
                set_parsed!(config.coding_agent.poll_interval_seconds, value, k);
            }
            "coding_agent.max_concurrent" => {
                set_parsed!(config.coding_agent.max_concurrent, value, k);
            }
            "coding_agent.max_runtime_seconds" => {
                set_parsed!(config.coding_agent.max_runtime_seconds, value, k);
            }

            // ── Goal Worker ───────────────────────────────────────────
            "goal_worker.enabled" => set_parsed!(config.goal_worker.enabled, value, k),
            "goal_worker.max_concurrent" => {
                set_parsed!(config.goal_worker.max_concurrent, value, k);
            }
            "goal_worker.poll_interval_seconds" => {
                set_parsed!(config.goal_worker.poll_interval_seconds, value, k);
            }
            "goal_worker.plan_max_steps" => {
                set_parsed!(config.goal_worker.plan_max_steps, value, k);
            }
            "goal_worker.step_max_turns" => {
                set_parsed!(config.goal_worker.step_max_turns, value, k);
            }
            "goal_worker.autonomous" => set_parsed!(config.goal_worker.autonomous, value, k),
            "goal_worker.ask_user_timeout_seconds" => {
                set_parsed!(config.goal_worker.ask_user_timeout_seconds, value, k);
            }
            "goal_worker.approval_timeout_minutes" => {
                set_parsed!(config.goal_worker.approval_timeout_minutes, value, k);
            }
            "goal_worker.max_failure_retries" => {
                set_parsed!(config.goal_worker.max_failure_retries, value, k);
            }
            "goal_worker.claude_model" => set_parsed!(config.goal_worker.claude_model, value, k),
            "goal_worker.projects_dir" => {
                set_parsed!(config.goal_worker.projects_dir, value, k);
            }

            // ── Top-level ─────────────────────────────────────────────
            "soul_file" => set_parsed!(config.soul_file, value, k),
            "seed_default_documents" => set_parsed!(config.seed_default_documents, value, k),
            "setup_completed" => set_parsed!(config.setup_completed, value, k),
            "locale_override" => set_parsed!(config.locale_override, value, k),

            _ => {} // Unknown — already warned during classification
        }
    }
}

pub(crate) fn normalize_key_pub(key: &str) -> String {
    normalize_key(key)
}

/// Normalize flat legacy keys to dotted notation. Already-dotted keys pass through.
fn normalize_key(key: &str) -> String {
    if key.contains('.') {
        return key.to_string();
    }

    match key {
        "host" => "server.host",
        "port" => "server.port",
        "mdns_enabled" => "server.mdns_enabled",
        "cors_origins" => "server.cors_origins",
        "database_url" => "database.url",
        "llm_backend" => "llm.backend",
        "llama_url" => "llm.llama_url",
        "openai_api_key" => "llm.openai_api_key",
        "openai_model" => "llm.openai_model",
        "azure_openai_endpoint" => "llm.azure_openai_endpoint",
        "azure_openai_api_key" => "llm.azure_openai_api_key",
        "azure_openai_deployment" => "llm.azure_openai_deployment",
        "anthropic_api_key" => "llm.anthropic_api_key",
        "llm_context_window" => "llm.context_window",
        "ollama_url" => "ollama.url",
        "ollama_model" => "ollama.model",
        "ollama_auto_start" => "ollama.auto_start",
        "ollama_auto_pull" => "ollama.auto_pull",
        "ollama_binary_path" => "ollama.binary_path",
        "vision_backend" => "vision.backend",
        "vision_ollama_model" => "vision.ollama_model",
        "vision_openai_model" => "vision.openai_model",
        "vision_azure_openai_deployment" => "vision.azure_openai_deployment",
        "embedding_model" => "embedding.model",
        "embedding_dimension" => "embedding.dimension",
        "capture_enabled" => "capture.enabled",
        "capture_interval_seconds" => "capture.interval_seconds",
        "checkin_enabled" => "checkin.enabled",
        "checkin_times" => "checkin.times",
        "checkin_jitter_minutes" => "checkin.jitter_minutes",
        "checkin_interval_minutes" => "checkin.interval_minutes",
        "conversation_inactivity_timeout_seconds" => "conversation.inactivity_timeout_seconds",
        "conversation_auto_close_minutes" => "conversation.auto_close_minutes",
        "conversation_summary_enabled" => "conversation.summary_enabled",
        "log_level" => "logging.level",
        "log_json" => "logging.json",
        "log_file" => "logging.file",
        "decision_cooldown_minutes" => "decision.cooldown_minutes",
        "decision_extended_cooldown_minutes" => "decision.extended_cooldown_minutes",
        "min_context_for_decision" => "decision.min_context",
        "semantic_search_limit" => "decision.semantic_search_limit",
        "recent_ai_messages_limit" => "decision.recent_ai_messages_limit",
        "max_response_tokens" => "decision.max_response_tokens",
        "response_temperature" => "decision.response_temperature",
        "tools_enabled" => "tools.enabled",
        "tools_max_iterations" => "tools.max_iterations",
        "tools_timeout_seconds" => "tools.timeout_seconds",
        "tools_preselector_enabled" => "tools.preselector_enabled",
        "tools_allowed_file_dirs" => "tools.allowed_file_dirs",
        "mcp_enabled" => "mcp.enabled",
        "mcp_config_file" => "mcp.config_file",
        "mcp_blocked_commands" => "mcp.blocked_commands",
        "mcp_dangerous_env_keys" => "mcp.dangerous_env_keys",
        "learning_enabled" => "learning.enabled",
        "learning_interval_minutes" => "learning.interval_minutes",
        "learning_min_context_items" => "learning.min_context_items",
        "learning_max_memories_per_cycle" => "learning.max_memories_per_cycle",
        "learning_max_goals_per_cycle" => "learning.max_goals_per_cycle",
        "learning_max_context_per_cycle" => "learning.max_context_per_cycle",
        "learning_max_memories_per_consolidation" => "learning.max_memories_per_consolidation",
        "daily_consolidation_enabled" => "learning.daily_consolidation_enabled",
        "daily_consolidation_hour" => "learning.daily_consolidation_hour",
        "similarity_deduplication_threshold" => "similarity.deduplication_threshold",
        "similarity_search_recall_threshold" => "similarity.search_recall_threshold",
        "similarity_clustering_threshold" => "similarity.clustering_threshold",
        "memory_raw_context_retention_days" => "memory.raw_context_retention_days",
        "memory_short_term_retention_days" => "memory.short_term_retention_days",
        "memory_long_term_retention_days" => "memory.long_term_retention_days",
        "memory_pruning_enabled" => "memory.pruning_enabled",
        "goal_retention_days" => "memory.goal_retention_days",
        "goals_file" => "goals.file",
        "goals_max_active" => "goals.max_active",
        "goals_sync_on_startup" => "goals.sync_on_startup",
        "goals_sync_interval_minutes" => "goals.sync_interval_minutes",
        "goal_check_interval_seconds" => "goals.check_interval_seconds",
        "coding_agents_enabled" => "coding_agent.enabled",
        "coding_agent_profiles" => "coding_agent.profiles",
        "coding_agent_output_dir" => "coding_agent.output_dir",
        "coding_agent_poll_interval_seconds" => "coding_agent.poll_interval_seconds",
        "coding_agent_max_concurrent" => "coding_agent.max_concurrent",
        "coding_agent_max_runtime_seconds" => "coding_agent.max_runtime_seconds",
        "goal_worker_enabled" => "goal_worker.enabled",
        "goal_worker_max_concurrent" => "goal_worker.max_concurrent",
        "goal_worker_poll_interval_seconds" => "goal_worker.poll_interval_seconds",
        "goal_worker_plan_max_steps" => "goal_worker.plan_max_steps",
        "goal_worker_step_max_turns" => "goal_worker.step_max_turns",
        "goal_worker_autonomous" => "goal_worker.autonomous",
        "goal_worker_ask_user_timeout_seconds" => "goal_worker.ask_user_timeout_seconds",
        "goal_worker_approval_timeout_minutes" => "goal_worker.approval_timeout_minutes",
        "goal_worker_max_failure_retries" => "goal_worker.max_failure_retries",
        "goal_worker_claude_model" => "goal_worker.claude_model",
        "projects_dir" => "goal_worker.projects_dir",
        "locale_override" => "locale_override",
        other => other,
    }
    .to_string()
}
