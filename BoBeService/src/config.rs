//! Layered: defaults → config.toml → BOBE_* env vars.

use figment::Figment;
use figment::providers::{Env, Format, Serialized, Toml};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct ServerConfig {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) mdns_enabled: bool,
    pub(crate) cors_origins: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 8766,
            mdns_enabled: false,
            cors_origins: vec!["http://localhost:5175".into()],
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct DatabaseConfig {
    pub(crate) url: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "sqlite:~/.bobe/data/bobrust.db".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct CaptureConfig {
    pub(crate) enabled: bool,
    pub(crate) interval_seconds: u64,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_seconds: 45,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct CheckinConfig {
    pub(crate) enabled: bool,
    pub(crate) times: Vec<String>,
    pub(crate) jitter_minutes: u32,
    pub(crate) interval_minutes: Option<u64>,
}

impl Default for CheckinConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            times: vec!["09:00".into(), "14:00".into(), "19:00".into()],
            jitter_minutes: 5,
            interval_minutes: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct ConversationConfig {
    pub(crate) inactivity_timeout_seconds: u64,
    pub(crate) auto_close_minutes: u64,
}

impl Default for ConversationConfig {
    fn default() -> Self {
        Self {
            inactivity_timeout_seconds: 30,
            auto_close_minutes: 10,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct LoggingConfig {
    pub(crate) level: String,
    pub(crate) json: bool,
    pub(crate) file: Option<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "INFO".into(),
            json: false,
            file: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct DecisionConfig {
    pub(crate) cooldown_minutes: i64,
    pub(crate) extended_cooldown_minutes: i64,
    pub(crate) recent_ai_messages_limit: i64,
}

impl Default for DecisionConfig {
    fn default() -> Self {
        Self {
            cooldown_minutes: 3,
            extended_cooldown_minutes: 5,
            recent_ai_messages_limit: 3,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct McpConfig {
    pub(crate) enabled: bool,
    pub(crate) config_file: Option<String>,
    pub(crate) blocked_commands: Vec<String>,
    pub(crate) dangerous_env_keys: Vec<String>,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            config_file: None,
            blocked_commands: vec![
                "rm".into(),
                "rmdir".into(),
                "dd".into(),
                "mkfs".into(),
                "fdisk".into(),
                "sudo".into(),
                "su".into(),
                "chmod".into(),
                "chown".into(),
                "kill".into(),
                "killall".into(),
                "shutdown".into(),
                "reboot".into(),
                "halt".into(),
            ],
            dangerous_env_keys: vec![
                "LD_PRELOAD".into(),
                "LD_LIBRARY_PATH".into(),
                "DYLD_INSERT_LIBRARIES".into(),
                "DYLD_LIBRARY_PATH".into(),
            ],
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct GoalsConfig {
    pub(crate) check_interval_seconds: f64,
}

impl Default for GoalsConfig {
    fn default() -> Self {
        Self {
            check_interval_seconds: 900.0,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct EngineConfig {
    /// `"copilot_cloud"` or `"local"`. Hot-applied via `ConfigManager`.
    pub(crate) engine: String,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub(crate) provider_base_url: Option<String>,
    /// Alias `provider_text_model` retained for pre-foundation configs on this branch.
    #[serde(
        alias = "provider_text_model",
        deserialize_with = "empty_string_as_none",
        default
    )]
    pub(crate) provider_chat_model: Option<String>,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub(crate) provider_batch_model: Option<String>,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub(crate) provider_vision_model: Option<String>,
    /// Per-class reasoning effort overrides (`"low"|"medium"|"high"|...`). Only meaningful for
    /// models with non-empty `supported_reasoning_efforts`; daemon silently ignores otherwise.
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub(crate) provider_chat_reasoning: Option<String>,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub(crate) provider_batch_reasoning: Option<String>,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub(crate) provider_vision_reasoning: Option<String>,
    /// Only effective in local mode; cloud mode refuses `COPILOT_OFFLINE=true`.
    pub(crate) provider_offline: bool,
}

/// Without this, a persisted `provider_x = ""` would deserialize as `Some("")` and pass an empty model to the SDK.
fn empty_string_as_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<String> = serde::Deserialize::deserialize(deserializer)?;
    Ok(opt.filter(|s| !s.is_empty()))
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            engine: "copilot_cloud".into(),
            provider_base_url: None,
            provider_chat_model: None,
            provider_batch_model: None,
            provider_vision_model: None,
            provider_chat_reasoning: None,
            provider_batch_reasoning: None,
            provider_vision_reasoning: None,
            provider_offline: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct VoiceConfig {
    /// Master toggle. When `false` the overlay hides the mic button and the
    /// daemon's `/voice/stream` returns an Error and closes.
    pub(crate) enabled: bool,
    /// Default Kokoro voice slot. Hello handshake's `voice_id` overrides
    /// this per WS connection. See `voice.rs::DEFAULT_KOKORO_VOICE`.
    pub(crate) persona: String,
    /// Default playback speed (0.5–2.0). Hello handshake's `speed` overrides
    /// this per WS connection. Outside the range gets clamped at synth time.
    pub(crate) speed: f32,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            persona: "af_bella".into(),
            speed: 1.0,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct Config {
    pub(crate) config_version: u32,

    pub(crate) data_dir: String,

    pub(crate) server: ServerConfig,
    pub(crate) database: DatabaseConfig,
    pub(crate) capture: CaptureConfig,
    pub(crate) checkin: CheckinConfig,
    pub(crate) conversation: ConversationConfig,
    pub(crate) logging: LoggingConfig,
    pub(crate) decision: DecisionConfig,
    pub(crate) mcp: McpConfig,
    pub(crate) goals: GoalsConfig,
    pub(crate) engine: EngineConfig,
    pub(crate) voice: VoiceConfig,

    pub(crate) seed_default_documents: bool,
}

impl Config {
    pub(crate) fn load() -> Result<Self, crate::error::AppError> {
        let data_dir = crate::util::paths::bobe_data_dir();
        let config_path = data_dir.join("config.toml");
        let data_dir_str = data_dir.to_string_lossy().into_owned();

        let defaults = Self {
            data_dir: data_dir_str,
            ..Self::default()
        };

        let mut config: Config = Figment::new()
            .merge(Serialized::defaults(&defaults))
            .merge(Toml::file(&config_path))
            .merge(Env::prefixed("BOBE_").split("__"))
            .extract()
            .map_err(|e| crate::error::AppError::Config(e.to_string()))?;

        if config.database.url.contains('~') {
            config.database.url = crate::util::paths::expand_tilde(&config.database.url)
                .to_string_lossy()
                .into_owned();
        }

        Ok(config)
    }

    pub(crate) fn checkin_times_vec(&self) -> &[String] {
        &self.checkin.times
    }

    pub(crate) fn mcp_blocked_commands_vec(&self) -> &[String] {
        &self.mcp.blocked_commands
    }

    pub(crate) fn mcp_dangerous_env_keys_vec(&self) -> &[String] {
        &self.mcp.dangerous_env_keys
    }

    pub(crate) fn cors_origins_vec(&self) -> &[String] {
        &self.server.cors_origins
    }
}
