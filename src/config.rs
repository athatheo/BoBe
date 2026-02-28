use serde::Deserialize;
use std::fmt;

/// Supported LLM backend providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, serde::Serialize)]
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

impl fmt::Display for LlmBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ollama => write!(f, "ollama"),
            Self::Openai => write!(f, "openai"),
            Self::AzureOpenai => write!(f, "azure_openai"),
            Self::LlamaCpp => write!(f, "llamacpp"),
            Self::None => write!(f, "none"),
        }
    }
}

/// Application configuration loaded from BOBE_* environment variables.
///
/// All fields have sensible defaults. Override via env vars or .env file.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    // ── Server ──────────────────────────────────────────────────────────
    pub host: String,
    pub port: u16,

    // ── Network ─────────────────────────────────────────────────────────
    pub mdns_enabled: bool,

    // ── Database ────────────────────────────────────────────────────────
    pub database_url: String,

    // ── LLM ─────────────────────────────────────────────────────────────
    pub llm_backend: LlmBackend,
    pub llama_url: String,
    pub openai_api_key: String,
    pub openai_model: String,

    // ── Azure OpenAI ────────────────────────────────────────────────────
    pub azure_openai_endpoint: String,
    pub azure_openai_api_key: String,
    pub azure_openai_deployment: String,

    // ── Ollama ──────────────────────────────────────────────────────────
    pub ollama_url: String,
    pub ollama_model: String,
    pub ollama_auto_start: bool,
    pub ollama_auto_pull: bool,
    pub ollama_binary_path: Option<String>,

    // ── Vision LLM ──────────────────────────────────────────────────────
    pub vision_backend: LlmBackend,
    pub vision_ollama_model: String,
    pub vision_openai_model: String,
    pub vision_azure_openai_deployment: String,

    // ── Embedding ───────────────────────────────────────────────────────
    pub embedding_model: String,
    pub embedding_dimension: usize,

    // ── Capture ─────────────────────────────────────────────────────────
    pub capture_interval_seconds: u64,
    pub capture_enabled: bool,

    // ── Soul ────────────────────────────────────────────────────────────
    pub soul_file: Option<String>,

    // ── Check-in ────────────────────────────────────────────────────────
    pub checkin_enabled: bool,
    pub checkin_times: String,
    pub checkin_jitter_minutes: u32,
    pub checkin_interval_minutes: Option<u64>,

    // ── Goal check ──────────────────────────────────────────────────────
    pub goal_check_interval_seconds: f64,

    // ── Conversation lifecycle ──────────────────────────────────────────
    pub conversation_inactivity_timeout_seconds: u64,
    pub conversation_auto_close_minutes: u64,
    pub conversation_summary_enabled: bool,

    // ── Logging ─────────────────────────────────────────────────────────
    pub log_level: String,
    pub log_json: bool,
    pub log_file: Option<String>,

    // ── Decision engine ────────────────────────────────────────────────
    pub decision_cooldown_minutes: i64,
    pub decision_extended_cooldown_minutes: i64,
    pub min_context_for_decision: usize,
    pub semantic_search_limit: i64,
    pub recent_ai_messages_limit: i64,
    pub max_response_tokens: u32,
    pub response_temperature: f32,

    // ── Tools ───────────────────────────────────────────────────────────
    pub tools_enabled: bool,
    pub tools_max_iterations: u32,
    pub tools_timeout_seconds: f64,
    pub tools_preselector_enabled: bool,
    pub tools_allowed_file_dirs: String,

    // ── MCP ─────────────────────────────────────────────────────────────
    pub mcp_enabled: bool,
    pub mcp_config_file: Option<String>,
    pub mcp_blocked_commands: String,
    pub mcp_dangerous_env_keys: String,

    // ── Learning ────────────────────────────────────────────────────────
    pub learning_enabled: bool,
    pub learning_interval_minutes: u64,
    pub learning_min_context_items: u32,
    pub learning_max_memories_per_cycle: u32,
    pub learning_max_goals_per_cycle: u32,
    pub learning_max_context_per_cycle: u32,
    pub learning_max_memories_per_consolidation: u32,

    // ── Similarity thresholds ───────────────────────────────────────────
    pub similarity_deduplication_threshold: f64,
    pub similarity_search_recall_threshold: f64,
    pub similarity_clustering_threshold: f64,

    // ── Memory retention ────────────────────────────────────────────────
    pub memory_raw_context_retention_days: u32,
    pub memory_short_term_retention_days: u32,
    pub memory_long_term_retention_days: u32,
    pub memory_pruning_enabled: bool,
    pub goal_retention_days: u32,

    // ── Goals ───────────────────────────────────────────────────────────
    pub goals_file: Option<String>,
    pub goals_max_active: u32,
    pub goals_sync_on_startup: bool,
    pub goals_sync_interval_minutes: u64,

    // ── Daily consolidation ─────────────────────────────────────────────
    pub daily_consolidation_enabled: bool,
    pub daily_consolidation_hour: u32,

    // ── Coding agents ───────────────────────────────────────────────────
    pub coding_agents_enabled: bool,
    pub coding_agent_profiles: String,
    pub coding_agent_output_dir: String,
    pub coding_agent_poll_interval_seconds: f64,
    pub coding_agent_max_concurrent: u32,
    pub coding_agent_max_runtime_seconds: u64,

    // ── Goal worker (Claude Agent SDK) ─────────────────────────────────
    pub goal_worker_enabled: bool,
    pub goal_worker_max_concurrent: u32,
    pub goal_worker_poll_interval_seconds: u64,
    pub goal_worker_plan_max_steps: u32,
    pub goal_worker_step_max_turns: u32,
    pub goal_worker_autonomous: bool,
    pub goal_worker_ask_user_timeout_seconds: u64,
    pub goal_worker_approval_timeout_minutes: u64,
    pub goal_worker_max_failure_retries: u32,
    pub goal_worker_claude_model: String,
    pub anthropic_api_key: String,
    pub projects_dir: String,

    // ── Database seeding ────────────────────────────────────────────────
    pub seed_default_documents: bool,

    // ── CORS ────────────────────────────────────────────────────────────
    pub cors_origins: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 8766,
            mdns_enabled: false,
            database_url: expand_sqlite_path("sqlite:~/.bobe/data/bobrust.db"),
            llm_backend: LlmBackend::Ollama,
            llama_url: "http://localhost:8080".into(),
            openai_api_key: String::new(),
            openai_model: "gpt-4o-mini".into(),
            azure_openai_endpoint: String::new(),
            azure_openai_api_key: String::new(),
            azure_openai_deployment: String::new(),
            ollama_url: "http://localhost:11434".into(),
            ollama_model: "qwen3:14b".into(),
            ollama_auto_start: true,
            ollama_auto_pull: true,
            ollama_binary_path: None,
            vision_backend: LlmBackend::None,
            vision_ollama_model: "qwen3-vl:8b".into(),
            vision_openai_model: "gpt-4o-mini".into(),
            vision_azure_openai_deployment: String::new(),
            embedding_model: "BAAI/bge-small-en-v1.5".into(),
            embedding_dimension: 384,
            capture_interval_seconds: 45,
            capture_enabled: true,
            soul_file: None,
            checkin_enabled: true,
            checkin_times: "09:00,14:00,19:00".into(),
            checkin_jitter_minutes: 5,
            checkin_interval_minutes: None,
            goal_check_interval_seconds: 900.0,
            conversation_inactivity_timeout_seconds: 30,
            conversation_auto_close_minutes: 10,
            conversation_summary_enabled: true,
            log_level: "INFO".into(),
            log_json: false,
            log_file: None,
            decision_cooldown_minutes: 3,
            decision_extended_cooldown_minutes: 5,
            min_context_for_decision: 2,
            semantic_search_limit: 10,
            recent_ai_messages_limit: 3,
            max_response_tokens: 500,
            response_temperature: 0.7,
            tools_enabled: true,
            tools_max_iterations: 5,
            tools_timeout_seconds: 30.0,
            tools_preselector_enabled: false,
            tools_allowed_file_dirs: String::new(),
            mcp_enabled: true,
            mcp_config_file: None,
            mcp_blocked_commands:
                "rm,rmdir,dd,mkfs,fdisk,sudo,su,chmod,chown,kill,killall,shutdown,reboot,halt"
                    .into(),
            mcp_dangerous_env_keys:
                "LD_PRELOAD,LD_LIBRARY_PATH,DYLD_INSERT_LIBRARIES,DYLD_LIBRARY_PATH".into(),
            learning_enabled: true,
            learning_interval_minutes: 30,
            learning_min_context_items: 5,
            learning_max_memories_per_cycle: 10,
            learning_max_goals_per_cycle: 3,
            learning_max_context_per_cycle: 50,
            learning_max_memories_per_consolidation: 1000,
            similarity_deduplication_threshold: 0.85,
            similarity_search_recall_threshold: 0.60,
            similarity_clustering_threshold: 0.80,
            memory_raw_context_retention_days: 7,
            memory_short_term_retention_days: 30,
            memory_long_term_retention_days: 90,
            memory_pruning_enabled: true,
            goal_retention_days: 30,
            goals_file: None,
            goals_max_active: 10,
            goals_sync_on_startup: true,
            goals_sync_interval_minutes: 60,
            daily_consolidation_enabled: true,
            daily_consolidation_hour: 3,
            coding_agents_enabled: false,
            coding_agent_profiles: "[]".into(),
            coding_agent_output_dir: "~/.bobe/agent_output".into(),
            coding_agent_poll_interval_seconds: 5.0,
            coding_agent_max_concurrent: 2,
            coding_agent_max_runtime_seconds: 1800,
            goal_worker_enabled: false,
            goal_worker_max_concurrent: 1,
            goal_worker_poll_interval_seconds: 60,
            goal_worker_plan_max_steps: 10,
            goal_worker_step_max_turns: 20,
            goal_worker_autonomous: true,
            goal_worker_ask_user_timeout_seconds: 300,
            goal_worker_approval_timeout_minutes: 60,
            goal_worker_max_failure_retries: 3,
            goal_worker_claude_model: "claude-sonnet-4-5-20250929".into(),
            anthropic_api_key: String::new(),
            projects_dir: String::new(),
            seed_default_documents: true,
            cors_origins: "http://localhost:5175".into(),
        }
    }
}

