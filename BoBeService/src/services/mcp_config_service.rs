//! MCP config service. Stateless free-fns over a per-call
//! `McpConfigDeps` borrow-bundle, intentionally NOT a struct: nothing
//! here owns state across calls, so an `Arc<McpConfigService>` would be
//! decoration. If this module ever needs cached parsing or a per-instance
//! file watcher, lift `McpConfigDeps` into a struct then.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use arc_swap::ArcSwap;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::warn;

use crate::config::Config;
use crate::copilot::registry::WorkerRegistry;
use crate::error::AppError;
use crate::mcp::config::{self as mcp_config, McpConfigFile, McpServerEntry};
use crate::mcp::security::{validate_mcp_command_with_args, validate_mcp_env};
use crate::secrets::SecretStore;

/// Deps the MCP config service needs. Bundled so each public function
/// can stay short instead of drilling four Arcs through every signature.
/// Constructed once per request from `AppState` in the route handlers
/// (see `api/handlers/tools_mcp.rs`).
pub(crate) struct McpConfigDeps<'a> {
    pub(crate) config: &'a ArcSwap<Config>,
    pub(crate) mcp_config_lock: &'a Mutex<()>,
    pub(crate) workers: &'a Arc<WorkerRegistry>,
    pub(crate) secret_store: &'a Arc<dyn SecretStore>,
}

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
    /// `None` means "indeterminate"; chat session not yet spawned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) status: Option<String>,
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

