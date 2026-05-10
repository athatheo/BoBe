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

/// Engine + LLM model configuration. Determines whether the daemon
/// drives Copilot CLI against GitHub Copilot's cloud (default) or against
/// a local OpenAI-compatible server (typically a managed Ollama).
///
/// One shared CLI subprocess in either mode. Per-session model + provider
/// is set at session-create time via the SDK's `SessionConfig.with_model`
/// / `with_provider` builders (verified in github-copilot-sdk 0.1.0). Each
/// of BoBe's five worker classes maps to one of three model slots:
/// - **chat** (the user-facing dialogue worker)
/// - **batch** (goals / decide / consolidate — autopilot background jobs)
/// - **vision** (capture pipeline — needs a VL model in local mode)
///
/// Engine changes are hot-applied: `ConfigManager` notifies the
/// `WorkerRegistry`, which stops the existing CLI and clears its session
/// caches so subsequent worker accesses re-spawn against the new config.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct EngineConfig {
    /// `"copilot_cloud"` (default) drives GitHub-hosted Copilot models.
    /// `"local"` drives a user-managed OpenAI-compat server (typically
    /// Ollama on `:11434`).
    pub(crate) engine: String,
    /// Local-mode provider URL, e.g. `http://127.0.0.1:11434/v1`. Ignored
    /// in cloud mode (the signed-in user's GitHub Copilot endpoint is
    /// implicit).
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub(crate) provider_base_url: Option<String>,
    /// Model used by the user-facing Chat worker. Cloud: a Copilot-served
    /// model (e.g. `"claude-sonnet-4"`); local: an Ollama tag (e.g.
    /// `"qwen2.5:7b-instruct"`). `None` = use the CLI's default.
    ///
    /// `provider_text_model` is the pre-foundation name (this branch
    /// only — never shipped to users). Aliased here so any local checkout
    /// that still has `provider_text_model = "…"` in `config.toml` loads
    /// the value as the chat model rather than dropping it.
    #[serde(
        alias = "provider_text_model",
        deserialize_with = "empty_string_as_none",
        default
    )]
    pub(crate) provider_chat_model: Option<String>,
    /// Model used by the autopilot batch workers (goals / decide /
    /// consolidate). May be the same as `provider_chat_model` or a
    /// cheaper / faster alternative for headless jobs.
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub(crate) provider_batch_model: Option<String>,
    /// Model used by the Vision worker (capture pipeline). Must support
    /// image inputs in local mode (e.g. `"qwen2.5-vl:7b"`).
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub(crate) provider_vision_model: Option<String>,
    /// Sets `COPILOT_OFFLINE=true` on the spawned CLI to suppress GitHub
    /// telemetry / metadata calls. Only takes effect in `local` engine
    /// mode (cloud mode needs GitHub network access and would refuse to
    /// start with offline=true).
    pub(crate) provider_offline: bool,
}

/// Deserializer that maps `""` and absent fields to `None`. Paired with
/// `config_manager::fields::set_opt_string!` which already normalizes
/// the runtime PATCH path; this handles the on-disk side. Without it,
/// a cleared field that gets persisted as `provider_x = ""` would
/// deserialize back as `Some("")` on next daemon start and pass an
/// empty model name to the SDK.
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
            provider_offline: true,
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
    pub(crate) engine: EngineConfig,

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
