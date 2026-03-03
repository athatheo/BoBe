use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use super::security::{validate_mcp_command, validate_mcp_env};
use crate::error::AppError;

const DEFAULT_MCP_CONFIG_JSON: &str = include_str!("../../assets/defaults/mcp_servers.json");

pub const SECRET_REF_PREFIX: &str = "bobe-secret://";

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

/// Top-level MCP configuration file format (Claude/Copilot style).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfigFile {
    #[serde(rename = "mcpServers", alias = "servers", default)]
    pub mcp_servers: HashMap<String, McpServerEntry>,
}

/// Parsed and validated MCP server configuration.
#[derive(Debug, Clone)]
pub struct McpParsedServer {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub timeout_seconds: f64,
    pub excluded_tools: Vec<String>,
}

/// Resolve the effective MCP config path.
pub fn resolve_mcp_config_path(config_file: Option<&str>) -> Result<PathBuf, AppError> {
    if let Some(path) = config_file
        && !path.trim().is_empty()
    {
        return Ok(expand_tilde(path));
    }

    let home = dirs::home_dir()
        .ok_or_else(|| AppError::Config("Cannot resolve home directory for MCP config".into()))?;
    Ok(home.join(".bobe").join("mcp.json"))
}

/// Ensure MCP config exists, creating it from bundled defaults when missing.
pub fn ensure_mcp_config_exists(config_file: Option<&str>) -> Result<PathBuf, AppError> {
    let path = resolve_mcp_config_path(config_file)?;
    if path.exists() {
        return Ok(path);
    }

    let defaults: McpConfigFile = serde_json::from_str(DEFAULT_MCP_CONFIG_JSON)
        .map_err(|e| AppError::Config(format!("Invalid bundled MCP defaults: {e}")))?;
    save_mcp_config_file(&path, &defaults)?;
    Ok(path)
}

/// Load the entire MCP config file (including disabled servers).
pub fn load_mcp_config_file(path: &Path) -> Result<McpConfigFile, AppError> {
    let content = fs::read_to_string(path)
        .map_err(|e| AppError::Config(format!("Cannot read MCP config {}: {e}", path.display())))?;
    serde_json::from_str(&content)
        .map_err(|e| AppError::Config(format!("Invalid MCP config JSON: {e}")))
}

/// Persist the entire MCP config file atomically.
pub fn save_mcp_config_file(path: &Path, config: &McpConfigFile) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        #[cfg(unix)]
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    }

    let content = serde_json::to_string_pretty(config)?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, format!("{content}\n"))?;

    #[cfg(unix)]
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;

    fs::rename(&tmp, path)?;
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

/// Load and parse enabled MCP server configurations.
pub fn load_mcp_config(
    path: &Path,
    blocked_commands: &[String],
    dangerous_env_keys: &[String],
) -> Result<Vec<McpParsedServer>, AppError> {
    let file = load_mcp_config_file(path)?;
    parse_enabled_servers(file, blocked_commands, dangerous_env_keys)
}

pub fn parse_enabled_servers(
    file: McpConfigFile,
    blocked_commands: &[String],
    dangerous_env_keys: &[String],
) -> Result<Vec<McpParsedServer>, AppError> {
    let mut servers = Vec::new();
    for (name, entry) in file.mcp_servers {
        if !entry.enabled {
            continue;
        }

        validate_mcp_env(&entry.env, dangerous_env_keys)?;

        let command = resolve_value(&entry.command)?;
        validate_mcp_command(&command, blocked_commands)?;
        let args = entry
            .args
            .iter()
            .map(|arg| resolve_value(arg))
            .collect::<Result<Vec<_>, _>>()?;
        let env = entry
            .env
            .iter()
            .map(|(k, v)| resolve_value(v).map(|resolved| (k.clone(), resolved)))
            .collect::<Result<HashMap<_, _>, _>>()?;

        servers.push(McpParsedServer {
            name,
            command,
            args,
            env,
            timeout_seconds: entry.timeout_seconds,
            excluded_tools: entry.excluded_tools,
        });
    }
    Ok(servers)
}

pub fn is_secret_ref(value: &str) -> bool {
    value.starts_with(SECRET_REF_PREFIX)
}

pub fn secret_ref(account: &str) -> String {
    format!("{SECRET_REF_PREFIX}{account}")
}

pub fn secret_account(server_name: &str, env_key: &str) -> String {
    format!(
        "mcp_{}_{}",
        sanitize_secret_component(server_name),
        sanitize_secret_component(env_key)
    )
}

pub fn should_treat_as_secret_key(key: &str) -> bool {
    let upper = key.to_uppercase();
    upper.contains("SECRET")
        || upper.contains("TOKEN")
        || upper.contains("PASSWORD")
        || upper.ends_with("API_KEY")
}

fn resolve_value(raw: &str) -> Result<String, AppError> {
    if let Some(account) = raw.strip_prefix(SECRET_REF_PREFIX) {
        return resolve_secret(account);
    }
    expand_env_templates(raw)
}

fn resolve_secret(account: &str) -> Result<String, AppError> {
    if account.trim().is_empty() {
        return Err(AppError::Config(
            "Invalid empty MCP secret reference".into(),
        ));
    }
    crate::secrets::read_secret(account)
        .ok_or_else(|| AppError::Config(format!("Missing MCP secret for account '{account}'")))
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(path)
}

fn expand_env_templates(input: &str) -> Result<String, AppError> {
    if !input.contains("${") {
        return Ok(input.to_owned());
    }

    let mut out = String::new();
    let mut rest = input;

    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let token_start = start + 2;
        let after_start = &rest[token_start..];
        let end = after_start.find('}').ok_or_else(|| {
            AppError::Config(format!("Invalid MCP template (missing }}): '{input}'"))
        })?;
        let token = &after_start[..end];
        rest = &after_start[end + 1..];

        let (name, default) = if let Some((n, d)) = token.split_once(":-") {
            (n.trim(), Some(d))
        } else {
            (token.trim(), None)
        };

        if name.is_empty() {
            return Err(AppError::Config(format!(
                "Invalid MCP template variable in '{input}'"
            )));
        }

        let value = match std::env::var(name) {
            Ok(v) => v,
            Err(_) => default.map_or_else(
                || {
                    Err(AppError::Config(format!(
                        "Missing environment variable '{name}' required by MCP config"
                    )))
                },
                |d| Ok(d.to_owned()),
            )?,
        };
        out.push_str(&value);
    }

    out.push_str(rest);
    Ok(out)
}

fn sanitize_secret_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    if out.trim_matches('_').is_empty() {
        "value".into()
    } else {
        out
    }
}