pub(crate) async fn get_document(
    deps: &McpConfigDeps<'_>,
) -> Result<McpConfigDocumentResponse, AppError> {
    let (_path, file) = load_mcp_file(deps)?;
    let raw_json = redacted_json(&file)?;

    let servers = build_runtime_summaries(deps.workers, &file).await;
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
    deps: &McpConfigDeps<'_>,
    body: &McpConfigMutationRequest,
) -> Result<McpConfigValidateResponse, AppError> {
    let (blocked_cmds, dangerous_keys) = blocked_and_dangerous(deps);

    match parse_and_validate(&body.raw_json, &blocked_cmds, &dangerous_keys) {
        Ok(file) => {
            let normalized = normalize_secrets(file, &body.secret_keys, None)?;
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
    deps: &McpConfigDeps<'_>,
    body: McpConfigMutationRequest,
) -> Result<McpConfigSaveResponse, AppError> {
    let (blocked_cmds, dangerous_keys) = blocked_and_dangerous(deps);
    let file = parse_and_validate(&body.raw_json, &blocked_cmds, &dangerous_keys)?;
    let file = normalize_secrets(file, &body.secret_keys, Some(deps.secret_store.as_ref()))?;

    let guard = deps.mcp_config_lock.lock().await;
    let path = resolve_config_path(deps)?;
    let previous = mcp_config::load_mcp_config_file(&path).ok();

    mcp_config::save_mcp_config_file(&path, &file)?;

    if let Some(ref prev) = previous {
        cleanup_removed_secret_refs(deps.secret_store.as_ref(), prev, &file);
    }

    drop(guard);

    let servers = build_runtime_summaries(deps.workers, &file).await;
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

pub(crate) async fn reset_document(
    deps: &McpConfigDeps<'_>,
) -> Result<McpConfigResetResponse, AppError> {
    let _guard = deps.mcp_config_lock.lock().await;
    let path = resolve_config_path(deps)?;
    let previous = mcp_config::load_mcp_config_file(&path).ok();

    let empty = McpConfigFile {
        mcp_servers: HashMap::new(),
    };
    mcp_config::save_mcp_config_file(&path, &empty)?;

    if let Some(ref prev) = previous {
        cleanup_removed_secret_refs(deps.secret_store.as_ref(), prev, &empty);
    }

    Ok(McpConfigResetResponse {
        message: "MCP config reset".into(),
        raw_json: redacted_json(&empty)?,
        count: 0,
    })
}

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

/// When `secret_store` is `Some`, real values are persisted via the store
/// (Keychain in production); `None` is validate-only mode.
fn normalize_secrets(
    mut file: McpConfigFile,
    secret_keys: &HashMap<String, Vec<String>>,
    secret_store: Option<&dyn SecretStore>,
) -> Result<McpConfigFile, AppError> {
    for (server_name, entry) in &mut file.mcp_servers {
        let explicit = secret_keys.get(server_name).cloned().unwrap_or_default();
        entry.env = build_env_with_refs(server_name, &entry.env, &explicit, secret_store)?;
    }
    Ok(file)
}

fn build_env_with_refs(
    server_name: &str,
    env: &HashMap<String, String>,
    explicit_secret_keys: &[String],
    secret_store: Option<&dyn SecretStore>,
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
            if let Some(store) = secret_store {
                store.store(&account, value).map_err(|e| {
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

async fn build_runtime_summaries(
    workers: &Arc<WorkerRegistry>,
    file: &McpConfigFile,
) -> Vec<McpServerSummary> {
    let mut entries: Vec<(&String, &McpServerEntry)> = file.mcp_servers.iter().collect();
    entries.sort_by_key(|(name, _)| *name);

    let live = workers.live_mcp_servers().await;
    let live_map: HashMap<String, mcp_live::ServerEntry> = live
        .map(|servers| {
            servers
                .into_iter()
                .map(|s| (s.name.clone(), mcp_live::ServerEntry::from_sdk(s)))
                .collect()
        })
        .unwrap_or_default();

    let mut summaries = Vec::with_capacity(entries.len());
    for (name, entry) in entries {
        summaries.push(build_server_summary(name, entry, live_map.get(name)).await);
    }
    summaries
}

async fn build_server_summary(
    name: &str,
    entry: &McpServerEntry,
    live: Option<&mcp_live::ServerEntry>,
) -> McpServerSummary {
    let (mut env_keys, mut secret_env_keys) = env_metadata(entry);
    env_keys.sort();
    secret_env_keys.sort();

    // Per-server tools/tool_count not exposed by SDK v0.1.0; leave defaulted.
    let (connected, status, error) = match live {
        Some(s) => (s.connected, Some(s.status.clone()), s.error.clone()),
        None => (false, None, None),
    };

    McpServerSummary {
        name: name.to_owned(),
        command: entry.command.clone(),
        args: entry.args.clone(),
        enabled: entry.enabled,
        connected,
        status,
        tool_count: 0,
        tools: Vec::new(),
        excluded_tools: entry.excluded_tools.clone(),
        env_keys,
        secret_env_keys,
        error,
    }
}

mod mcp_live {
    use github_copilot_sdk::generated::api_types::{McpServer, McpServerStatus};

    pub(super) struct ServerEntry {
        pub(super) connected: bool,
        pub(super) status: String,
        pub(super) error: Option<String>,
    }

    impl ServerEntry {
        pub(super) fn from_sdk(server: McpServer) -> Self {
            use crate::constants::mcp_status::{
                CONNECTED, DISABLED, FAILED, NEEDS_AUTH, NOT_CONFIGURED, PENDING, UNKNOWN,
            };
            let connected = matches!(server.status, McpServerStatus::Connected);
            let status = match server.status {
                McpServerStatus::Connected => CONNECTED,
                McpServerStatus::Failed => FAILED,
                McpServerStatus::NeedsAuth => NEEDS_AUTH,
                McpServerStatus::Pending => PENDING,
                McpServerStatus::Disabled => DISABLED,
                McpServerStatus::NotConfigured => NOT_CONFIGURED,
                McpServerStatus::Unknown => UNKNOWN,
            }
            .to_string();
            Self {
                connected,
                status,
                error: server.error,
            }
        }
    }
}

fn load_mcp_file(deps: &McpConfigDeps<'_>) -> Result<(PathBuf, McpConfigFile), AppError> {
    let path = resolve_config_path(deps)?;
    let file = mcp_config::load_mcp_config_file(&path)?;
    Ok((path, file))
}

fn resolve_config_path(deps: &McpConfigDeps<'_>) -> Result<PathBuf, AppError> {
    let cfg = deps.config.load();
    mcp_config::ensure_mcp_config_exists(cfg.mcp.config_file.as_deref())
}

fn blocked_and_dangerous(deps: &McpConfigDeps<'_>) -> (Vec<String>, Vec<String>) {
    let cfg = deps.config.load();
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

fn cleanup_removed_secret_refs(
    secret_store: &dyn SecretStore,
    previous: &McpConfigFile,
    current: &McpConfigFile,
) {
    let prev = secret_accounts_in(previous);
    let curr = secret_accounts_in(current);

    for account in prev.difference(&curr) {
        if let Err(e) = secret_store.delete(account) {
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
