//! GoalsService — manages goals in the database.
//!
//! Goals are stored in the database. A GOALS.md file can be used to seed
//! initial/default goals on startup, but inferred goals are DB-only.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::domain::goal::Goal;
use crate::domain::types::{GoalPriority, GoalSource, GoalStatus};
use crate::error::AppError;
use crate::ports::embedding::EmbeddingProvider;
use crate::ports::repos::goal_repo::GoalRepository;

use super::goals_config::GoalConfig;
use super::goals_file_parser::parse_goals_file;

/// Maximum file size for GOALS.md (1 MB).
const MAX_GOALS_FILE_SIZE: u64 = 1024 * 1024;

/// Result of syncing GOALS.md to database.
#[derive(Debug, Clone)]
pub struct SyncResult {
    pub created: u32,
    pub updated: u32,
    pub archived: u32,
}

pub struct GoalsService {
    repo: Arc<dyn GoalRepository>,
    embedding: Arc<dyn EmbeddingProvider>,
    config: GoalConfig,
}

impl GoalsService {
    pub fn new(
        repo: Arc<dyn GoalRepository>,
        embedding: Arc<dyn EmbeddingProvider>,
        config: GoalConfig,
    ) -> Self {
        Self {
            repo,
            embedding,
            config,
        }
    }

    // ── Database Operations ─────────────────────────────────────────────

    /// Create a goal with embedding.
    pub async fn create(
        &self,
        content: &str,
        source: GoalSource,
        priority: GoalPriority,
        inference_reason: Option<String>,
    ) -> Result<Goal, AppError> {
        let embedding_vec = self.embedding.embed(content).await?;

        let mut goal = Goal::new(content.to_owned(), source, priority);
        goal.inference_reason = inference_reason;
        goal.embedding = Some(serde_json::to_string(&embedding_vec)?);

        let saved = self.repo.save(&goal).await?;
        info!(
            goal_id = %saved.id,
            content_preview = &content[..content.len().min(50)],
            source = source.as_str(),
            priority = priority.as_str(),
            "goals_service.created"
        );
        Ok(saved)
    }

    /// Get active goals ordered by priority (high → medium → low).
    pub async fn get_active(&self, limit: usize) -> Result<Vec<Goal>, AppError> {
        let mut goals = self.repo.find_active(true).await?;

        // Sort by priority
        goals.sort_by_key(|g| match g.priority.as_str() {
            "high" => 0,
            "medium" => 1,
            "low" => 2,
            _ => 1,
        });

        goals.truncate(limit);
        debug!(count = goals.len(), limit, "goals_service.get_active");
        Ok(goals)
    }

    /// Get a goal by ID.
    pub async fn get_by_id(&self, goal_id: Uuid) -> Result<Option<Goal>, AppError> {
        self.repo.get_by_id(goal_id).await
    }

    /// Semantic search for relevant goals.
    pub async fn get_by_embedding(
        &self,
        embedding: &[f32],
        limit: i64,
        min_score: f64,
        include_statuses: Option<&[GoalStatus]>,
    ) -> Result<Vec<Goal>, AppError> {
        let results = self.repo.find_similar(embedding, limit * 2, true).await?;

        let status_filter: HashSet<&str> = match include_statuses {
            Some(statuses) => statuses.iter().map(|s| s.as_str()).collect(),
            None => {
                let mut set = HashSet::new();
                set.insert("active");
                set
            }
        };

        let goals: Vec<Goal> = results
            .into_iter()
            .filter(|(_, score)| *score >= min_score)
            .filter(|(goal, _)| status_filter.contains(goal.status.as_str()))
            .take(limit as usize)
            .map(|(goal, _)| goal)
            .collect();

        debug!(
            results = goals.len(),
            limit,
            min_score,
            "goals_service.get_by_embedding"
        );
        Ok(goals)
    }

    /// Update goal status (active/completed/archived).
    pub async fn update_status(
        &self,
        goal_id: Uuid,
        status: GoalStatus,
    ) -> Result<Option<Goal>, AppError> {
        let updated = self.repo.update_status(goal_id, Some(status), None).await?;
        if updated.is_some() {
            info!(
                goal_id = %goal_id,
                new_status = status.as_str(),
                "goals_service.status_updated"
            );
        }
        Ok(updated)
    }

    /// Update a goal's content and re-generate its embedding.
    pub async fn update_content(
        &self,
        goal_id: Uuid,
        content: &str,
    ) -> Result<Option<Goal>, AppError> {
        let embedding_vec = self.embedding.embed(content).await?;
        let embedding_json = serde_json::to_string(&embedding_vec)?;

        let goal = self.repo.get_by_id(goal_id).await?;
        let Some(mut goal) = goal else {
            warn!(goal_id = %goal_id, "goals_service.update_content.not_found");
            return Ok(None);
        };

        goal.content = content.to_owned();
        goal.embedding = Some(embedding_json);
        goal.updated_at = chrono::Utc::now();

        let updated = self.repo.save(&goal).await?;
        info!(
            goal_id = %goal_id,
            content_preview = &content[..content.len().min(80)],
            "goals_service.content_updated"
        );
        Ok(Some(updated))
    }

