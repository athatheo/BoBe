//! Application configuration. Layered: defaults → config.toml → BOBE_* env vars.

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
pub(crate) struct CodingAgentConfig {
    pub(crate) enabled: bool,
    pub(crate) profiles: String,
    pub(crate) output_dir: String,
    pub(crate) max_concurrent: u32,
    pub(crate) max_runtime_seconds: u64,
}

impl Default for CodingAgentConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            profiles: "[]".into(),
            output_dir: "~/.bobe/agent_output".into(),
            max_concurrent: 2,
            max_runtime_seconds: 1800,
        }
    }
}

/// Application configuration. Layered: defaults → config.toml → BOBE_* env vars.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct Config {
    /// Schema version for future migrations.
    pub(crate) config_version: u32,

    /// Base data directory (default: `~/.bobe`).
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
    pub(crate) coding_agent: CodingAgentConfig,

    pub(crate) seed_default_documents: bool,
    pub(crate) locale_override: Option<String>,
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

    /// Effective locale: config override → `en-US`.
    ///
    /// The frontend is responsible for detecting the system locale and persisting
    /// it as `locale_override` on startup.
    pub(crate) fn effective_locale(&self) -> String {
        if let Some(locale) = self
            .locale_override
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return crate::i18n::resolve_supported_locale(locale);
        }

        crate::i18n::FALLBACK_LOCALE.to_string()
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
