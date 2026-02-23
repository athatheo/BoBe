use async_trait::async_trait;
use chrono::{Duration, Utc};
use sqlx::SqlitePool;
use tracing::{debug, info};
use uuid::Uuid;

use crate::domain::observation::Observation;
use crate::error::AppError;
use crate::ports::repos::observation_repo::ObservationRepository;

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
        let id = observation.id.to_string();
        sqlx::query(
            r#"INSERT INTO observations (id, source, content, category, embedding, metadata, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
               ON CONFLICT(id) DO UPDATE SET
                   content = excluded.content,
                   embedding = excluded.embedding,
                   metadata = excluded.metadata,
                   updated_at = excluded.updated_at"#,
        )
        .bind(&id)
        .bind(&observation.source)
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
            observation_id = %id,
            category = %observation.category,
            has_embedding = observation.embedding.is_some(),
            "observation_repo.saved"
        );
        Ok(observation.clone())
    }

    async fn get_by_id(&self, id: Uuid) -> Result<Option<Observation>, AppError> {
        sqlx::query_as::<_, Observation>("SELECT * FROM observations WHERE id = ?1")
            .bind(id.to_string())
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
        let mut sql = String::from("SELECT * FROM observations");
        if since.is_some() {
            sql.push_str(" WHERE created_at > ?1");
        }
        sql.push_str(" ORDER BY created_at ASC");
        if let Some(lim) = limit {
            sql.push_str(&format!(" LIMIT {lim}"));
        }

        let rows = if let Some(s) = since {
            sqlx::query_as::<_, Observation>(&sql)
                .bind(s)
                .fetch_all(&self.pool)
                .await
        } else {
            sqlx::query_as::<_, Observation>(&sql)
                .fetch_all(&self.pool)
                .await
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
            "SELECT * FROM observations WHERE embedding IS NOT NULL",
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
        info!(days, deleted_count = count, "observation_repo.deleted_older_than");
        Ok(count)
    }

    async fn delete(&self, id: Uuid) -> Result<bool, AppError> {
        let result = sqlx::query("DELETE FROM observations WHERE id = ?1")
            .bind(id.to_string())
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
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;
        Ok(())
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
