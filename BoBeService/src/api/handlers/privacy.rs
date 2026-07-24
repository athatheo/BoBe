use std::collections::HashSet;
use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use serde::Serialize;

use crate::app_state::AppState;
use crate::error::AppError;
use crate::mcp::config::{self as mcp_config, McpConfigFile};

#[derive(Serialize)]
pub(crate) struct PrivacyPurgeResponse {
    message: &'static str,
    deleted: Deleted,
    retained: [&'static str; 4],
}

#[derive(Serialize)]
struct Deleted {
    conversations: u64,
    turns: u64,
    message_requests: u64,
    custom_souls: u64,
    custom_profiles: u64,
    goals: usize,
    mcp_secrets: usize,
}

pub(crate) async fn purge(
    State(state): State<Arc<AppState>>,
) -> Result<Json<PrivacyPurgeResponse>, AppError> {
    tokio::time::timeout(
        crate::constants::privacy::PURGE_DEADLINE,
        purge_inner(state),
    )
    .await
    .map_err(|_| {
        AppError::Internal(
            "Privacy purge timed out; some data may already be deleted. Retry to finish.".into(),
        )
    })?
}

async fn purge_inner(state: Arc<AppState>) -> Result<Json<PrivacyPurgeResponse>, AppError> {
    let _purge_guard = state
        .runtime
        .runtime_session
        .try_begin_privacy_purge()
        .map_err(|message| AppError::Conflict(message.into()))?;

    // Erase authoritative local context before the potentially slow remote
    // session purge. If the request times out, no later maintenance cycle can
    // re-upload the personal memory or goals that the user asked to delete.
    state
        .runtime
        .memory_file
        .replace_all(crate::copilot::memory_file::DEFAULT_BODY.into())
        .await?;
    let goals = state.services.goals_service.delete_all().await?;

    let mut transaction = state.infra.db.begin().await?;
    let turns = sqlx::query("DELETE FROM conversation_turns")
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    sqlx::query(
        "INSERT OR IGNORE INTO message_request_tombstones (request_id) \
         SELECT request_id FROM message_requests",
    )
    .execute(&mut *transaction)
    .await?;
    let message_requests = sqlx::query("DELETE FROM message_requests")
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    let conversations = sqlx::query("DELETE FROM conversations")
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    sqlx::query("DELETE FROM cooldown_state")
        .execute(&mut *transaction)
        .await?;
    let custom_souls = sqlx::query("DELETE FROM souls WHERE is_default = 0")
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    let custom_profiles = sqlx::query("DELETE FROM user_profiles WHERE is_default = 0")
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    transaction.commit().await?;

    let data_root = std::path::PathBuf::from(&state.config().data_dir);
    crate::util::durable_fs::durable_remove_dir_all(&data_root.join("tool-output")).await?;
    state.runtime.workers.purge_sessions().await?;
    let mcp_secrets = purge_mcp(&state).await?;

    Ok(Json(PrivacyPurgeResponse {
        message: "Personal data deleted; operational data retained",
        deleted: Deleted {
            conversations,
            turns,
            message_requests,
            custom_souls,
            custom_profiles,
            goals,
            mcp_secrets,
        },
        retained: [
            "settings and API credentials",
            "installed models and runtimes",
            "Copilot login",
            "content-free request replay guards",
        ],
    }))
}

async fn purge_mcp(state: &AppState) -> Result<usize, AppError> {
    let _guard = state.infra.mcp_config_lock.lock().await;
    let config = state.config();
    let path = mcp_config::resolve_mcp_config_path(
        std::path::Path::new(&config.data_dir),
        config.mcp.config_file.as_deref(),
    )?;
    let previous = mcp_config::load_mcp_config_file(&path).unwrap_or_else(|_| McpConfigFile {
        mcp_servers: std::collections::HashMap::new(),
    });
    let mut accounts = HashSet::new();
    for (server, entry) in &previous.mcp_servers {
        for (key, value) in &entry.env {
            if let Some(account) = value.strip_prefix(mcp_config::SECRET_REF_PREFIX) {
                accounts.insert(account.to_owned());
            }
            accounts.insert(mcp_config::secret_account(server, key));
            accounts.insert(mcp_config::legacy_secret_account(server, key));
        }
    }
    for account in &accounts {
        state.infra.secret_store.delete(account).map_err(|error| {
            AppError::Config(format!("Failed to delete MCP secret '{account}': {error}"))
        })?;
    }
    mcp_config::save_mcp_config_file(
        &path,
        &McpConfigFile {
            mcp_servers: std::collections::HashMap::new(),
        },
    )?;
    state
        .runtime
        .workers
        .apply_mcp_config(github_copilot_sdk::IndexMap::new(), Vec::new())
        .await;
    Ok(accounts.len())
}