    /// Check if a similar goal already exists (for deduplication).
    pub async fn find_similar(
        &self,
        content: &str,
        threshold: f64,
    ) -> Result<Option<Goal>, AppError> {
        let embedding_vec = self.embedding.embed(content).await?;
        let results = self.repo.find_similar(&embedding_vec, 1, true).await?;

        if let Some((goal, score)) = results.into_iter().next() {
            if score >= threshold {
                return Ok(Some(goal));
            }
        }
        Ok(None)
    }

    /// Get all goals, optionally including archived.
    pub async fn get_all(&self, include_archived: bool) -> Result<Vec<Goal>, AppError> {
        self.repo.get_all(include_archived).await
    }

    // ── File Operations ─────────────────────────────────────────────────

    /// Parse GOALS.md and sync to database.
    pub async fn sync_from_file(&self) -> Result<SyncResult, AppError> {
        let goals_file = self.config.resolved_file_path();
        let mut created = 0u32;
        let mut updated = 0u32;
        let mut archived = 0u32;

        if !goals_file.exists() {
            return Err(AppError::NotFound(format!(
                "GOALS.md not found at {}",
                goals_file.display()
            )));
        }

        // Check file size
        let metadata = tokio::fs::metadata(&goals_file).await?;
        if metadata.len() > MAX_GOALS_FILE_SIZE {
            return Err(AppError::Validation(format!(
                "GOALS.md file too large (>{MAX_GOALS_FILE_SIZE} bytes)"
            )));
        }

        let content = tokio::fs::read_to_string(&goals_file).await?;
        let parsed_goals = parse_goals_file(&content);

        info!(
            file_path = %goals_file.display(),
            goal_count = parsed_goals.len(),
            "goals_service.sync_from_file.parsed"
        );

        let existing_goals = self.get_all(true).await?;
        let existing_by_content: HashMap<String, &Goal> = existing_goals
            .iter()
            .map(|g| (g.content.to_lowercase().trim().to_owned(), g))
            .collect();
        let mut seen_contents: HashSet<String> = HashSet::new();

        for parsed in &parsed_goals {
            let content_key = parsed.content.to_lowercase().trim().to_owned();
            seen_contents.insert(content_key.clone());

            let status = if parsed.completed {
                GoalStatus::Completed
            } else {
                GoalStatus::Active
            };
            let source = if parsed.is_inferred {
                GoalSource::Inferred
            } else {
                GoalSource::User
            };
            let priority = match parsed.priority.as_str() {
                "high" => GoalPriority::High,
                "low" => GoalPriority::Low,
                _ => GoalPriority::Medium,
            };

            if let Some(existing) = existing_by_content.get(&content_key) {
                // Update existing goal if status or priority changed
                if existing.status != status.as_str()
                    || existing.priority != priority.as_str()
                {
                    self.repo
                        .update_fields(
                            existing.id,
                            None,
                            Some(status),
                            Some(priority),
                            Some(source),
                            None,
                        )
                        .await?;
                    updated += 1;
                }
            } else {
                // Create new goal with embedding
                match self.embedding.embed(&parsed.content).await {
                    Ok(embedding_vec) => {
                        let mut new_goal =
                            Goal::new(parsed.content.clone(), source, priority);
                        new_goal.status = status.as_str().to_owned();
                        new_goal.embedding = Some(serde_json::to_string(&embedding_vec)?);
                        self.repo.save(&new_goal).await?;
                        created += 1;
                    }
                    Err(e) => {
                        warn!(
                            content_preview = &parsed.content[..parsed.content.len().min(50)],
                            error = %e,
                            "goals_service.sync_from_file.embedding_failed"
                        );
                    }
                }
            }
        }

        // Archive goals not in file (user removed them)
        for (content_key, existing) in &existing_by_content {
            if !seen_contents.contains(content_key)
                && existing.status != GoalStatus::Archived.as_str()
            {
                self.repo
                    .update_status(existing.id, Some(GoalStatus::Archived), None)
                    .await?;
                archived += 1;
            }
        }

        let result = SyncResult {
            created,
            updated,
            archived,
        };
        info!(
            created,
            updated,
            archived,
            "goals_service.sync_from_file.complete"
        );
        Ok(result)
    }

    /// Create GOALS.md with template if it doesn't exist.
    pub fn ensure_goals_file_exists(&self) -> Result<(), AppError> {
        let goals_file = self.config.resolved_file_path();
        if goals_file.exists() {
            return Ok(());
        }

        if let Some(parent) = goals_file.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let template = r#"# My Goals

## High Priority
- [ ]

## Medium Priority
- [ ]

## Low Priority
- [ ]

## Completed
- [x] Example completed goal

---
## Inferred by BoBe

"#;
        std::fs::write(&goals_file, template)?;
        info!(file_path = %goals_file.display(), "goals_service.created_template");
        Ok(())
    }
}

impl std::fmt::Debug for GoalsService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoalsService")
            .field("config", &self.config)
            .finish()
    }
}
