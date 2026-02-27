use async_trait::async_trait;
use chrono::{Duration, Utc};
use sqlx::SqlitePool;
use tracing::{debug, info};
use uuid::Uuid;

use crate::db::ObservationRepository;
use crate::error::AppError;
use crate::models::observation::Observation;
use crate::util::similarity::cosine_similarity;

pub struct SqliteObservationRepo {
    pool: SqlitePool,
}

impl SqliteObservationRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ObservationRepository for SqliteObservationRepo {
    async fn save(&self, observation: &Observation) -> Result<Observation, AppError> {
        sqlx::query(
            r#"INSERT INTO observations (id, source, content, category, embedding, metadata, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
               ON CONFLICT(id) DO UPDATE SET
                   content = excluded.content,
                   embedding = excluded.embedding,
                   metadata = excluded.metadata,
                   updated_at = excluded.updated_at"#,
        )
        .bind(observation.id)
        .bind(observation.source)
        .bind(&observation.content)
        .bind(&observation.category)
        .bind(&observation.embedding)
        .bind(&observation.metadata)
        .bind(observation.created_at)
        .bind(observation.updated_at)
        .execute(&self.pool)
        .await
        .map_err(AppError::Database)?;

        debug!(
            observation_id = %observation.id,
            category = %observation.category,
            has_embedding = observation.embedding.is_some(),
            "observation_repo.saved"
        );
        Ok(observation.clone())
    }

    async fn get_by_id(&self, id: Uuid) -> Result<Option<Observation>, AppError> {
        sqlx::query_as::<_, Observation>("SELECT * FROM observations WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_recent(&self, minutes: i64) -> Result<Vec<Observation>, AppError> {
        let cutoff = Utc::now() - Duration::minutes(minutes);
        sqlx::query_as::<_, Observation>(
            "SELECT * FROM observations WHERE created_at >= ?1 ORDER BY created_at DESC",
        )
        .bind(cutoff)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn find_since(
        &self,
        since: Option<chrono::DateTime<Utc>>,
        limit: Option<i64>,
    ) -> Result<Vec<Observation>, AppError> {
        // Exclude embedding blob — callers need content, not vectors
        let cols =
            "id, source, content, category, NULL as embedding, metadata, created_at, updated_at";
        let rows = match (since, limit) {
            (Some(s), Some(lim)) => {
                sqlx::query_as::<_, Observation>(
                    &format!("SELECT {cols} FROM observations WHERE created_at > ?1 ORDER BY created_at ASC LIMIT ?2"),
                )
                .bind(s)
                .bind(lim)
                .fetch_all(&self.pool)
                .await
            }
            (Some(s), None) => {
                sqlx::query_as::<_, Observation>(
                    &format!("SELECT {cols} FROM observations WHERE created_at > ?1 ORDER BY created_at ASC"),
                )
                .bind(s)
                .fetch_all(&self.pool)
                .await
            }
            (None, Some(lim)) => {
                sqlx::query_as::<_, Observation>(
                    &format!("SELECT {cols} FROM observations ORDER BY created_at ASC LIMIT ?1"),
                )
                .bind(lim)
                .fetch_all(&self.pool)
                .await
            }
            (None, None) => {
                sqlx::query_as::<_, Observation>(
                    &format!("SELECT {cols} FROM observations ORDER BY created_at ASC"),
                )
                .fetch_all(&self.pool)
                .await
            }
        }
        .map_err(AppError::Database)?;

        Ok(rows)
    }

    async fn find_similar(
        &self,
        embedding: &[f32],
        limit: i64,
    ) -> Result<Vec<(Observation, f64)>, AppError> {
        let observations = sqlx::query_as::<_, Observation>(
            "SELECT * FROM observations WHERE embedding IS NOT NULL \
             ORDER BY created_at DESC LIMIT 5000",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)?;

        let mut scored: Vec<(Observation, f64)> = observations
            .into_iter()
            .filter_map(|o| {
                let stored: Vec<f32> = serde_json::from_str(o.embedding.as_ref()?).ok()?;
                let sim = cosine_similarity(embedding, &stored);
                Some((o, sim))
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit as usize);

        debug!(
            results_count = scored.len(),
            top_scores = ?scored.iter().take(3).map(|(_, s)| (*s * 1000.0).round() / 1000.0).collect::<Vec<_>>(),
            "observation_repo.find_similar"
        );
        Ok(scored)
    }

    async fn delete_older_than(&self, days: i64) -> Result<i64, AppError> {
        let cutoff = Utc::now() - Duration::days(days);
        let result = sqlx::query("DELETE FROM observations WHERE created_at < ?1")
            .bind(cutoff)
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;

        let count = result.rows_affected() as i64;
        info!(
            days,
            deleted_count = count,
            "observation_repo.deleted_older_than"
        );
        Ok(count)
    }

    async fn delete(&self, id: Uuid) -> Result<bool, AppError> {
        let result = sqlx::query("DELETE FROM observations WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;

        if result.rows_affected() > 0 {
            info!(observation_id = %id, "observation_repo.deleted");
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn find_null_embedding(&self, limit: i64) -> Result<Vec<Observation>, AppError> {
        sqlx::query_as::<_, Observation>(
            "SELECT * FROM observations WHERE embedding IS NULL ORDER BY created_at DESC LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn update_embedding(&self, id: Uuid, embedding: &[f32]) -> Result<(), AppError> {
        let json = serde_json::to_string(embedding)
            .map_err(|e| AppError::Internal(format!("Failed to serialize embedding: {e}")))?;
        sqlx::query("UPDATE observations SET embedding = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(&json)
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;
        Ok(())
    }
}
