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
    retained: [&'static str; 3],
}

#[derive(Serialize)]
struct Deleted {
    conversations: u64,
    turns: u64,
    custom_souls: u64,
    custom_profiles: u64,
    goals: usize,
    mcp_secrets: usize,
}

pub(crate) async fn purge(
    State(state): State<Arc<AppState>>,
) -> Result<Json<PrivacyPurgeResponse>, AppError> {
    if state
        .runtime
        .in_flight_text_turns
        .load(std::sync::atomic::Ordering::Acquire)
        != 0
        || state
            .voice
            .voice_turn_active
            .load(std::sync::atomic::Ordering::Acquire)
    {
        return Err(AppError::Conflict(
            "Cannot purge data while a turn is active".into(),
        ));
    }

    state.runtime.workers.purge_sessions().await?;

    let mut transaction = state.infra.db.begin().await?;
    let turns = sqlx::query("DELETE FROM conversation_turns")
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

    state
        .runtime
        .memory_file
        .replace_all(crate::copilot::memory_file::DEFAULT_BODY.into())
        .await?;
    let goals = state.services.goals_service.delete_all().await?;

    let mcp_secrets = purge_mcp(&state).await?;
    let data_root = std::path::PathBuf::from(&state.config().data_dir);
    crate::util::durable_fs::durable_remove_dir_all(&data_root.join("tool-output")).await?;

    Ok(Json(PrivacyPurgeResponse {
        message: "Personal data deleted; operational data retained",
        deleted: Deleted {
            conversations,
            turns,
            custom_souls,
            custom_profiles,
            goals,
            mcp_secrets,
        },
        retained: [
            "settings and API credentials",
            "installed models and runtimes",
            "Copilot login",
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
    let previous = mcp_config::load_mcp_config_file(&path).unwrap_or(McpConfigFile {
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
