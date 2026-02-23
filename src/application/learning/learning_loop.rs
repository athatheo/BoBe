//! Learning loop — background task that runs learners periodically.
//!
//! Responsibilities:
//! 1. Poll for closed conversations → extract memories and goals
//! 2. Poll for raw observations → distill memories
//! 3. Daily consolidation: merge short-term → long-term memories
//! 4. Scheduled pruning: delete old data per retention config

use std::sync::Arc;

use chrono::{Duration, Utc};
use tracing::{debug, error, info, warn};

use crate::application::learners::{GoalLearner, MemoryConsolidator, MemoryLearner};
use crate::application::learning::config::{LearningConfig, RetentionConfig};
use crate::application::services::conversation_service::ConversationService;
use crate::application::services::goals::goals_service::GoalsService;
use crate::domain::learning_state::LearningState;
use crate::domain::types::{MemorySource, MemoryType};
use crate::ports::embedding::EmbeddingProvider;
use crate::ports::repos::goal_repo::GoalRepository;
use crate::ports::repos::learning_state_repo::LearningStateRepository;
use crate::ports::repos::memory_repo::MemoryRepository;
use crate::ports::repos::observation_repo::ObservationRepository;

#[derive(Default)]
struct ReEmbedStats {
    re_embedded: usize,
    deleted: usize,
    skipped: usize,
}

pub struct LearningLoop {
    conversation: Arc<ConversationService>,
    goals_service: Arc<GoalsService>,
    memory_learner: Arc<MemoryLearner>,
    goal_learner: Arc<GoalLearner>,
    memory_consolidator: Arc<MemoryConsolidator>,
    memory_repo: Arc<dyn MemoryRepository>,
    observation_repo: Arc<dyn ObservationRepository>,
    goal_repo: Arc<dyn GoalRepository>,
    learning_state_repo: Arc<dyn LearningStateRepository>,
    embedding: Arc<dyn EmbeddingProvider>,
    config: LearningConfig,
    retention_config: RetentionConfig,
    running: std::sync::atomic::AtomicBool,
    stop_notify: tokio::sync::Notify,
}

