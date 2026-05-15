//! Accepts dotted keys (`"capture.enabled"`) and flat legacy keys (`"capture_enabled"`).

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

    /// Swift omits `nil` via `JSONEncoder` default, so `""` from the wire means "clear".
    macro_rules! set_opt_string {
        ($field:expr, $value:expr, $key:expr) => {
            match $value {
                serde_json::Value::Null => $field = None,
                serde_json::Value::String(s) if s.is_empty() => $field = None,
                v => match serde_json::from_value::<String>(v.clone()) {
                    Ok(s) => $field = Some(s),
                    Err(_) => warn!(field = $key, "config_manager.parse_failed"),
                },
            }
        };
    }

    for (key, value) in changes {
        let dotted = normalize_key(key);
        let k = dotted.as_str();

        match k {
            "server.host" => set_parsed!(config.server.host, value, k),
            "server.port" => set_parsed!(config.server.port, value, k),
            "server.mdns_enabled" => set_parsed!(config.server.mdns_enabled, value, k),
            "server.cors_origins" => set_parsed!(config.server.cors_origins, value, k),

            "database.url" => set_parsed!(config.database.url, value, k),

            "capture.enabled" => set_parsed!(config.capture.enabled, value, k),
            "capture.interval_seconds" => {
                set_parsed!(config.capture.interval_seconds, value, k);
            }

            "checkin.enabled" => set_parsed!(config.checkin.enabled, value, k),
            "checkin.times" => set_parsed!(config.checkin.times, value, k),
            "checkin.jitter_minutes" => set_parsed!(config.checkin.jitter_minutes, value, k),
            "checkin.interval_minutes" => {
                set_parsed!(config.checkin.interval_minutes, value, k);
            }

            "conversation.inactivity_timeout_seconds" => {
                set_parsed!(config.conversation.inactivity_timeout_seconds, value, k);
            }
            "conversation.auto_close_minutes" => {
                set_parsed!(config.conversation.auto_close_minutes, value, k);
            }

            "logging.level" => set_parsed!(config.logging.level, value, k),
            "logging.json" => set_parsed!(config.logging.json, value, k),
            "logging.file" => set_parsed!(config.logging.file, value, k),

            "decision.cooldown_minutes" => {
                set_parsed!(config.decision.cooldown_minutes, value, k);
            }
            "decision.extended_cooldown_minutes" => {
                set_parsed!(config.decision.extended_cooldown_minutes, value, k);
            }
            "decision.recent_ai_messages_limit" => {
                set_parsed!(config.decision.recent_ai_messages_limit, value, k);
            }

            "mcp.enabled" => set_parsed!(config.mcp.enabled, value, k),
            "mcp.config_file" => set_parsed!(config.mcp.config_file, value, k),
            "mcp.blocked_commands" => set_parsed!(config.mcp.blocked_commands, value, k),
            "mcp.dangerous_env_keys" => set_parsed!(config.mcp.dangerous_env_keys, value, k),

            "goals.check_interval_seconds" => {
                set_parsed!(config.goals.check_interval_seconds, value, k);
            }

            "engine.engine" => set_parsed!(config.engine.engine, value, k),
            "engine.provider_base_url" => {
                set_opt_string!(config.engine.provider_base_url, value, k);
            }
            "engine.provider_chat_model" => {
                set_opt_string!(config.engine.provider_chat_model, value, k);
            }
            "engine.provider_batch_model" => {
                set_opt_string!(config.engine.provider_batch_model, value, k);
            }
            "engine.provider_vision_model" => {
                set_opt_string!(config.engine.provider_vision_model, value, k);
            }
            "engine.provider_chat_reasoning" => {
                set_opt_string!(config.engine.provider_chat_reasoning, value, k);
            }
            "engine.provider_batch_reasoning" => {
                set_opt_string!(config.engine.provider_batch_reasoning, value, k);
            }
            "engine.provider_vision_reasoning" => {
                set_opt_string!(config.engine.provider_vision_reasoning, value, k);
            }
            "engine.provider_offline" => {
                set_parsed!(config.engine.provider_offline, value, k);
            }

            "voice.enabled" => set_parsed!(config.voice.enabled, value, k),
            "voice.persona" => set_parsed!(config.voice.persona, value, k),
            "voice.speed" => set_parsed!(config.voice.speed, value, k),
            "voice.stt_language" => set_parsed!(config.voice.stt_language, value, k),
            "voice.pause_sensitivity" => set_parsed!(config.voice.pause_sensitivity, value, k),
            "voice.show_partial_caption" => {
                set_parsed!(config.voice.show_partial_caption, value, k);
            }

            "seed_default_documents" => set_parsed!(config.seed_default_documents, value, k),

            _ => {}
        }
    }
}

pub(crate) fn normalize_key_pub(key: &str) -> String {
    normalize_key(key)
}

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
        "engine" => "engine.engine",
        "provider_base_url" => "engine.provider_base_url",
        "provider_chat_model" => "engine.provider_chat_model",
        "provider_batch_model" => "engine.provider_batch_model",
        "provider_vision_model" => "engine.provider_vision_model",
        "provider_chat_reasoning" => "engine.provider_chat_reasoning",
        "provider_batch_reasoning" => "engine.provider_batch_reasoning",
        "provider_vision_reasoning" => "engine.provider_vision_reasoning",
        "provider_offline" => "engine.provider_offline",
        "voice_enabled" => "voice.enabled",
        "voice_persona" => "voice.persona",
        "voice_speed" => "voice.speed",
        "voice_stt_language" => "voice.stt_language",
        "voice_pause_sensitivity" => "voice.pause_sensitivity",
        "voice_show_partial_caption" => "voice.show_partial_caption",
        other => other,
    }
    .to_string()
}
