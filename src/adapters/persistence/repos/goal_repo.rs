use async_trait::async_trait;
use sqlx::SqlitePool;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::domain::goal::Goal;
use crate::domain::types::{GoalPriority, GoalSource, GoalStatus};
use crate::error::AppError;
use crate::ports::repos::goal_repo::GoalRepository;

pub struct SqliteGoalRepo {
    pool: SqlitePool,
}

impl SqliteGoalRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl GoalRepository for SqliteGoalRepo {
    async fn save(&self, goal: &Goal) -> Result<Goal, AppError> {
        let id = goal.id.to_string();
        sqlx::query(
            r#"INSERT INTO goals (id, content, priority, source, status, enabled, inference_reason, embedding, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
               ON CONFLICT(id) DO UPDATE SET
                   content = excluded.content,
                   priority = excluded.priority,
                   source = excluded.source,
                   status = excluded.status,
                   enabled = excluded.enabled,
                   inference_reason = excluded.inference_reason,
                   embedding = excluded.embedding,
                   updated_at = excluded.updated_at"#,
        )
        .bind(&id)
        .bind(&goal.content)
        .bind(&goal.priority)
        .bind(&goal.source)
        .bind(&goal.status)
        .bind(goal.enabled)
        .bind(&goal.inference_reason)
        .bind(&goal.embedding)
        .bind(&goal.created_at)
        .bind(&goal.updated_at)
        .execute(&self.pool)
        .await
        .map_err(AppError::Database)?;

        debug!(
            goal_id = %id,
            source = %goal.source,
            status = %goal.status,
            has_embedding = goal.embedding.is_some(),
            "goal_repo.saved"
        );
        Ok(goal.clone())
    }

    async fn get_by_id(&self, id: Uuid) -> Result<Option<Goal>, AppError> {
        sqlx::query_as::<_, Goal>("SELECT * FROM goals WHERE id = ?1")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_by_status(
        &self,
        status: GoalStatus,
        enabled_only: bool,
    ) -> Result<Vec<Goal>, AppError> {
        let sql = if enabled_only {
            "SELECT * FROM goals WHERE status = ?1 AND enabled = 1 ORDER BY priority DESC, created_at DESC"
        } else {
            "SELECT * FROM goals WHERE status = ?1 ORDER BY priority DESC, created_at DESC"
        };

        sqlx::query_as::<_, Goal>(sql)
            .bind(status.as_str())
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_active(&self, enabled_only: bool) -> Result<Vec<Goal>, AppError> {
        self.find_by_status(GoalStatus::Active, enabled_only).await
    }

    async fn find_enabled(&self) -> Result<Vec<Goal>, AppError> {
        sqlx::query_as::<_, Goal>(
            "SELECT * FROM goals WHERE enabled = 1 ORDER BY priority DESC, created_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn find_similar(
        &self,
        embedding: &[f32],
        limit: i64,
        enabled_only: bool,
    ) -> Result<Vec<(Goal, f64)>, AppError> {
        let sql = if enabled_only {
            "SELECT * FROM goals WHERE embedding IS NOT NULL AND enabled = 1"
        } else {
            "SELECT * FROM goals WHERE embedding IS NOT NULL"
        };

        let goals = sqlx::query_as::<_, Goal>(sql)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)?;

        let mut scored: Vec<(Goal, f64)> = goals
            .into_iter()
            .filter_map(|g| {
                let stored: Vec<f32> = serde_json::from_str(g.embedding.as_ref()?).ok()?;
                let sim = cosine_similarity(embedding, &stored);
                Some((g, sim))
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit as usize);

        debug!(
            results_count = scored.len(),
            top_scores = ?scored.iter().take(3).map(|(_, s)| (*s * 1000.0).round() / 1000.0).collect::<Vec<_>>(),
            "goal_repo.find_similar"
        );
        Ok(scored)
    }

    async fn update_status(
        &self,
        id: Uuid,
        status: Option<GoalStatus>,
        enabled: Option<bool>,
    ) -> Result<Option<Goal>, AppError> {
        let existing = self.get_by_id(id).await?;
        if existing.is_none() {
            warn!(goal_id = %id, "goal_repo.update.not_found");
            return Ok(None);
        }

        let mut sets = Vec::new();
        if status.is_some() {
            sets.push("status = ?");
        }
        if enabled.is_some() {
            sets.push("enabled = ?");
        }
        sets.push("updated_at = ?");

        let sql = format!("UPDATE goals SET {} WHERE id = ?", sets.join(", "));
        let mut q = sqlx::query(&sql);
        if let Some(s) = &status {
            q = q.bind(s.as_str());
        }
        if let Some(e) = enabled {
            q = q.bind(e);
        }
        q = q.bind(chrono::Utc::now()).bind(id.to_string());
        q.execute(&self.pool).await.map_err(AppError::Database)?;

        info!(goal_id = %id, status = ?status.map(|s| s.as_str()), enabled = ?enabled, "goal_repo.updated");
        self.get_by_id(id).await
    }

    async fn update_fields(
        &self,
        id: Uuid,
        content: Option<&str>,
        status: Option<GoalStatus>,
        priority: Option<GoalPriority>,
        source: Option<GoalSource>,
        enabled: Option<bool>,
    ) -> Result<Option<Goal>, AppError> {
        let existing = self.get_by_id(id).await?;
        if existing.is_none() {
            warn!(goal_id = %id, "goal_repo.update_fields.not_found");
            return Ok(None);
        }

        let mut sets = Vec::new();
        if content.is_some() {
            sets.push("content = ?");
        }
        if status.is_some() {
            sets.push("status = ?");
        }
        if priority.is_some() {
            sets.push("priority = ?");
        }
        if source.is_some() {
            sets.push("source = ?");
        }
        if enabled.is_some() {
            sets.push("enabled = ?");
        }
        sets.push("updated_at = ?");

        let sql = format!("UPDATE goals SET {} WHERE id = ?", sets.join(", "));
        let mut q = sqlx::query(&sql);
        if let Some(c) = content {
            q = q.bind(c);
        }
        if let Some(s) = &status {
            q = q.bind(s.as_str());
        }
        if let Some(p) = &priority {
            q = q.bind(p.as_str());
        }
        if let Some(s) = &source {
            q = q.bind(s.as_str());
        }
        if let Some(e) = enabled {
            q = q.bind(e);
        }
        q = q.bind(chrono::Utc::now()).bind(id.to_string());
        q.execute(&self.pool).await.map_err(AppError::Database)?;

        info!(goal_id = %id, content_updated = content.is_some(), "goal_repo.update_fields");
        self.get_by_id(id).await
    }

    async fn delete(&self, id: Uuid) -> Result<bool, AppError> {
        let result = sqlx::query("DELETE FROM goals WHERE id = ?1")
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;

        if result.rows_affected() > 0 {
            info!(goal_id = %id, "goal_repo.deleted");
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn find_by_content(&self, content: &str) -> Result<Option<Goal>, AppError> {
        sqlx::query_as::<_, Goal>("SELECT * FROM goals WHERE content = ?1")
            .bind(content)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn get_all(&self, include_archived: bool) -> Result<Vec<Goal>, AppError> {
        let sql = if include_archived {
            "SELECT * FROM goals ORDER BY created_at DESC"
        } else {
            "SELECT * FROM goals WHERE status != 'archived' ORDER BY created_at DESC"
        };

        sqlx::query_as::<_, Goal>(sql)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| *x as f64 * *y as f64).sum();
    let norm_a: f64 = a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}
