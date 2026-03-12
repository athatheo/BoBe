use std::path::PathBuf;

use figment::Figment;
use figment::providers::{Env, Format, Serialized, Toml};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LlmBackend {
    #[default]
    Ollama,
    Openai,
    #[serde(rename = "azure_openai")]
    AzureOpenai,
    #[serde(rename = "llamacpp")]
    LlamaCpp,
    /// Disabled — no LLM provider needed.
    #[serde(rename = "none")]
    None,
}

impl std::fmt::Display for LlmBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ollama => write!(f, "ollama"),
            Self::Openai => write!(f, "openai"),
            Self::AzureOpenai => write!(f, "azure_openai"),
            Self::LlamaCpp => write!(f, "llamacpp"),
            Self::None => write!(f, "none"),
        }
    }
}

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
pub(crate) struct LlmConfig {
    pub(crate) backend: LlmBackend,
    pub(crate) llama_url: String,
    #[serde(skip_serializing)]
    pub(crate) openai_api_key: SecretString,
    pub(crate) openai_model: String,
    pub(crate) azure_openai_endpoint: String,
    #[serde(skip_serializing)]
    pub(crate) azure_openai_api_key: SecretString,
    pub(crate) azure_openai_deployment: String,
    #[serde(skip_serializing)]
    pub(crate) anthropic_api_key: SecretString,
    /// Context window in tokens. Auto-detected for Ollama; env override takes precedence.
    pub(crate) context_window: u32,
}

impl LlmConfig {
    pub(crate) fn has_openai_key(&self) -> bool {
        !self.openai_api_key.expose_secret().is_empty()
    }

    pub(crate) fn has_azure_key(&self) -> bool {
        !self.azure_openai_api_key.expose_secret().is_empty()
    }

