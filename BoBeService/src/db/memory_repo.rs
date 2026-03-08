use crate::db::MemoryRepository;
use crate::error::AppError;
use crate::models::ids::MemoryId;
use crate::models::memory::Memory;
use crate::models::types::{MemorySource, MemoryType};
use crate::util::similarity::cosine_similarity;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use tracing::{debug, info, warn};

pub(crate) struct SqliteMemoryRepo {
    pool: SqlitePool,
}

impl SqliteMemoryRepo {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MemoryRepository for SqliteMemoryRepo {
    async fn save(&self, memory: &Memory) -> Result<Memory, AppError> {
        sqlx::query(
            r"INSERT INTO memories (id, content, memory_type, enabled, category, source, embedding,
                   source_observation_id, source_conversation_id, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
               ON CONFLICT(id) DO UPDATE SET
                   content = excluded.content,
                   memory_type = excluded.memory_type,
                   enabled = excluded.enabled,
                   category = excluded.category,
                   source = excluded.source,
                   embedding = excluded.embedding,
                   updated_at = excluded.updated_at",
        )
        .bind(memory.id)
        .bind(&memory.content)
        .bind(memory.memory_type)
        .bind(memory.enabled)
        .bind(&memory.category)
        .bind(memory.source)
        .bind(&memory.embedding)
        .bind(memory.source_observation_id)
        .bind(memory.source_conversation_id)
        .bind(memory.created_at)
        .bind(memory.updated_at)
        .execute(&self.pool)
        .await
        .map_err(AppError::Database)?;

        debug!(
            memory_id = %memory.id,
            memory_type = %memory.memory_type,
            category = %memory.category,
            has_embedding = memory.embedding.is_some(),
            "memory_repo.saved"
        );
        Ok(memory.clone())
    }

    async fn get_by_id(&self, id: MemoryId) -> Result<Option<Memory>, AppError> {
        sqlx::query_as::<_, Memory>("SELECT * FROM memories WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_by_type(
        &self,
        memory_type: MemoryType,
        enabled_only: bool,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<Memory>, AppError> {
        let cols = "id, content, memory_type, enabled, category, source, \
                    NULL as embedding, source_observation_id, source_conversation_id, \
                    created_at, updated_at";
        let mut sql = format!("SELECT {cols} FROM memories WHERE memory_type = ?1");
        if enabled_only {
            sql.push_str(" AND enabled = 1");
        }
        if since.is_some() {
            sql.push_str(" AND created_at > ?2");
        }
        sql.push_str(" ORDER BY created_at DESC LIMIT 500");

        let mut q = sqlx::query_as::<_, Memory>(&sql).bind(memory_type.as_str());
        if let Some(s) = since {
            q = q.bind(s);
        }

        q.fetch_all(&self.pool).await.map_err(AppError::Database)
    }

    async fn find_enabled(&self, limit: Option<i64>) -> Result<Vec<Memory>, AppError> {
        let cols = "id, content, memory_type, enabled, category, source, \
                    NULL as embedding, source_observation_id, source_conversation_id, \
                    created_at, updated_at";
        if let Some(lim) = limit {
            sqlx::query_as::<_, Memory>(&format!(
                "SELECT {cols} FROM memories WHERE enabled = 1 ORDER BY created_at DESC LIMIT ?1"
            ))
            .bind(lim)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
        } else {
            sqlx::query_as::<_, Memory>(&format!(
                "SELECT {cols} FROM memories WHERE enabled = 1 ORDER BY created_at DESC"
            ))
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
        }
    }

    async fn find_all(
        &self,
        memory_type: Option<MemoryType>,
        category: Option<&str>,
        source: Option<MemorySource>,
        enabled_only: bool,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<Memory>, i64), AppError> {
        let mut conditions = Vec::new();
        if memory_type.is_some() {
            conditions.push("memory_type = ?");
        }
        if category.is_some() {
            conditions.push("category = ?");
        }
        if source.is_some() {
            conditions.push("source = ?");
        }
        if enabled_only {
            conditions.push("enabled = 1");
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };

        let count_sql = format!("SELECT COUNT(*) as cnt FROM memories{where_clause}");
        let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql);
        if let Some(mt) = memory_type {
            count_q = count_q.bind(mt.as_str());
        }
        if let Some(cat) = category {
            count_q = count_q.bind(cat);
        }
        if let Some(src) = source {
            count_q = count_q.bind(src.as_str());
        }
        let total = count_q
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::Database)?;

        let data_sql = format!(
            "SELECT id, content, memory_type, enabled, category, source, \
             NULL as embedding, source_observation_id, source_conversation_id, \
             created_at, updated_at \
             FROM memories{where_clause} ORDER BY created_at DESC LIMIT ? OFFSET ?"
        );
        let mut data_q = sqlx::query_as::<_, Memory>(&data_sql);
        if let Some(mt) = memory_type {
            data_q = data_q.bind(mt.as_str());
        }
        if let Some(cat) = category {
            data_q = data_q.bind(cat);
        }
        if let Some(src) = source {
            data_q = data_q.bind(src.as_str());
        }
        data_q = data_q.bind(limit).bind(offset);

        let memories = data_q
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)?;

