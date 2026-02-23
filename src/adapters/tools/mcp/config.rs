use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::security::{validate_mcp_command, validate_mcp_env};
use crate::error::AppError;

/// Configuration for a single MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerEntry {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: f64,
    #[serde(default)]
    pub excluded_tools: Vec<String>,
}

fn default_true() -> bool {
    true
}

fn default_timeout() -> f64 {
    30.0
}

/// Top-level MCP configuration file format (Claude Desktop compatible).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfigFile {
    #[serde(rename = "mcpServers", default)]
    pub mcp_servers: HashMap<String, McpServerEntry>,
}

/// Parsed and validated MCP server configuration.
#[derive(Debug, Clone)]
pub struct McpParsedServer {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub enabled: bool,
    pub timeout_seconds: f64,
    pub excluded_tools: Vec<String>,
}

/// Load and parse MCP configuration from a file.
pub fn load_mcp_config(path: &Path) -> Result<Vec<McpParsedServer>, AppError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| AppError::Config(format!("Cannot read MCP config {}: {e}", path.display())))?;

    let config: McpConfigFile = serde_json::from_str(&content)
        .map_err(|e| AppError::Config(format!("Invalid MCP config JSON: {e}")))?;

    let mut servers = Vec::new();
    for (name, entry) in config.mcp_servers {
        // Validate command against blocklist
        validate_mcp_command(&entry.command)?;
        // Validate environment variables
        validate_mcp_env(&entry.env)?;

        servers.push(McpParsedServer {
            name,
            command: entry.command,
            args: entry.args,
            env: entry.env,
            enabled: entry.enabled,
            timeout_seconds: entry.timeout_seconds,
            excluded_tools: entry.excluded_tools,
        });
    }
    Ok(servers)
}

/// Load the default MCP config from ~/.bobe/mcp.json.
pub fn load_default_mcp_config() -> Vec<McpParsedServer> {
    let path = default_config_path();
    match path {
        Some(p) if p.exists() => load_mcp_config(&p).unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn default_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".bobe").join("mcp.json"))
}
