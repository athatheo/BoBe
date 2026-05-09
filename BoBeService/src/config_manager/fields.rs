//! Field-name → Config-field mapping for runtime updates.
//! Accepts both dotted keys (`"capture.enabled"`) and flat legacy keys (`"capture_enabled"`).

use std::collections::HashMap;

use tracing::warn;

use crate::config::Config;

pub(crate) fn apply(config: &mut Config, changes: &HashMap<String, serde_json::Value>) {
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
            "decision.recent_ai_messages_limit" => {
                set_parsed!(config.decision.recent_ai_messages_limit, value, k);
            }

            // ── MCP ───────────────────────────────────────────────────
            "mcp.enabled" => set_parsed!(config.mcp.enabled, value, k),
            "mcp.config_file" => set_parsed!(config.mcp.config_file, value, k),
            "mcp.blocked_commands" => set_parsed!(config.mcp.blocked_commands, value, k),
            "mcp.dangerous_env_keys" => set_parsed!(config.mcp.dangerous_env_keys, value, k),

            // ── Goals ─────────────────────────────────────────────────
            "goals.check_interval_seconds" => {
                set_parsed!(config.goals.check_interval_seconds, value, k);
            }

            // ── Coding Agent ──────────────────────────────────────────
            "coding_agent.enabled" => set_parsed!(config.coding_agent.enabled, value, k),
            "coding_agent.profiles" => set_parsed!(config.coding_agent.profiles, value, k),
            "coding_agent.output_dir" => set_parsed!(config.coding_agent.output_dir, value, k),
            "coding_agent.max_concurrent" => {
                set_parsed!(config.coding_agent.max_concurrent, value, k);
            }
            "coding_agent.max_runtime_seconds" => {
                set_parsed!(config.coding_agent.max_runtime_seconds, value, k);
            }

            // ── Top-level ─────────────────────────────────────────────
            "seed_default_documents" => set_parsed!(config.seed_default_documents, value, k),
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
        "capture_enabled" => "capture.enabled",
        "capture_interval_seconds" => "capture.interval_seconds",
        "checkin_enabled" => "checkin.enabled",
        "checkin_times" => "checkin.times",
        "checkin_jitter_minutes" => "checkin.jitter_minutes",
        "checkin_interval_minutes" => "checkin.interval_minutes",
        "conversation_inactivity_timeout_seconds" => "conversation.inactivity_timeout_seconds",
        "conversation_auto_close_minutes" => "conversation.auto_close_minutes",
        "log_level" => "logging.level",
        "log_json" => "logging.json",
        "log_file" => "logging.file",
        "decision_cooldown_minutes" => "decision.cooldown_minutes",
        "decision_extended_cooldown_minutes" => "decision.extended_cooldown_minutes",
        "recent_ai_messages_limit" => "decision.recent_ai_messages_limit",
        "mcp_enabled" => "mcp.enabled",
        "mcp_config_file" => "mcp.config_file",
        "mcp_blocked_commands" => "mcp.blocked_commands",
        "mcp_dangerous_env_keys" => "mcp.dangerous_env_keys",
        "goal_check_interval_seconds" => "goals.check_interval_seconds",
        "coding_agents_enabled" => "coding_agent.enabled",
        "coding_agent_profiles" => "coding_agent.profiles",
        "coding_agent_output_dir" => "coding_agent.output_dir",
        "coding_agent_max_concurrent" => "coding_agent.max_concurrent",
        "coding_agent_max_runtime_seconds" => "coding_agent.max_runtime_seconds",
        "locale_override" => "locale_override",
        other => other,
    }
    .to_string()
}
