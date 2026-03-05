//! Learning loop maintenance: consolidation, pruning, and re-embedding.

use chrono::{Duration, Timelike, Utc};
use tracing::{debug, info, warn};

use crate::error::AppError;
use crate::llm::EmbeddingProvider;
use crate::models::types::{MemorySource, MemoryType};

use super::learning_loop::LearningDeps;

/// Returns `(consolidated_count, state_changed)`.
pub(crate) async fn daily_consolidation_if_needed(
    deps: &LearningDeps,
    state: &mut crate::models::learning_state::LearningState,
) -> (usize, bool) {
    let cfg = deps.config.load();
    let now = Utc::now();

    let should_run = match state.last_consolidation_at {
        None => now.hour() == cfg.learning.daily_consolidation_hour,
        Some(last) => {
            last.date_naive() < now.date_naive()
                && now.hour() >= cfg.learning.daily_consolidation_hour
        }
    };

    if !should_run {
        return (0, false);
    }

    info!("learning_loop.starting_consolidation");

    let short_term = match deps
        .memory_repo
        .find_by_type(MemoryType::ShortTerm, false, None)
        .await
    {
        Ok(memories) => memories,
        Err(e) => {
            warn!(error = %e, "learning_loop.short_term_fetch_failed");
            return (0, false);
        }
    };

    let total_short_term = short_term.len();
    let (short_term, deferred_visual_diary): (Vec<_>, Vec<_>) =
        short_term.into_iter().partition(is_consolidation_candidate);

    if !deferred_visual_diary.is_empty() {
        debug!(
            total_short_term = total_short_term,
            consolidation_candidates = short_term.len(),
            deferred_visual_diary = deferred_visual_diary.len(),
            "learning_loop.visual_diary_deferred"
        );
    }

    if short_term.is_empty() {
        state.last_consolidation_at = Some(now);
        return (0, true);
    }

    let long_term = deps.memory_consolidator.consolidate(&short_term).await;
    state.last_consolidation_at = Some(now);

    info!(
        short_term_input = short_term.len(),
        long_term_output = long_term.len(),
        "learning_loop.consolidation_complete"
    );

    (long_term.len(), true)
}

pub(crate) fn is_consolidation_candidate(memory: &crate::models::memory::Memory) -> bool {
    if memory.source != MemorySource::VisualDiary {
        return true;
    }

    !memory
        .content
        .trim_start()
        .to_ascii_lowercase()
        .starts_with("# visual memory")
}

/// Returns `(total_deleted, state_changed)`.
pub(crate) async fn scheduled_pruning_if_needed(
    deps: &LearningDeps,
    state: &mut crate::models::learning_state::LearningState,
) -> (usize, bool) {
    let cfg = deps.config.load();

    if !cfg.memory.pruning_enabled {
        return (0, false);
    }

    let now = Utc::now();

    if let Some(last) = state.last_pruning_at
        && last.date_naive() >= now.date_naive()
    {
        return (0, false);
    }

    info!("learning_loop.starting_pruning");

    let obs_deleted = match deps
        .observation_repo
        .delete_older_than(cfg.memory.raw_context_retention_days as i64)
        .await
    {
        Ok(n) => n,
        Err(e) => {
            warn!(error = %e, "learning_loop.prune_observations_failed");
            0
        }
    };

    let st_cutoff = now - Duration::days(cfg.memory.short_term_retention_days as i64);
    let st_deleted = match deps
        .memory_repo
        .delete_by_criteria(MemoryType::ShortTerm, st_cutoff)
        .await
    {
        Ok(n) => n,
        Err(e) => {
            warn!(error = %e, "learning_loop.prune_short_term_failed");
            0
        }
    };

    let lt_cutoff = now - Duration::days(cfg.memory.long_term_retention_days as i64);
    let lt_deleted = match deps
        .memory_repo
        .delete_by_criteria(MemoryType::LongTerm, lt_cutoff)
        .await
    {
        Ok(n) => n,
        Err(e) => {
            warn!(error = %e, "learning_loop.prune_long_term_failed");
            0
        }
    };

    let goal_cutoff = now - Duration::days(cfg.memory.goal_retention_days as i64);
    let goals_deleted = match deps
        .goal_repo
        .delete_stale_goals(
            &[
                crate::models::types::GoalStatus::Archived,
                crate::models::types::GoalStatus::Completed,
            ],
            goal_cutoff,
        )
        .await
    {
        Ok(n) => n,
        Err(e) => {
            warn!(error = %e, "learning_loop.prune_stale_goals_failed");
            0
        }
    };

    let total_deleted =
        obs_deleted + st_deleted + lt_deleted + i64::try_from(goals_deleted).unwrap_or(0);
    state.last_pruning_at = Some(now);

    info!(
        total_deleted = total_deleted,
        observations = obs_deleted,
        short_term = st_deleted,
        long_term = lt_deleted,
        goals = goals_deleted,
        "learning_loop.pruning_complete"
    );

    (usize::try_from(total_deleted).unwrap_or(0), true)
}

