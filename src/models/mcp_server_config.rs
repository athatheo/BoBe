use chrono::{DateTime, Utc};
use uuid::Uuid;

/// MCP server configuration stored in database.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct McpServerConfig {
    pub id: Uuid,
    pub server_name: String,
    pub command: String,
    /// JSON-encoded `Vec<String>`.
    pub args: String,
    /// JSON-encoded `HashMap<String, String>`.
    pub env: String,
    pub enabled: bool,
    pub timeout_seconds: f64,
    pub is_default: bool,
    pub last_connected_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    /// JSON-encoded list of tool names to exclude.
    pub excluded_tools: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl McpServerConfig {
    pub fn new(server_name: String, command: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            server_name,
            command,
            args: "[]".to_owned(),
            env: "{}".to_owned(),
            enabled: true,
            timeout_seconds: 30.0,
            is_default: false,
            last_connected_at: None,
            last_error: None,
            excluded_tools: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn args_vec(&self) -> Vec<String> {
        match serde_json::from_str(&self.args) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, raw = %self.args, "mcp_server_config.args_parse_failed");
                Vec::new()
            }
        }
    }

    pub fn env_map(&self) -> std::collections::HashMap<String, String> {
        match serde_json::from_str(&self.env) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, raw = %self.env, "mcp_server_config.env_parse_failed");
                std::collections::HashMap::new()
            }
        }
    }
}
