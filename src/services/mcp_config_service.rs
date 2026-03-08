//! MCP configuration service — schema validation, secret normalization,
//! file persistence, and runtime adapter reload.
//!
//! Design follows Claude Code / VS Code Copilot patterns:
//! - **Validate** is schema-only (pure, no subprocesses, no side effects).
//! - **Save** persists config + reloads adapter; server connections happen
//!   lazily during adapter reload (per-server failures are non-blocking).
//! - **GET** returns persisted config + live runtime state from the adapter.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::app_state::AppState;
use crate::error::AppError;
use crate::tools::mcp::config::{self as mcp_config, McpConfigFile, McpServerEntry};
use crate::tools::mcp::security::{validate_mcp_command_with_args, validate_mcp_env};

// ── Response / request DTOs ────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub(crate) struct McpToolMetadata {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) excluded: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct McpServerSummary {
    pub(crate) name: String,
    pub(crate) command: String,
    pub(crate) args: Vec<String>,
    pub(crate) enabled: bool,
    pub(crate) connected: bool,
    pub(crate) tool_count: usize,
    pub(crate) tools: Vec<McpToolMetadata>,
    pub(crate) excluded_tools: Vec<String>,
    pub(crate) env_keys: Vec<String>,
    pub(crate) secret_env_keys: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct McpConfigDocumentResponse {
    pub(crate) raw_json: String,
    pub(crate) servers: Vec<McpServerSummary>,
    pub(crate) count: usize,
    pub(crate) connected_count: usize,
}

#[derive(Debug, Deserialize)]
pub(crate) struct McpConfigMutationRequest {
    pub(crate) raw_json: String,
    #[serde(default)]
    pub(crate) secret_keys: HashMap<String, Vec<String>>,
}