#[derive(Default)]
pub(crate) struct ReEmbedStats {
    pub(crate) re_embedded: usize,
    pub(crate) deleted: usize,
    pub(crate) skipped: usize,
}

pub(crate) async fn re_embed_null_records(deps: &LearningDeps) -> ReEmbedStats {
    let mut stats = ReEmbedStats::default();

    if let Ok(records) = deps.observation_repo.find_null_embedding(50).await {
        let (r, d, s) = re_embed_batch(
            records.into_iter().map(|o| (o.id, o.content)).collect(),
            &*deps.embedding,
            |id, emb| {
                let repo = deps.observation_repo.clone();
                Box::pin(async move { repo.update_embedding(id, &emb).await })
            },
            |id| {
                let repo = deps.observation_repo.clone();
                Box::pin(async move {
                    let _ = repo.delete(id).await;
                })
            },
        )
        .await;
        stats.re_embedded += r;
        stats.deleted += d;
        stats.skipped += s;
    }

    if let Ok(records) = deps.memory_repo.find_null_embedding(50).await {
        let (r, d, s) = re_embed_batch(
            records.into_iter().map(|m| (m.id, m.content)).collect(),
            &*deps.embedding,
            |id, emb| {
                let repo = deps.memory_repo.clone();
                Box::pin(async move { repo.update_embedding(id, &emb).await })
            },
            |id| {
                let repo = deps.memory_repo.clone();
                Box::pin(async move {
                    let _ = repo.delete(id).await;
                })
            },
        )
        .await;
        stats.re_embedded += r;
        stats.deleted += d;
        stats.skipped += s;
    }

    if let Ok(records) = deps.goal_repo.find_null_embedding(50).await {
        let (r, d, s) = re_embed_batch(
            records.into_iter().map(|g| (g.id, g.content)).collect(),
            &*deps.embedding,
            |id, emb| {
                let repo = deps.goal_repo.clone();
                Box::pin(async move { repo.update_embedding(id, &emb).await })
            },
            |id| {
                let repo = deps.goal_repo.clone();
                Box::pin(async move {
                    let _ = repo.delete(id).await;
                })
            },
        )
        .await;
        stats.re_embedded += r;
        stats.deleted += d;
        stats.skipped += s;
    }

    if stats.re_embedded > 0 || stats.deleted > 0 {
        info!(
            re_embedded = stats.re_embedded,
            deleted = stats.deleted,
            skipped = stats.skipped,
            "learning_loop.re_embed_complete"
        );
    }

    stats
}

/// Returns `(re_embedded, deleted, skipped)`.
async fn re_embed_batch<U, D>(
    records: Vec<(uuid::Uuid, String)>,
    embedding: &dyn EmbeddingProvider,
    on_update: impl Fn(uuid::Uuid, Vec<f32>) -> U,
    on_delete: impl Fn(uuid::Uuid) -> D,
) -> (usize, usize, usize)
where
    U: std::future::Future<Output = Result<(), AppError>>,
    D: std::future::Future<Output = ()>,
{
    let mut re_embedded = 0usize;
    let mut deleted = 0usize;
    let mut skipped = 0usize;

    for (id, content) in records {
        if content.trim().is_empty() {
            on_delete(id).await;
            deleted += 1;
            continue;
        }
        match embedding.embed(&content).await {
            Ok(emb) if !emb.is_empty() => {
                if on_update(id, emb).await.is_ok() {
                    re_embedded += 1;
                } else {
                    skipped += 1;
                }
            }
            _ => {
                on_delete(id).await;
                deleted += 1;
            }
        }
    }

    (re_embedded, deleted, skipped)
}
