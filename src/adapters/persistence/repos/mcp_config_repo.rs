use async_trait::async_trait;
use sqlx::SqlitePool;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::domain::mcp_server_config::McpServerConfig;
use crate::error::AppError;
use crate::ports::repos::mcp_config_repo::McpConfigRepository;

pub struct SqliteMcpConfigRepo {
    pool: SqlitePool,
}

impl SqliteMcpConfigRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl McpConfigRepository for SqliteMcpConfigRepo {
    async fn save(&self, config: &McpServerConfig) -> Result<McpServerConfig, AppError> {
        sqlx::query(
            r#"INSERT INTO mcp_server_configs (id, server_name, command, args, env, enabled,
                   timeout_seconds, is_default, last_connected_at, last_error, excluded_tools,
                   created_at, updated_at)
               VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
               ON CONFLICT(id) DO UPDATE SET
                   server_name = excluded.server_name,
                   command = excluded.command,
                   args = excluded.args,
                   env = excluded.env,
                   enabled = excluded.enabled,
                   timeout_seconds = excluded.timeout_seconds,
                   is_default = excluded.is_default,
                   last_connected_at = excluded.last_connected_at,
                   last_error = excluded.last_error,
                   excluded_tools = excluded.excluded_tools,
                   updated_at = excluded.updated_at"#,
        )
        .bind(config.id)
        .bind(&config.server_name)
        .bind(&config.command)
        .bind(&config.args)
        .bind(&config.env)
        .bind(config.enabled)
        .bind(config.timeout_seconds)
        .bind(config.is_default)
        .bind(config.last_connected_at)
        .bind(&config.last_error)
        .bind(&config.excluded_tools)
        .bind(config.created_at)
        .bind(config.updated_at)
        .execute(&self.pool)
        .await
        .map_err(AppError::Database)?;

        debug!(
            config_id = %config.id,
            server_name = %config.server_name,
            enabled = config.enabled,
            "mcp_config_repo.saved"
        );
        Ok(config.clone())
    }

    async fn get_by_id(&self, id: Uuid) -> Result<Option<McpServerConfig>, AppError> {
        sqlx::query_as::<_, McpServerConfig>("SELECT * FROM mcp_server_configs WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn get_by_name(&self, name: &str) -> Result<Option<McpServerConfig>, AppError> {
        sqlx::query_as::<_, McpServerConfig>(
            "SELECT * FROM mcp_server_configs WHERE server_name = ?1",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn get_all(&self) -> Result<Vec<McpServerConfig>, AppError> {
        sqlx::query_as::<_, McpServerConfig>(
            "SELECT * FROM mcp_server_configs ORDER BY server_name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn find_enabled(&self) -> Result<Vec<McpServerConfig>, AppError> {
        sqlx::query_as::<_, McpServerConfig>(
            "SELECT * FROM mcp_server_configs WHERE enabled = 1 ORDER BY server_name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn update(
        &self,
        id: Uuid,
        command: Option<&str>,
        args: Option<&str>,
        env: Option<&str>,
        enabled: Option<bool>,
        timeout_seconds: Option<f64>,
        excluded_tools: Option<&str>,
    ) -> Result<Option<McpServerConfig>, AppError> {
        let existing = self.get_by_id(id).await?;
        if existing.is_none() {
            warn!(config_id = %id, "mcp_config_repo.update_not_found");
            return Ok(None);
        }

        let mut sets = Vec::new();
        if command.is_some() {
            sets.push("command = ?");
        }
        if args.is_some() {
            sets.push("args = ?");
        }
        if env.is_some() {
            sets.push("env = ?");
        }
        if enabled.is_some() {
            sets.push("enabled = ?");
        }
        if timeout_seconds.is_some() {
            sets.push("timeout_seconds = ?");
        }
        if excluded_tools.is_some() {
            sets.push("excluded_tools = ?");
        }
        sets.push("updated_at = ?");

        let sql = format!(
            "UPDATE mcp_server_configs SET {} WHERE id = ?",
            sets.join(", ")
        );
        let mut q = sqlx::query(&sql);
        if let Some(cmd) = command {
            q = q.bind(cmd);
        }
        if let Some(a) = args {
            q = q.bind(a);
        }
        if let Some(e) = env {
            q = q.bind(e);
        }
        if let Some(en) = enabled {
            q = q.bind(en);
        }
        if let Some(t) = timeout_seconds {
            q = q.bind(t);
        }
        if let Some(et) = excluded_tools {
            q = q.bind(et);
        }
        q = q.bind(chrono::Utc::now()).bind(id);
        q.execute(&self.pool).await.map_err(AppError::Database)?;

        info!(config_id = %id, "mcp_config_repo.updated");
        self.get_by_id(id).await
    }

    async fn delete(&self, id: Uuid) -> Result<bool, AppError> {
        let result = sqlx::query("DELETE FROM mcp_server_configs WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;

        if result.rows_affected() > 0 {
            info!(config_id = %id, "mcp_config_repo.deleted");
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