#[derive(Debug, Serialize)]
pub(crate) struct McpConfigValidateResponse {
    pub(crate) valid: bool,
    pub(crate) normalized_json: String,
    pub(crate) server_count: usize,
    pub(crate) errors: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct McpConfigSaveResponse {
    pub(crate) message: String,
    pub(crate) raw_json: String,
    pub(crate) servers: Vec<McpServerSummary>,
    pub(crate) count: usize,
    pub(crate) connected_count: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct McpConfigResetResponse {
    pub(crate) message: String,
    pub(crate) raw_json: String,
    pub(crate) count: usize,
}

// ── Public API ─────────────────────────────────────────────────────────────

pub(crate) async fn get_document(state: &AppState) -> Result<McpConfigDocumentResponse, AppError> {
    let (_path, file) = load_mcp_file(state)?;
    let raw_json = redacted_json(&file)?;

    let servers = build_runtime_summaries(state, &file).await;
    let count = servers.len();
    let connected_count = servers.iter().filter(|s| s.connected).count();

    Ok(McpConfigDocumentResponse {
        raw_json,
        servers,
        count,
        connected_count,
    })
}

pub(crate) fn validate_document(
    state: &AppState,
    body: &McpConfigMutationRequest,
) -> Result<McpConfigValidateResponse, AppError> {
    let (blocked_cmds, dangerous_keys) = blocked_and_dangerous(state);

    match parse_and_validate(&body.raw_json, &blocked_cmds, &dangerous_keys) {
        Ok(file) => {
            let normalized = normalize_secrets(file, &body.secret_keys, false)?;
            Ok(McpConfigValidateResponse {
                valid: true,
                normalized_json: redacted_json(&normalized)?,
                server_count: normalized.mcp_servers.len(),
                errors: Vec::new(),
            })
        }
        Err(AppError::Validation(msg)) => Ok(McpConfigValidateResponse {
            valid: false,
            normalized_json: String::new(),
            server_count: 0,
            errors: vec![msg],
        }),
        Err(e) => Err(e),
    }
}

pub(crate) async fn save_document(
    state: &AppState,
    body: McpConfigMutationRequest,
) -> Result<McpConfigSaveResponse, AppError> {
    let (blocked_cmds, dangerous_keys) = blocked_and_dangerous(state);
    let file = parse_and_validate(&body.raw_json, &blocked_cmds, &dangerous_keys)?;
    let file = normalize_secrets(file, &body.secret_keys, true)?;

    let guard = state.mcp_config_lock.lock().await;
    let path = resolve_config_path(state)?;
    let previous = mcp_config::load_mcp_config_file(&path).ok();

    mcp_config::save_mcp_config_file(&path, &file)?;

    if let Some(adapter) = state.mcp_tool_adapter.as_ref()
        && let Err(e) = adapter.reload_from_config().await
    {
        warn!(error = %e, "mcp_config.adapter_reload_partial_failure");
    }

    if let Err(e) = state.tool_registry.refresh_index().await {
        warn!(error = %e, "mcp_config.tool_index_refresh_failed");
    }

    if let Some(ref prev) = previous {
        cleanup_removed_secret_refs(prev, &file);
    }

    drop(guard);

    let servers = build_runtime_summaries(state, &file).await;
    let count = servers.len();
    let connected_count = servers.iter().filter(|s| s.connected).count();

    Ok(McpConfigSaveResponse {
        message: "MCP config saved".into(),
        raw_json: redacted_json(&file)?,
        servers,
        count,
        connected_count,
    })
}

pub(crate) async fn reset_document(state: &AppState) -> Result<McpConfigResetResponse, AppError> {
    let _guard = state.mcp_config_lock.lock().await;
    let path = resolve_config_path(state)?;
    let previous = mcp_config::load_mcp_config_file(&path).ok();

    let empty = McpConfigFile {
        mcp_servers: HashMap::new(),
    };
    mcp_config::save_mcp_config_file(&path, &empty)?;

    if let Some(adapter) = state.mcp_tool_adapter.as_ref()
        && let Err(e) = adapter.reload_from_config().await
    {
        warn!(error = %e, "mcp_config.adapter_reload_failed_on_reset");
    }

    if let Err(e) = state.tool_registry.refresh_index().await {
        warn!(error = %e, "mcp_config.tool_index_refresh_failed_on_reset");
    }

    if let Some(ref prev) = previous {
        cleanup_removed_secret_refs(prev, &empty);
    }

    Ok(McpConfigResetResponse {
        message: "MCP config reset".into(),
        raw_json: redacted_json(&empty)?,
        count: 0,
    })
}

// ── Internals ──────────────────────────────────────────────────────────────

fn parse_and_validate(
    raw_json: &str,
    blocked_commands: &[String],
    dangerous_env_keys: &[String],
) -> Result<McpConfigFile, AppError> {
    if raw_json.trim().is_empty() {
        return Err(AppError::Validation("raw_json must not be empty".into()));
    }

    let file: McpConfigFile = serde_json::from_str(raw_json)
        .map_err(|e| AppError::Validation(format!("Invalid MCP JSON: {e}")))?;

    for (name, entry) in &file.mcp_servers {
        if name.trim().is_empty() {
            return Err(AppError::Validation(
                "MCP server name must not be empty".into(),
            ));
        }
        if entry.command.trim().is_empty() {
            return Err(AppError::Validation(format!(
                "MCP server '{name}' command must not be empty"
            )));
        }
        validate_mcp_command_with_args(&entry.command, &entry.args, blocked_commands)?;
        validate_mcp_env(&entry.env, dangerous_env_keys)?;
    }

    Ok(file)
}

/// When `persist` is true, real values are stored in Keychain.
fn normalize_secrets(
    mut file: McpConfigFile,
    secret_keys: &HashMap<String, Vec<String>>,
    persist: bool,
) -> Result<McpConfigFile, AppError> {
    for (server_name, entry) in &mut file.mcp_servers {
        let explicit = secret_keys.get(server_name).cloned().unwrap_or_default();
        entry.env = build_env_with_refs(server_name, &entry.env, &explicit, persist)?;
    }
    Ok(file)
}

fn build_env_with_refs(
    server_name: &str,
    env: &HashMap<String, String>,
    explicit_secret_keys: &[String],
    persist: bool,
) -> Result<HashMap<String, String>, AppError> {
    let explicit: HashSet<&str> = explicit_secret_keys.iter().map(String::as_str).collect();
    let mut out = HashMap::with_capacity(env.len());

    for (key, value) in env {
        if mcp_config::is_secret_ref(value) || value.trim().is_empty() || value.contains("${") {
            out.insert(key.clone(), value.clone());
            continue;
        }

        if explicit.contains(key.as_str()) || mcp_config::should_treat_as_secret_key(key) {
            let account = mcp_config::secret_account(server_name, key);
            if persist {
                crate::secrets::store_secret(&account, value).map_err(|e| {
                    AppError::Config(format!("Failed to store MCP secret '{key}': {e}"))
                })?;
            }
            out.insert(key.clone(), mcp_config::secret_ref(&account));
        } else {
            out.insert(key.clone(), value.clone());
        }
    }

    Ok(out)
}

async fn build_runtime_summaries(state: &AppState, file: &McpConfigFile) -> Vec<McpServerSummary> {
    let mut entries: Vec<(&String, &McpServerEntry)> = file.mcp_servers.iter().collect();
    entries.sort_by_key(|(name, _)| *name);

    let mut summaries = Vec::with_capacity(entries.len());
    for (name, entry) in entries {
        summaries.push(build_server_summary(state, name, entry).await);
    }
    summaries
}

async fn build_server_summary(
    state: &AppState,
    name: &str,
    entry: &McpServerEntry,
) -> McpServerSummary {
    let (mut env_keys, mut secret_env_keys) = env_metadata(entry);
    env_keys.sort();
    secret_env_keys.sort();

    let excluded: HashSet<&str> = entry.excluded_tools.iter().map(String::as_str).collect();

    let (connected, tools, runtime_error) = match state.mcp_tool_adapter.as_ref() {
        Some(adapter) => match adapter.get_raw_tools_for_server(name).await {
            Ok(raw) => {
                let mut t: Vec<McpToolMetadata> = raw
                    .iter()
                    .map(|tool| McpToolMetadata {
                        name: tool.name.clone(),
                        description: tool.description.clone(),
                        excluded: excluded.contains(tool.name.as_str()),
                    })
                    .collect();
                t.sort_by(|a, b| a.name.cmp(&b.name));
                (true, t, adapter.get_server_error(name).await)
            }
            Err(e) => (
                false,
                Vec::new(),
                adapter
                    .get_server_error(name)
                    .await
                    .or_else(|| Some(e.to_string())),
            ),
        },
        None => (false, Vec::new(), None),
    };

    let tool_count = tools.iter().filter(|t| !t.excluded).count();

    McpServerSummary {
        name: name.to_owned(),
        command: entry.command.clone(),
        args: entry.args.clone(),
        enabled: entry.enabled,
        connected,
        tool_count,
        tools,
        excluded_tools: entry.excluded_tools.clone(),
        env_keys,
        secret_env_keys,
        error: runtime_error,
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn load_mcp_file(state: &AppState) -> Result<(PathBuf, McpConfigFile), AppError> {
    let path = resolve_config_path(state)?;
    let file = mcp_config::load_mcp_config_file(&path)?;
    Ok((path, file))
}

fn resolve_config_path(state: &AppState) -> Result<PathBuf, AppError> {
    let cfg = state.config();
    mcp_config::ensure_mcp_config_exists(cfg.mcp.config_file.as_deref())
}

fn blocked_and_dangerous(state: &AppState) -> (Vec<String>, Vec<String>) {
    let cfg = state.config();
    (
        cfg.mcp_blocked_commands_vec().to_vec(),
        cfg.mcp_dangerous_env_keys_vec().to_vec(),
    )
}

fn redacted_json(file: &McpConfigFile) -> Result<String, AppError> {
    Ok(format!("{}\n", serde_json::to_string_pretty(file)?))
}

fn env_metadata(entry: &McpServerEntry) -> (Vec<String>, Vec<String>) {
    let env_keys: Vec<String> = entry.env.keys().cloned().collect();
    let secret_env_keys: Vec<String> = entry
        .env
        .iter()
        .filter(|(_, v)| mcp_config::is_secret_ref(v))
        .map(|(k, _)| k.clone())
        .collect();
    (env_keys, secret_env_keys)
}

fn cleanup_removed_secret_refs(previous: &McpConfigFile, current: &McpConfigFile) {
    let prev = secret_accounts_in(previous);
    let curr = secret_accounts_in(current);

    for account in prev.difference(&curr) {
        if let Err(e) = crate::secrets::delete_secret(account) {
            warn!(account, error = %e, "mcp_config.cleanup_secret_failed");
        }
    }
}

fn secret_accounts_in(file: &McpConfigFile) -> HashSet<String> {
    file.mcp_servers
        .values()
        .flat_map(|e| e.env.values())
        .filter_map(|v| {
            v.strip_prefix(mcp_config::SECRET_REF_PREFIX)
                .map(ToOwned::to_owned)
        })
        .collect()
}
