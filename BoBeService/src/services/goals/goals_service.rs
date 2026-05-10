use std::sync::Arc;

use chrono::Utc;
use tracing::{info, warn};

use crate::error::AppError;
use crate::models::ids::GoalId;
use crate::models::types::GoalStatus;

use super::file_store::GoalFileStore;
use super::goal_md::GoalDoc;

pub(crate) struct GoalsService {
    store: Arc<GoalFileStore>,
}

impl GoalsService {
    pub(crate) fn new(store: Arc<GoalFileStore>) -> Self {
        Self { store }
    }

    /// Priority desc, then updated_at desc.
    pub(crate) async fn list_all(&self) -> Result<Vec<GoalDoc>, AppError> {
        let mut goals = self.store.list().await?;
        sort_goals(&mut goals);
        Ok(goals)
    }

    pub(crate) async fn list_active(&self) -> Result<Vec<GoalDoc>, AppError> {
        let mut goals = self.store.list().await?;
        goals.retain(|g| g.status == GoalStatus::Active);
        sort_goals(&mut goals);
        Ok(goals)
    }

    pub(crate) async fn get(&self, id: GoalId) -> Result<Option<GoalDoc>, AppError> {
        self.store.get(id).await
    }

    /// Always assigns fresh id to avoid clashes with agent-written files.
    pub(crate) async fn create(&self, mut doc: GoalDoc) -> Result<GoalDoc, AppError> {
        doc.id = GoalId::new();
        let now = Utc::now();
        doc.created_at = now;
        doc.updated_at = now;
        if doc.status_is_default() {
            doc.status = GoalStatus::Active;
        }
        self.store.save(&doc).await?;
        info!(
            goal_id = %doc.id,
            title = %doc.title,
            "goals_service.created"
        );
        Ok(doc)
    }

    /// `Some` overwrites, `None` leaves as-is; bumps `updated_at` on any change.
    #[allow(
        clippy::too_many_arguments,
        reason = "patch surface mirrors the API request shape; explicit options are clearer than a builder"
    )]
    pub(crate) async fn update(
        &self,
        id: GoalId,
        title: Option<String>,
        status: Option<GoalStatus>,
        priority: Option<u8>,
        summary: Option<String>,
        why_it_matters: Option<String>,
        notes: Option<String>,
    ) -> Result<Option<GoalDoc>, AppError> {
        let Some(mut doc) = self.store.get(id).await? else {
            return Ok(None);
        };

        let mut changed = false;
        if let Some(t) = title {
            doc.title = t;
            changed = true;
        }
        if let Some(s) = status {
            doc.status = s;
            changed = true;
        }
        if let Some(p) = priority {
            doc.priority = p;
            changed = true;
        }
        if let Some(s) = summary {
            doc.summary = s;
            changed = true;
        }
        if let Some(w) = why_it_matters {
            doc.why_it_matters = w;
            changed = true;
        }
        if let Some(n) = notes {
            doc.notes = n;
            changed = true;
        }

        if !changed {
            return Ok(Some(doc));
        }

        doc.updated_at = Utc::now();
        self.store.save(&doc).await?;
        info!(goal_id = %id, "goals_service.updated");
        Ok(Some(doc))
    }

    pub(crate) async fn set_status(
        &self,
        id: GoalId,
        status: GoalStatus,
    ) -> Result<Option<GoalDoc>, AppError> {
        self.update(id, None, Some(status), None, None, None, None)
            .await
    }

    pub(crate) async fn delete(&self, id: GoalId) -> Result<bool, AppError> {
        let removed = self.store.delete(id).await?;
        if removed {
            info!(goal_id = %id, "goals_service.deleted");
        } else {
            warn!(goal_id = %id, "goals_service.delete_missing");
        }
        Ok(removed)
    }
}

fn sort_goals(goals: &mut [GoalDoc]) {
    goals.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then(b.updated_at.cmp(&a.updated_at))
    });
}

trait GoalDocExt {
    fn status_is_default(&self) -> bool;
}

impl GoalDocExt for GoalDoc {
    fn status_is_default(&self) -> bool {
        self.status == GoalStatus::Active
    }
}