    pub(crate) fn has_anthropic_key(&self) -> bool {
        !self.anthropic_api_key.expose_secret().is_empty()
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            backend: LlmBackend::Ollama,
            llama_url: "http://localhost:8080".into(),
            openai_api_key: SecretString::default(),
            openai_model: "gpt-5-mini".into(),
            azure_openai_endpoint: String::new(),
            azure_openai_api_key: SecretString::default(),
            azure_openai_deployment: String::new(),
            anthropic_api_key: SecretString::default(),
            context_window: 128_000,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct OllamaConfig {
    pub(crate) url: String,
    pub(crate) model: String,
    pub(crate) auto_start: bool,
    pub(crate) auto_pull: bool,
    pub(crate) binary_path: Option<String>,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:11434".into(),
            model: "qwen3:14b".into(),
            auto_start: true,
            auto_pull: true,
            binary_path: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct VisionConfig {
    pub(crate) backend: LlmBackend,
    pub(crate) ollama_model: String,
    pub(crate) openai_model: String,
    pub(crate) azure_openai_deployment: String,
}

impl Default for VisionConfig {
    fn default() -> Self {
        Self {
            backend: LlmBackend::None,
            ollama_model: "qwen3-vl:8b".into(),
            openai_model: "gpt-5-mini".into(),
            azure_openai_deployment: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct EmbeddingConfig {
    pub(crate) model: String,
    pub(crate) dimension: usize,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            model: "nomic-embed-text".into(),
            dimension: 768,
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
    pub(crate) summary_enabled: bool,
}

impl Default for ConversationConfig {
    fn default() -> Self {
        Self {
            inactivity_timeout_seconds: 30,
            auto_close_minutes: 10,
            summary_enabled: true,
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
    pub(crate) min_context: usize,
    pub(crate) semantic_search_limit: i64,
    pub(crate) recent_ai_messages_limit: i64,
    pub(crate) max_response_tokens: u32,
    pub(crate) response_temperature: f32,
}

impl Default for DecisionConfig {
    fn default() -> Self {
        Self {
            cooldown_minutes: 3,
            extended_cooldown_minutes: 5,
            min_context: 2,
            semantic_search_limit: 10,
            recent_ai_messages_limit: 3,
            max_response_tokens: 500,
            response_temperature: 0.7,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct ToolsConfig {
    pub(crate) enabled: bool,
    pub(crate) max_iterations: u32,
    pub(crate) timeout_seconds: f64,
    pub(crate) preselector_enabled: bool,
    pub(crate) allowed_file_dirs: Vec<String>,
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_iterations: 5,
            timeout_seconds: 30.0,
            preselector_enabled: false,
            allowed_file_dirs: vec![],
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
pub(crate) struct LearningConfig {
    pub(crate) enabled: bool,
    pub(crate) interval_minutes: u64,
    pub(crate) min_context_items: u32,
    pub(crate) max_memories_per_cycle: u32,
    pub(crate) max_goals_per_cycle: u32,
    pub(crate) max_context_per_cycle: u32,
    pub(crate) max_memories_per_consolidation: u32,
    pub(crate) daily_consolidation_enabled: bool,
    pub(crate) daily_consolidation_hour: u32,
}

impl Default for LearningConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_minutes: 30,
            min_context_items: 5,
            max_memories_per_cycle: 10,
            max_goals_per_cycle: 1,
            max_context_per_cycle: 50,
            max_memories_per_consolidation: 1000,
            daily_consolidation_enabled: true,
            daily_consolidation_hour: 3,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct SimilarityConfig {
    pub(crate) deduplication_threshold: f64,
    pub(crate) search_recall_threshold: f64,
    pub(crate) clustering_threshold: f64,
}

impl Default for SimilarityConfig {
    fn default() -> Self {
        Self {
            deduplication_threshold: 0.85,
            search_recall_threshold: 0.60,
            clustering_threshold: 0.80,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct MemoryConfig {
    pub(crate) raw_context_retention_days: u32,
    pub(crate) short_term_retention_days: u32,
    pub(crate) long_term_retention_days: u32,
    pub(crate) pruning_enabled: bool,
    pub(crate) goal_retention_days: u32,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            raw_context_retention_days: 7,
            short_term_retention_days: 30,
            long_term_retention_days: 90,
            pruning_enabled: true,
            goal_retention_days: 30,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct GoalsConfig {
    pub(crate) file: Option<String>,
    pub(crate) max_active: u32,
    pub(crate) sync_on_startup: bool,
    pub(crate) sync_interval_minutes: u64,
    pub(crate) check_interval_seconds: f64,
}

impl Default for GoalsConfig {
    fn default() -> Self {
        Self {
            file: None,
            max_active: 10,
            sync_on_startup: true,
            sync_interval_minutes: 60,
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
    pub(crate) poll_interval_seconds: f64,
    pub(crate) max_concurrent: u32,
    pub(crate) max_runtime_seconds: u64,
}

impl Default for CodingAgentConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            profiles: "[]".into(),
            output_dir: "~/.bobe/agent_output".into(),
            poll_interval_seconds: 5.0,
            max_concurrent: 2,
            max_runtime_seconds: 1800,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct GoalWorkerConfig {
    pub(crate) enabled: bool,
    pub(crate) max_concurrent: u32,
    pub(crate) poll_interval_seconds: u64,
    pub(crate) plan_max_steps: u32,
    pub(crate) step_max_turns: u32,
    pub(crate) autonomous: bool,
    pub(crate) ask_user_timeout_seconds: u64,
    pub(crate) approval_timeout_minutes: u64,
    pub(crate) max_failure_retries: u32,
    pub(crate) claude_model: String,
    pub(crate) projects_dir: String,
}

impl Default for GoalWorkerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_concurrent: 1,
            poll_interval_seconds: 60,
            plan_max_steps: 10,
            step_max_turns: 20,
            autonomous: true,
            ask_user_timeout_seconds: 300,
            approval_timeout_minutes: 60,
            max_failure_retries: 3,
            claude_model: "claude-sonnet-4-5-20250929".into(),
            projects_dir: String::new(),
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
    pub(crate) llm: LlmConfig,
    pub(crate) ollama: OllamaConfig,
    pub(crate) vision: VisionConfig,
    pub(crate) embedding: EmbeddingConfig,
    pub(crate) capture: CaptureConfig,
    pub(crate) checkin: CheckinConfig,
    pub(crate) conversation: ConversationConfig,
    pub(crate) logging: LoggingConfig,
    pub(crate) decision: DecisionConfig,
    pub(crate) tools: ToolsConfig,
    pub(crate) mcp: McpConfig,
    pub(crate) learning: LearningConfig,
    pub(crate) similarity: SimilarityConfig,
    pub(crate) memory: MemoryConfig,
    pub(crate) goals: GoalsConfig,
    pub(crate) coding_agent: CodingAgentConfig,
    pub(crate) goal_worker: GoalWorkerConfig,

    pub(crate) soul_file: Option<String>,
    pub(crate) seed_default_documents: bool,
    pub(crate) locale_override: Option<String>,

    /// Prevents re-triggering onboarding when LLM is temporarily unreachable.
    #[serde(default)]
    pub(crate) setup_completed: bool,
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

        let secrets = crate::secrets::load_secrets();
        for (key, value) in &secrets {
            match key.as_str() {
                "llm.openai_api_key" if !config.llm.has_openai_key() => {
                    config.llm.openai_api_key = SecretString::from(value.clone());
                }
                "llm.azure_openai_api_key" if !config.llm.has_azure_key() => {
                    config.llm.azure_openai_api_key = SecretString::from(value.clone());
                }
                "llm.anthropic_api_key" if !config.llm.has_anthropic_key() => {
                    config.llm.anthropic_api_key = SecretString::from(value.clone());
                }
                _ => {}
            }
        }

        Ok(config)
    }

    /// Goals file path, defaulting to `<data_dir>/GOALS.md`.
    pub(crate) fn resolved_goals_file_path(&self) -> PathBuf {
        if let Some(ref path) = self.goals.file {
            PathBuf::from(path)
        } else {
            PathBuf::from(&self.resolved_data_dir()).join("GOALS.md")
        }
    }

    pub(crate) fn resolved_projects_dir(&self) -> PathBuf {
        let raw = if self.goal_worker.projects_dir.is_empty() {
            format!("{}/goal-work", self.resolved_data_dir().display())
        } else {
            self.goal_worker.projects_dir.clone()
        };
        crate::util::paths::expand_tilde(&raw)
    }

    pub(crate) fn resolved_data_dir(&self) -> PathBuf {
        crate::util::paths::expand_tilde(&self.data_dir)
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