impl LearningLoop {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        conversation: Arc<ConversationService>,
        goals_service: Arc<GoalsService>,
        memory_learner: Arc<MemoryLearner>,
        goal_learner: Arc<GoalLearner>,
        memory_consolidator: Arc<MemoryConsolidator>,
        memory_repo: Arc<dyn MemoryRepository>,
        observation_repo: Arc<dyn ObservationRepository>,
        goal_repo: Arc<dyn GoalRepository>,
        learning_state_repo: Arc<dyn LearningStateRepository>,
        embedding: Arc<dyn EmbeddingProvider>,
        config: LearningConfig,
        retention_config: RetentionConfig,
    ) -> Self {
        Self {
            conversation,
            goals_service,
            memory_learner,
            goal_learner,
            memory_consolidator,
            memory_repo,
            observation_repo,
            goal_repo,
            learning_state_repo,
            embedding,
            config,
            retention_config,
            running: std::sync::atomic::AtomicBool::new(false),
            stop_notify: tokio::sync::Notify::new(),
        }
    }

    pub fn update_config(&mut self, config: LearningConfig) {
        self.config = config;
    }

    /// Main learning loop. Runs until stop() is called.
    pub async fn run(&self) {
        self.running.store(true, std::sync::atomic::Ordering::Relaxed);
        info!(
            interval_minutes = self.config.interval_minutes,
            daily_consolidation_hour = self.config.daily_consolidation_hour,
            "learning_loop.starting"
        );

        // Load or create state
        let mut state = match self.learning_state_repo.get_or_create().await {
            Ok(s) => s,
            Err(e) => {
                error!(error = %e, "learning_loop.state_load_failed");
                LearningState::new()
            }
        };

        while self.running.load(std::sync::atomic::Ordering::Relaxed) {
            if let Err(e) = self.learning_cycle(&mut state).await {
                error!(error = %e, "learning_loop.cycle_error");
            }

            // Interruptible sleep
            let sleep_duration = std::time::Duration::from_secs(self.config.interval_minutes * 60);
            tokio::select! {
                _ = tokio::time::sleep(sleep_duration) => {},
                _ = self.stop_notify.notified() => {
                    break;
                }
            }
        }

        info!("learning_loop.stopped");
    }

    /// Gracefully stop the learning loop.
    pub fn stop(&self) {
        info!("learning_loop.stopping");
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
        self.stop_notify.notify_one();
    }

    async fn learning_cycle(&self, state: &mut LearningState) -> Result<(), crate::error::AppError> {
        let start = std::time::Instant::now();
        debug!("learning_loop.cycle_start");

        let mut state_changed = false;

        // 1. Process closed conversations
        let (conv_count, conv_memories, conv_goals, conv_changed) =
            self.process_closed_conversations(state).await;
        if conv_changed {
            state_changed = true;
        }

        // 2. Process accumulated context
        let (ctx_items, ctx_memories, ctx_changed) =
            self.process_accumulated_context(state).await;
        if ctx_changed {
            state_changed = true;
        }

        // 3. Daily consolidation
        let (consolidated, consolidation_changed) =
            self.daily_consolidation_if_needed(state).await;
        if consolidation_changed {
            state_changed = true;
        }

        // 4. Scheduled pruning
        let (pruned, pruning_changed) =
            self.scheduled_pruning_if_needed(state).await;
        if pruning_changed {
            state_changed = true;
        }

        // 5. Re-embed records with null embeddings
        let re_embed_stats = self.re_embed_null_records().await;

        // Save state
        if state_changed {
            state.updated_at = Utc::now();
            if let Err(e) = self.learning_state_repo.save(state).await {
                warn!(error = %e, "learning_loop.state_save_failed");
            }
        }

        let total_ms = start.elapsed().as_millis();
        info!(
            conversations = conv_count,
            memories_from_conversations = conv_memories,
            goals_extracted = conv_goals,
            observations_processed = ctx_items,
            memories_from_context = ctx_memories,
            consolidated = consolidated,
            pruned = pruned,
            re_embedded = re_embed_stats.re_embedded,
            re_embed_deleted = re_embed_stats.deleted,
            total_ms = total_ms,
            "learning_loop.cycle_complete"
        );

        Ok(())
    }

    async fn process_closed_conversations(
        &self,
        state: &mut LearningState,
    ) -> (usize, usize, usize, bool) {
        let conversations = match self.conversation
            .get_closed_since(state.last_conversation_processed_at)
            .await
        {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "learning_loop.get_closed_failed");
                return (0, 0, 0, false);
            }
        };

        if conversations.is_empty() {
            return (0, 0, 0, false);
        }

        debug!(count = conversations.len(), "learning_loop.processing_conversations");

        // Get existing for dedup
        let existing_memories = self.get_all_memories().await;
        let existing_goals = self.goals_service.get_active(100).await.unwrap_or_default();

        let mut total_memories = 0usize;
        let mut total_goals = 0usize;
        let mut all_memories = existing_memories;
        let mut all_goals = existing_goals;
        let mut processed_closed_times: Vec<chrono::DateTime<chrono::Utc>> = Vec::new();

        for conv in &conversations {
            let turns = match self.conversation.get_conversation_turns(conv.id, 100).await {
                Ok(t) => t,
                Err(e) => {
                    warn!(conversation_id = %conv.id, error = %e, "learning_loop.conversation_turns_failed");
                    continue;
                }
            };

            let turn_tuples: Vec<(String, String)> = turns
                .iter()
                .map(|t| (t.role.clone(), t.content.clone()))
                .collect();

            if turn_tuples.is_empty() {
                // Empty conversation — still mark as processed
                if let Some(closed) = conv.closed_at {
                    processed_closed_times.push(closed);
                }
                continue;
            }

            // Extract memories
            let memories = self.memory_learner
                .distill_from_conversation(&turn_tuples, &all_memories)
                .await;
            total_memories += memories.len();
            all_memories.extend(memories);

            // Extract goals
            let goals = self.goal_learner
                .extract_from_conversation(&turn_tuples, &all_goals)
                .await;
            total_goals += goals.len();
            all_goals.extend(goals);

            // Only advance timestamp for successfully processed conversations
            if let Some(closed) = conv.closed_at {
                processed_closed_times.push(closed);
            }
        }

        // Update state only based on successfully processed conversations
        let mut changed = false;
        if let Some(&latest) = processed_closed_times.iter().max() {
            state.last_conversation_processed_at = Some(latest);
            changed = true;
        }

        (conversations.len(), total_memories, total_goals, changed)
    }

    async fn process_accumulated_context(
        &self,
        state: &mut LearningState,
    ) -> (usize, usize, bool) {
        let observations = match self.observation_repo
            .find_since(state.last_context_processed_at, Some(self.config.max_context_per_cycle as i64 * 2))
            .await
        {
            Ok(o) => o,
            Err(e) => {
                warn!(error = %e, "learning_loop.get_observations_failed");
                return (0, 0, false);
            }
        };

        // Filter out already-processed sources
        let observations: Vec<_> = observations
            .into_iter()
            .filter(|obs| obs.source != "user_message" && obs.source != "screen")
            .collect();

        if (observations.len() as u32) < self.config.min_context_items {
            return (0, 0, false);
        }

        let to_process = &observations[..observations.len().min(self.config.max_context_per_cycle as usize)];
        let existing_memories = self.get_all_memories().await;
        let goals = self.goals_service.get_active(100).await.unwrap_or_default();

        let memories = self.memory_learner
            .distill_from_observations(to_process, &existing_memories, &goals)
            .await;

        // Only advance state if processing actually produced results
        let mut changed = false;
        if !memories.is_empty() {
            let created_times: Vec<_> = to_process.iter().map(|o| o.created_at).collect();
            if let Some(&latest) = created_times.iter().max() {
                state.last_context_processed_at = Some(latest);
                changed = true;
            }
        }

        (to_process.len(), memories.len(), changed)
    }

    async fn daily_consolidation_if_needed(
        &self,
        state: &mut LearningState,
    ) -> (usize, bool) {
        let now = Utc::now();

        let should_run = match state.last_consolidation_at {
            None => now.format("%H").to_string().parse::<u32>().unwrap_or(0) == self.config.daily_consolidation_hour,
            Some(last) => {
                last.date_naive() < now.date_naive()
                    && now.format("%H").to_string().parse::<u32>().unwrap_or(0) >= self.config.daily_consolidation_hour
            }
        };

        if !should_run {
            return (0, false);
        }

        info!("learning_loop.starting_consolidation");

        let short_term = self.memory_repo
            .find_by_type(MemoryType::ShortTerm, false, None)
            .await
            .unwrap_or_default();

        // Exclude visual diary entries
        let short_term: Vec<_> = short_term
            .into_iter()
            .filter(|m| m.source != MemorySource::VisualDiary.as_str())
            .collect();

        if short_term.is_empty() {
            state.last_consolidation_at = Some(now);
            return (0, true);
        }

        let long_term = self.memory_consolidator.consolidate(&short_term).await;
        state.last_consolidation_at = Some(now);

        info!(
            short_term_input = short_term.len(),
            long_term_output = long_term.len(),
            "learning_loop.consolidation_complete"
        );

        (long_term.len(), true)
    }

    async fn scheduled_pruning_if_needed(
        &self,
        state: &mut LearningState,
    ) -> (usize, bool) {
        if !self.retention_config.pruning_enabled {
            return (0, false);
        }

        let now = Utc::now();

        if let Some(last) = state.last_pruning_at
            && last.date_naive() >= now.date_naive() {
                return (0, false);
            }

        info!("learning_loop.starting_pruning");

        let obs_deleted = self.observation_repo
            .delete_older_than(self.retention_config.raw_context_days as i64)
            .await
            .unwrap_or(0);

        let st_cutoff = now - Duration::days(self.retention_config.short_term_memory_days as i64);
        let st_deleted = self.memory_repo
            .delete_by_criteria(MemoryType::ShortTerm, st_cutoff)
            .await
            .unwrap_or(0);

        let lt_cutoff = now - Duration::days(self.retention_config.long_term_memory_days as i64);
        let lt_deleted = self.memory_repo
            .delete_by_criteria(MemoryType::LongTerm, lt_cutoff)
            .await
            .unwrap_or(0);

        // Delete stale archived/completed goals
        let goal_cutoff = now - Duration::days(self.retention_config.goal_retention_days as i64);
        let goals_deleted = self.goal_repo
            .delete_stale_goals(
                &[crate::domain::types::GoalStatus::Archived, crate::domain::types::GoalStatus::Completed],
                goal_cutoff,
            )
            .await
            .unwrap_or(0);

        let total_deleted = obs_deleted + st_deleted + lt_deleted + goals_deleted as i64;
        state.last_pruning_at = Some(now);

        info!(
            total_deleted = total_deleted,
            observations = obs_deleted,
            short_term = st_deleted,
            long_term = lt_deleted,
            goals = goals_deleted,
            "learning_loop.pruning_complete"
        );

        (total_deleted as usize, true)
    }

    async fn re_embed_null_records(&self) -> ReEmbedStats {
        let mut stats = ReEmbedStats::default();

        // Observations
        let (re, del, skip) = self.re_embed_observations(50).await;
        stats.re_embedded += re;
        stats.deleted += del;
        stats.skipped += skip;

        // Memories
        let (re, del, skip) = self.re_embed_memories(50).await;
        stats.re_embedded += re;
        stats.deleted += del;
        stats.skipped += skip;

        // Goals
        let (re, del, skip) = self.re_embed_goals(50).await;
        stats.re_embedded += re;
        stats.deleted += del;
        stats.skipped += skip;

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

    async fn re_embed_observations(&self, limit: i64) -> (usize, usize, usize) {
        let records = match self.observation_repo.find_null_embedding(limit).await {
            Ok(r) => r,
            Err(e) => {
                warn!(error = %e, "learning_loop.re_embed_observations_fetch_failed");
                return (0, 0, 0);
            }
        };

        let mut re_embedded = 0usize;
        let mut deleted = 0usize;
        let mut skipped = 0usize;

        for record in records {
            if record.content.trim().is_empty() {
                let _ = self.observation_repo.delete(record.id).await;
                deleted += 1;
                continue;
            }
            match self.embedding.embed(&record.content).await {
                Ok(emb) if !emb.is_empty() => {
                    if let Err(e) = self.observation_repo.update_embedding(record.id, &emb).await {
                        warn!(error = %e, id = %record.id, "learning_loop.re_embed_observation_update_failed");
                        skipped += 1;
                    } else {
                        re_embedded += 1;
                    }
                }
                _ => {
                    let _ = self.observation_repo.delete(record.id).await;
                    deleted += 1;
                }
            }
        }

        (re_embedded, deleted, skipped)
    }

    async fn re_embed_memories(&self, limit: i64) -> (usize, usize, usize) {
        let records = match self.memory_repo.find_null_embedding(limit).await {
            Ok(r) => r,
            Err(e) => {
                warn!(error = %e, "learning_loop.re_embed_memories_fetch_failed");
                return (0, 0, 0);
            }
        };

        let mut re_embedded = 0usize;
        let mut deleted = 0usize;
        let mut skipped = 0usize;

        for record in records {
            if record.content.trim().is_empty() {
                let _ = self.memory_repo.delete(record.id).await;
                deleted += 1;
                continue;
            }
            match self.embedding.embed(&record.content).await {
                Ok(emb) if !emb.is_empty() => {
                    if let Err(e) = self.memory_repo.update_embedding(record.id, &emb).await {
                        warn!(error = %e, id = %record.id, "learning_loop.re_embed_memory_update_failed");
                        skipped += 1;
                    } else {
                        re_embedded += 1;
                    }
                }
                _ => {
                    let _ = self.memory_repo.delete(record.id).await;
                    deleted += 1;
                }
            }
        }

        (re_embedded, deleted, skipped)
    }

    async fn re_embed_goals(&self, limit: i64) -> (usize, usize, usize) {
        let records = match self.goal_repo.find_null_embedding(limit).await {
            Ok(r) => r,
            Err(e) => {
                warn!(error = %e, "learning_loop.re_embed_goals_fetch_failed");
                return (0, 0, 0);
            }
        };

        let mut re_embedded = 0usize;
        let mut deleted = 0usize;
        let mut skipped = 0usize;

        for record in records {
            if record.content.trim().is_empty() {
                let _ = self.goal_repo.delete(record.id).await;
                deleted += 1;
                continue;
            }
            match self.embedding.embed(&record.content).await {
                Ok(emb) if !emb.is_empty() => {
                    if let Err(e) = self.goal_repo.update_embedding(record.id, &emb).await {
                        warn!(error = %e, id = %record.id, "learning_loop.re_embed_goal_update_failed");
                        skipped += 1;
                    } else {
                        re_embedded += 1;
                    }
                }
                _ => {
                    let _ = self.goal_repo.delete(record.id).await;
                    deleted += 1;
                }
            }
        }

        (re_embedded, deleted, skipped)
    }

    async fn get_all_memories(&self) -> Vec<crate::domain::memory::Memory> {
        let mut all = Vec::new();
        if let Ok(st) = self.memory_repo.find_by_type(MemoryType::ShortTerm, false, None).await {
            all.extend(st);
        }
        if let Ok(lt) = self.memory_repo.find_by_type(MemoryType::LongTerm, false, None).await {
            all.extend(lt);
        }
        all
    }
}