        Ok((memories, total))
    }

    async fn find_similar(
        &self,
        embedding: &[f32],
        limit: i64,
        enabled_only: bool,
        min_score: f64,
    ) -> Result<Vec<(Memory, f64)>, AppError> {
        // Cap candidate set to 5000 most recent to bound memory usage.
        let sql = if enabled_only {
            "SELECT * FROM memories WHERE embedding IS NOT NULL AND enabled = 1 \
             ORDER BY updated_at DESC LIMIT 5000"
        } else {
            "SELECT * FROM memories WHERE embedding IS NOT NULL \
             ORDER BY updated_at DESC LIMIT 5000"
        };

        let memories = sqlx::query_as::<_, Memory>(sql)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)?;

        let mut scored: Vec<(Memory, f64)> = memories
            .into_iter()
            .filter_map(|m| {
                let stored: Vec<f32> = serde_json::from_str(m.embedding.as_ref()?).ok()?;
                let sim = cosine_similarity(embedding, &stored);
                if sim >= min_score {
                    Some((m, sim))
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(usize::try_from(limit).unwrap_or(usize::MAX));

        debug!(
            results_count = scored.len(),
            min_score,
            top_scores = ?scored.iter().take(3).map(|(_, s)| (*s * 1000.0).round() / 1000.0).collect::<Vec<_>>(),
            "memory_repo.find_similar"
        );
        Ok(scored)
    }

    async fn update(
        &self,
        id: MemoryId,
        content: Option<&str>,
        enabled: Option<bool>,
        category: Option<&str>,
    ) -> Result<Option<Memory>, AppError> {
        let existing = self.get_by_id(id).await?;
        if existing.is_none() {
            warn!(memory_id = %id, "memory_repo.update.not_found");
            return Ok(None);
        }

        let mut sets = Vec::new();
        if content.is_some() {
            sets.push("content = ?");
        }
        if enabled.is_some() {
            sets.push("enabled = ?");
        }
        if category.is_some() {
            sets.push("category = ?");
        }
        sets.push("updated_at = ?");

        let sql = format!("UPDATE memories SET {} WHERE id = ?", sets.join(", "));
        let mut q = sqlx::query(&sql);
        if let Some(c) = content {
            q = q.bind(c);
        }
        if let Some(e) = enabled {
            q = q.bind(e);
        }
        if let Some(cat) = category {
            q = q.bind(cat);
        }
        q = q.bind(Utc::now()).bind(id);

        q.execute(&self.pool).await.map_err(AppError::Database)?;

        info!(
            memory_id = %id,
            content_updated = content.is_some(),
            enabled = ?enabled,
            category = ?category,
            "memory_repo.updated"
        );
        self.get_by_id(id).await
    }

    async fn delete_by_criteria(
        &self,
        memory_type: MemoryType,
        older_than: DateTime<Utc>,
    ) -> Result<i64, AppError> {
        let result = sqlx::query("DELETE FROM memories WHERE memory_type = ?1 AND created_at < ?2")
            .bind(memory_type.as_str())
            .bind(older_than)
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;

        let count = i64::try_from(result.rows_affected()).unwrap_or(0);
        info!(
            memory_type = %memory_type.as_str(),
            older_than = %older_than,
            deleted_count = count,
            "memory_repo.deleted_by_criteria"
        );
        Ok(count)
    }

    async fn delete(&self, id: MemoryId) -> Result<bool, AppError> {
        let result = sqlx::query("DELETE FROM memories WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;

        if result.rows_affected() > 0 {
            info!(memory_id = %id, "memory_repo.deleted");
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn find_null_embedding(&self, limit: i64) -> Result<Vec<Memory>, AppError> {
        sqlx::query_as::<_, Memory>(
            "SELECT * FROM memories WHERE embedding IS NULL ORDER BY created_at DESC LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn update_embedding(&self, id: MemoryId, embedding: &[f32]) -> Result<(), AppError> {
        let json = serde_json::to_string(embedding)
            .map_err(|e| AppError::Internal(format!("Failed to serialize embedding: {e}")))?;
        sqlx::query("UPDATE memories SET embedding = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(&json)
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;
        Ok(())
    }
}
