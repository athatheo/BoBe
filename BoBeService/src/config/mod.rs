//! Layered: defaults → config.toml → BOBE_* env vars.
//!
//! Domain configs are split per file (server.rs, runtime.rs, engine.rs,
//! voice.rs, mcp.rs); this module assembles them into the top-level
//! `Config` and owns the figment loader. Re-exports the per-domain
//! types so consumers continue to import `crate::config::Foo` unchanged.

mod engine;
pub(crate) mod manager;
mod manager_fields;
mod manager_persistence;
mod mcp;
mod runtime;
mod server;
mod voice;

use figment::Figment;
use figment::providers::{Env, Format, Serialized, Toml};
use serde::{Deserialize, Serialize};

pub(crate) use engine::EngineConfig;
pub(crate) use mcp::McpConfig;
pub(crate) use runtime::{
    CaptureConfig, CheckinConfig, ConversationConfig, DecisionConfig, GoalsConfig,
};
pub(crate) use server::{DatabaseConfig, LoggingConfig, ServerConfig};
pub(crate) use voice::VoiceConfig;

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