impl Config {
    /// Load configuration from BOBE_* environment variables with defaults.
    pub fn from_env() -> Result<Self, crate::error::AppError> {
        // Load .env files (local first, then user-level)
        let _ = dotenvy::dotenv();
        let home = dirs_path();
        let user_env = format!("{home}/.bobe/.env");
        let _ = dotenvy::from_filename(user_env);

        let mut config: Config = envy::prefixed("BOBE_")
            .from_env()
            .map_err(|e| crate::error::AppError::Config(e.to_string()))?;

        // Expand ~ in sqlite path
        if config.database_url.starts_with("sqlite") && config.database_url.contains('~') {
            config.database_url = expand_sqlite_path(&config.database_url);
        }

        Ok(config)
    }

    /// Parse checkin_times from comma-separated string.
    pub fn checkin_times_vec(&self) -> Vec<String> {
        self.checkin_times
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Parse MCP blocked commands from comma-separated string.
    pub fn mcp_blocked_commands_vec(&self) -> Vec<String> {
        self.mcp_blocked_commands
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Parse MCP dangerous env keys from comma-separated string.
    pub fn mcp_dangerous_env_keys_vec(&self) -> Vec<String> {
        self.mcp_dangerous_env_keys
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Get the resolved goals file path, using default if not specified.
    pub fn resolved_goals_file_path(&self) -> std::path::PathBuf {
        if let Some(ref path) = self.goals_file {
            std::path::PathBuf::from(path)
        } else {
            dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
                .join(".bobe")
                .join("GOALS.md")
        }
    }

    /// Parse CORS origins from comma-separated string.
    pub fn cors_origins_vec(&self) -> Vec<String> {
        self.cors_origins
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Resolved projects directory for goal worker. Defaults to ~/.bobe/goal-work.
    pub fn resolved_projects_dir(&self) -> std::path::PathBuf {
        let raw = if self.projects_dir.is_empty() {
            "~/.bobe/goal-work".to_string()
        } else {
            self.projects_dir.clone()
        };
        let expanded = raw.replace('~', &dirs_path());
        std::path::PathBuf::from(expanded)
    }
}

fn dirs_path() -> String {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| std::env::var("HOME").unwrap_or_else(|_| "/tmp".into()))
}

fn expand_sqlite_path(url: &str) -> String {
    let home = dirs_path();
    url.replace('~', &home)
}
