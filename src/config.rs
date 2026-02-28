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
            max_goals_per_cycle: 3,
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
}

impl Config {
    /// Load configuration: defaults → config.toml → BOBE_* env vars.
    pub fn load() -> Result<Self, crate::error::AppError> {
        let data_dir = resolve_data_dir();
        let config_path = PathBuf::from(&data_dir).join("config.toml");

        // Try one-time migration from .env if config.toml doesn't exist
        let env_path = PathBuf::from(&data_dir).join(".env");
        if !config_path.exists() && env_path.exists() {
            if let Err(e) = migrate_env_to_toml(&env_path, &config_path) {
                tracing::warn!(error = %e, "config.migration_failed, using defaults + env vars");
            }
        }

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
                    config.llm.openai_api_key = value.clone();
                }
                "llm.azure_openai_api_key" if config.llm.azure_openai_api_key.is_empty() => {
                    config.llm.azure_openai_api_key = value.clone();
                }
                "llm.anthropic_api_key" if config.llm.anthropic_api_key.is_empty() => {
                    config.llm.anthropic_api_key = value.clone();
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

/// One-time migration from .env to config.toml.
fn migrate_env_to_toml(
    env_path: &std::path::Path,
    config_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(env_path)?;
    let mut doc = toml_edit::DocumentMut::new();

    // Parse .env key=value pairs and map to nested TOML structure
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_lowercase();
        let value = value.trim();

        // Strip BOBE_ prefix if present
        let key = key.strip_prefix("bobe_").unwrap_or(&key);

        map_flat_key_to_toml(&mut doc, key, value);
    }

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Write config.toml
    let toml_content = format!(
        "# BoBe configuration - migrated from .env\n# See docs for all options\nconfig_version = 1\n\n{}",
        doc
    );
    std::fs::write(config_path, toml_content)?;

    // Rename .env to .env.migrated
    let migrated = env_path.with_extension("env.migrated");
    std::fs::rename(env_path, migrated)?;

    tracing::info!("config.migrated_env_to_toml");
    Ok(())
}

/// Map a flat BOBE_* key to the correct nested TOML path.
fn map_flat_key_to_toml(doc: &mut toml_edit::DocumentMut, key: &str, value: &str) {
    // Mapping from flat env var keys (without BOBE_ prefix) to nested TOML paths
    let (section, field) = match key {
        "host" => ("server", "host"),
        "port" => ("server", "port"),
        "mdns_enabled" => ("server", "mdns_enabled"),
        "cors_origins" => ("server", "cors_origins"),
        "database_url" => ("database", "url"),
        "llm_backend" => ("llm", "backend"),
        "llama_url" => ("llm", "llama_url"),
        "openai_api_key" => ("llm", "openai_api_key"),
        "openai_model" => ("llm", "openai_model"),
        "azure_openai_endpoint" => ("llm", "azure_openai_endpoint"),
        "azure_openai_api_key" => ("llm", "azure_openai_api_key"),
        "azure_openai_deployment" => ("llm", "azure_openai_deployment"),
        "anthropic_api_key" => ("llm", "anthropic_api_key"),
        "ollama_url" => ("ollama", "url"),
        "ollama_model" => ("ollama", "model"),
        "ollama_auto_start" => ("ollama", "auto_start"),
        "ollama_auto_pull" => ("ollama", "auto_pull"),
        "ollama_binary_path" => ("ollama", "binary_path"),
        "vision_backend" => ("vision", "backend"),
        "vision_ollama_model" => ("vision", "ollama_model"),
        "vision_openai_model" => ("vision", "openai_model"),
        "vision_azure_openai_deployment" => ("vision", "azure_openai_deployment"),
        "embedding_model" => ("embedding", "model"),
        "embedding_dimension" => ("embedding", "dimension"),
        "capture_enabled" => ("capture", "enabled"),
        "capture_interval_seconds" => ("capture", "interval_seconds"),
        "checkin_enabled" => ("checkin", "enabled"),
        "checkin_times" => ("checkin", "times"),
        "checkin_jitter_minutes" => ("checkin", "jitter_minutes"),
        "checkin_interval_minutes" => ("checkin", "interval_minutes"),
        "conversation_inactivity_timeout_seconds" => ("conversation", "inactivity_timeout_seconds"),
        "conversation_auto_close_minutes" => ("conversation", "auto_close_minutes"),
        "conversation_summary_enabled" => ("conversation", "summary_enabled"),
        "log_level" => ("logging", "level"),
        "log_json" => ("logging", "json"),
        "log_file" => ("logging", "file"),
        "decision_cooldown_minutes" => ("decision", "cooldown_minutes"),
        "decision_extended_cooldown_minutes" => ("decision", "extended_cooldown_minutes"),
        "min_context_for_decision" => ("decision", "min_context"),
        "semantic_search_limit" => ("decision", "semantic_search_limit"),
        "recent_ai_messages_limit" => ("decision", "recent_ai_messages_limit"),
        "max_response_tokens" => ("decision", "max_response_tokens"),
        "response_temperature" => ("decision", "response_temperature"),
        "tools_enabled" => ("tools", "enabled"),
        "tools_max_iterations" => ("tools", "max_iterations"),
        "tools_timeout_seconds" => ("tools", "timeout_seconds"),
        "tools_preselector_enabled" => ("tools", "preselector_enabled"),
        "tools_allowed_file_dirs" => ("tools", "allowed_file_dirs"),
        "mcp_enabled" => ("mcp", "enabled"),
        "mcp_config_file" => ("mcp", "config_file"),
        "mcp_blocked_commands" => ("mcp", "blocked_commands"),
        "mcp_dangerous_env_keys" => ("mcp", "dangerous_env_keys"),
        "learning_enabled" => ("learning", "enabled"),
        "learning_interval_minutes" => ("learning", "interval_minutes"),
        "learning_min_context_items" => ("learning", "min_context_items"),
        "learning_max_memories_per_cycle" => ("learning", "max_memories_per_cycle"),
        "learning_max_goals_per_cycle" => ("learning", "max_goals_per_cycle"),
        "learning_max_context_per_cycle" => ("learning", "max_context_per_cycle"),
        "learning_max_memories_per_consolidation" => ("learning", "max_memories_per_consolidation"),
        "daily_consolidation_enabled" => ("learning", "daily_consolidation_enabled"),
        "daily_consolidation_hour" => ("learning", "daily_consolidation_hour"),
        "similarity_deduplication_threshold" => ("similarity", "deduplication_threshold"),
        "similarity_search_recall_threshold" => ("similarity", "search_recall_threshold"),
        "similarity_clustering_threshold" => ("similarity", "clustering_threshold"),
        "memory_raw_context_retention_days" => ("memory", "raw_context_retention_days"),
        "memory_short_term_retention_days" => ("memory", "short_term_retention_days"),
        "memory_long_term_retention_days" => ("memory", "long_term_retention_days"),
        "memory_pruning_enabled" => ("memory", "pruning_enabled"),
        "goal_retention_days" => ("memory", "goal_retention_days"),
        "goals_file" | "goals_file_path" => ("goals", "file"),
        "goals_max_active" => ("goals", "max_active"),
        "goals_sync_on_startup" => ("goals", "sync_on_startup"),
        "goals_sync_interval_minutes" => ("goals", "sync_interval_minutes"),
        "goal_check_interval_seconds" => ("goals", "check_interval_seconds"),
        "coding_agents_enabled" => ("coding_agent", "enabled"),
        "coding_agent_profiles" => ("coding_agent", "profiles"),
        "coding_agent_output_dir" => ("coding_agent", "output_dir"),
        "coding_agent_poll_interval_seconds" => ("coding_agent", "poll_interval_seconds"),
        "coding_agent_max_concurrent" => ("coding_agent", "max_concurrent"),
        "coding_agent_max_runtime_seconds" => ("coding_agent", "max_runtime_seconds"),
        "goal_worker_enabled" => ("goal_worker", "enabled"),
        "goal_worker_max_concurrent" => ("goal_worker", "max_concurrent"),
        "goal_worker_poll_interval_seconds" => ("goal_worker", "poll_interval_seconds"),
        "goal_worker_plan_max_steps" => ("goal_worker", "plan_max_steps"),
        "goal_worker_step_max_turns" => ("goal_worker", "step_max_turns"),
        "goal_worker_autonomous" => ("goal_worker", "autonomous"),
        "goal_worker_ask_user_timeout_seconds" => ("goal_worker", "ask_user_timeout_seconds"),
        "goal_worker_approval_timeout_minutes" => ("goal_worker", "approval_timeout_minutes"),
        "goal_worker_max_failure_retries" => ("goal_worker", "max_failure_retries"),
        "goal_worker_claude_model" => ("goal_worker", "claude_model"),
        "projects_dir" => ("goal_worker", "projects_dir"),
        "soul_file" | "seed_default_documents" => {
            // Top-level fields
            if let Some(v) = parse_toml_value(value) {
                doc[key] = v;
            }
            return;
        }
        _ => return, // Unknown keys silently skipped during migration
    };

    // Ensure the section table exists
    if doc.get(section).is_none() {
        doc[section] = toml_edit::Item::Table(toml_edit::Table::new());
    }

    // Handle special array fields (comma-separated → TOML arrays)
    let is_array_field = matches!(
        (section, field),
        ("server", "cors_origins")
            | ("checkin", "times")
            | ("mcp", "blocked_commands" | "dangerous_env_keys")
            | ("tools", "allowed_file_dirs")
    );

    if is_array_field {
        let mut arr = toml_edit::Array::new();
        for item in value.split(',') {
            let trimmed = item.trim();
            if !trimmed.is_empty() {
                arr.push(trimmed);
            }
        }
        doc[section][field] = toml_edit::value(arr);
    } else if let Some(v) = parse_toml_value(value) {
        doc[section][field] = v;
    }
}

/// Try to parse a string value into the most appropriate TOML type.
fn parse_toml_value(value: &str) -> Option<toml_edit::Item> {
    if value == "true" {
        return Some(toml_edit::value(true));
    }
    if value == "false" {
        return Some(toml_edit::value(false));
    }
    if let Ok(n) = value.parse::<i64>() {
        return Some(toml_edit::value(n));
    }
    if let Ok(f) = value.parse::<f64>() {
        return Some(toml_edit::value(f));
    }
    Some(toml_edit::value(value))
}
