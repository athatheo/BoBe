use std::path::PathBuf;

use figment::Figment;
use figment::providers::{Env, Format, Serialized, Toml};
use serde::{Deserialize, Serialize};

/// Supported LLM backend providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmBackend {
    #[default]
    Ollama,
    Openai,
    #[serde(rename = "azure_openai")]
    AzureOpenai,
    #[serde(rename = "llamacpp")]
    LlamaCpp,
    /// Vision/voice disabled — no LLM provider needed.
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

// ── Sub-structs ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub mdns_enabled: bool,
    pub cors_origins: Vec<String>,
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
pub struct DatabaseConfig {
    pub url: String,
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
pub struct LlmConfig {
    pub backend: LlmBackend,
    pub llama_url: String,
    pub openai_api_key: String,
    pub openai_model: String,
    pub azure_openai_endpoint: String,
    pub azure_openai_api_key: String,
    pub azure_openai_deployment: String,
    pub anthropic_api_key: String,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            backend: LlmBackend::Ollama,
            llama_url: "http://localhost:8080".into(),
            openai_api_key: String::new(),
            openai_model: "gpt-4o-mini".into(),
            azure_openai_endpoint: String::new(),
            azure_openai_api_key: String::new(),
            azure_openai_deployment: String::new(),
            anthropic_api_key: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct OllamaConfig {
    pub url: String,
    pub model: String,
    pub auto_start: bool,
    pub auto_pull: bool,
    pub binary_path: Option<String>,
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
pub struct VisionConfig {
    pub backend: LlmBackend,
    pub ollama_model: String,
    pub openai_model: String,
    pub azure_openai_deployment: String,
}

impl Default for VisionConfig {
    fn default() -> Self {
        Self {
            backend: LlmBackend::None,
            ollama_model: "qwen3-vl:8b".into(),
            openai_model: "gpt-4o-mini".into(),
            azure_openai_deployment: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct EmbeddingConfig {
    pub model: String,
    pub dimension: usize,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            model: "BAAI/bge-small-en-v1.5".into(),
            dimension: 384,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct CaptureConfig {
    pub enabled: bool,
    pub interval_seconds: u64,
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
pub struct CheckinConfig {
    pub enabled: bool,
    pub times: Vec<String>,
    pub jitter_minutes: u32,
    pub interval_minutes: Option<u64>,
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
pub struct ConversationConfig {
    pub inactivity_timeout_seconds: u64,
    pub auto_close_minutes: u64,
    pub summary_enabled: bool,
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
pub struct LoggingConfig {
    pub level: String,
    pub json: bool,
    pub file: Option<String>,
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
pub struct DecisionConfig {
    pub cooldown_minutes: i64,
    pub extended_cooldown_minutes: i64,
    pub min_context: usize,
    pub semantic_search_limit: i64,
    pub recent_ai_messages_limit: i64,
    pub max_response_tokens: u32,
    pub response_temperature: f32,
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
pub struct ToolsConfig {
    pub enabled: bool,
    pub max_iterations: u32,
    pub timeout_seconds: f64,
    pub preselector_enabled: bool,
    pub allowed_file_dirs: Vec<String>,
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
pub struct McpConfig {
    pub enabled: bool,
    pub config_file: Option<String>,
    pub blocked_commands: Vec<String>,
    pub dangerous_env_keys: Vec<String>,
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
pub struct LearningConfig {
    pub enabled: bool,
    pub interval_minutes: u64,
    pub min_context_items: u32,
    pub max_memories_per_cycle: u32,
    pub max_goals_per_cycle: u32,
    pub max_context_per_cycle: u32,
    pub max_memories_per_consolidation: u32,
    pub daily_consolidation_enabled: bool,
    pub daily_consolidation_hour: u32,
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
pub struct SimilarityConfig {
    pub deduplication_threshold: f64,
    pub search_recall_threshold: f64,
    pub clustering_threshold: f64,
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
pub struct MemoryConfig {
    pub raw_context_retention_days: u32,
    pub short_term_retention_days: u32,
    pub long_term_retention_days: u32,
    pub pruning_enabled: bool,
    pub goal_retention_days: u32,
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
pub struct GoalsConfig {
    pub file: Option<String>,
    pub max_active: u32,
    pub sync_on_startup: bool,
    pub sync_interval_minutes: u64,
    pub check_interval_seconds: f64,
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
pub struct CodingAgentConfig {
    pub enabled: bool,
    pub profiles: String,
    pub output_dir: String,
    pub poll_interval_seconds: f64,
    pub max_concurrent: u32,
    pub max_runtime_seconds: u64,
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
pub struct GoalWorkerConfig {
    pub enabled: bool,
    pub max_concurrent: u32,
    pub poll_interval_seconds: u64,
    pub plan_max_steps: u32,
    pub step_max_turns: u32,
    pub autonomous: bool,
    pub ask_user_timeout_seconds: u64,
    pub approval_timeout_minutes: u64,
    pub max_failure_retries: u32,
    pub claude_model: String,
    pub projects_dir: String,
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

// ── Root Config ─────────────────────────────────────────────────────────────

/// Application configuration loaded from config.toml + env var overrides.
///
/// Layered loading: defaults → config.toml → BOBE_* env vars.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    /// Version of the config schema for future migrations.
    pub config_version: u32,

    /// Base data directory (default: ~/.bobe).
    pub data_dir: String,

    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub llm: LlmConfig,
    pub ollama: OllamaConfig,
    pub vision: VisionConfig,
    pub embedding: EmbeddingConfig,
    pub capture: CaptureConfig,
    pub checkin: CheckinConfig,
    pub conversation: ConversationConfig,
    pub logging: LoggingConfig,
    pub decision: DecisionConfig,
    pub tools: ToolsConfig,
    pub mcp: McpConfig,
    pub learning: LearningConfig,
    pub similarity: SimilarityConfig,
    pub memory: MemoryConfig,
    pub goals: GoalsConfig,
    pub coding_agent: CodingAgentConfig,
    pub goal_worker: GoalWorkerConfig,

    pub soul_file: Option<String>,
    pub seed_default_documents: bool,

    /// Set to true after successful setup completion. Prevents re-triggering
    /// the onboarding wizard when the LLM backend is temporarily unreachable.
    #[serde(default)]
    pub setup_completed: bool,
}

impl Config {
    /// Load configuration: defaults → config.toml → BOBE_* env vars.
    pub fn load() -> Result<Self, crate::error::AppError> {
        let data_dir = resolve_data_dir();
        let config_path = PathBuf::from(&data_dir).join("config.toml");

        let defaults = Self {
            data_dir,
            ..Self::default()
        };

        let mut config: Config = Figment::new()
            .merge(Serialized::defaults(&defaults))
            .merge(Toml::file(&config_path))
            .merge(Env::prefixed("BOBE_").split("__"))
            .extract()
            .map_err(|e| crate::error::AppError::Config(e.to_string()))?;

        // Expand ~ in database URL
        if config.database.url.contains('~') {
            config.database.url = expand_tilde(&config.database.url);
        }

        // Load secrets from macOS Keychain (env vars override Keychain)
        let secrets = crate::secrets::load_secrets();
        for (key, value) in &secrets {
            match key.as_str() {
                "llm.openai_api_key" if config.llm.openai_api_key.is_empty() => {
                    config.llm.openai_api_key.clone_from(value);
                }
                "llm.azure_openai_api_key" if config.llm.azure_openai_api_key.is_empty() => {
                    config.llm.azure_openai_api_key.clone_from(value);
                }
                "llm.anthropic_api_key" if config.llm.anthropic_api_key.is_empty() => {
                    config.llm.anthropic_api_key.clone_from(value);
                }
                _ => {}
            }
        }

        Ok(config)
    }

    /// Resolved goals file path, defaulting to <data_dir>/GOALS.md.
    pub fn resolved_goals_file_path(&self) -> PathBuf {
        if let Some(ref path) = self.goals.file {
            PathBuf::from(path)
        } else {
            PathBuf::from(&self.resolved_data_dir()).join("GOALS.md")
        }
    }

    /// Resolved projects directory for goal worker.
    pub fn resolved_projects_dir(&self) -> PathBuf {
        let raw = if self.goal_worker.projects_dir.is_empty() {
            format!("{}/goal-work", self.resolved_data_dir().display())
        } else {
            self.goal_worker.projects_dir.clone()
        };
        PathBuf::from(expand_tilde(&raw))
    }

    /// Resolved data directory (expands ~).
    pub fn resolved_data_dir(&self) -> PathBuf {
        PathBuf::from(expand_tilde(&self.data_dir))
    }

    // ── Compat accessors ────────────────────────────────────────────────
    // These provide flat-style access for code that hasn't migrated yet.
    // They will be removed as code is updated to use nested fields directly.

    pub fn checkin_times_vec(&self) -> &[String] {
        &self.checkin.times
    }

    pub fn mcp_blocked_commands_vec(&self) -> &[String] {
        &self.mcp.blocked_commands
    }

    pub fn mcp_dangerous_env_keys_vec(&self) -> &[String] {
        &self.mcp.dangerous_env_keys
    }

    pub fn cors_origins_vec(&self) -> &[String] {
        &self.server.cors_origins
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn resolve_data_dir() -> String {
    std::env::var("BOBE_DATA_DIR").unwrap_or_else(|_| {
        let home = home_dir();
        format!("{home}/.bobe")
    })
}

fn home_dir() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/tmp".into())
}

fn expand_tilde(s: &str) -> String {
    s.replace('~', &home_dir())
}
