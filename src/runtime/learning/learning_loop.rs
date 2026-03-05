//! Background loop: conversation learning, observation distillation, consolidation, and pruning.

use std::sync::Arc;

use arc_swap::ArcSwap;
use chrono::Utc;
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::db::{GoalRepository, LearningStateRepository, MemoryRepository, ObservationRepository};
use crate::llm::EmbeddingProvider;
use crate::models::learning_state::LearningState;
use crate::runtime::learners::{GoalLearner, MemoryConsolidator, MemoryLearner};
use crate::services::conversation_service::ConversationService;
use crate::services::goals::goals_service::GoalsService;

use super::maintenance;
use super::processors;

pub(crate) struct LearningDeps {
    pub(crate) conversation: Arc<ConversationService>,
    pub(crate) goals_service: Arc<GoalsService>,
    pub(crate) memory_learner: Arc<MemoryLearner>,
    pub(crate) goal_learner: Arc<GoalLearner>,
    pub(crate) memory_consolidator: Arc<MemoryConsolidator>,
    pub(crate) memory_repo: Arc<dyn MemoryRepository>,
    pub(crate) observation_repo: Arc<dyn ObservationRepository>,
    pub(crate) goal_repo: Arc<dyn GoalRepository>,
    pub(crate) embedding: Arc<dyn EmbeddingProvider>,
    pub(crate) config: Arc<ArcSwap<Config>>,
}

pub(crate) struct LearningLoop {
    deps: LearningDeps,
    learning_state_repo: Arc<dyn LearningStateRepository>,
    running: std::sync::atomic::AtomicBool,
    stop_notify: tokio::sync::Notify,
}

impl LearningLoop {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
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
        config: Arc<ArcSwap<Config>>,
    ) -> Self {
        Self {
            deps: LearningDeps {
                conversation,
                goals_service,
                memory_learner,
                goal_learner,
                memory_consolidator,
                memory_repo,
                observation_repo,
                goal_repo,
                embedding,
                config,
            },
            learning_state_repo,
            running: std::sync::atomic::AtomicBool::new(false),
            stop_notify: tokio::sync::Notify::new(),
        }
    }

    pub(crate) async fn run(&self) {
        self.running
            .store(true, std::sync::atomic::Ordering::Release);
        let cfg = self.deps.config.load();
        info!(
            interval_minutes = cfg.learning.interval_minutes,
            daily_consolidation_hour = cfg.learning.daily_consolidation_hour,
            "learning_loop.starting"
        );

        let mut state = match self.learning_state_repo.get_or_create().await {
            Ok(s) => s,
            Err(e) => {
                error!(error = %e, "learning_loop.state_load_failed");
                LearningState::new()
            }
        };

        while self.running.load(std::sync::atomic::Ordering::Acquire) {
            if let Err(e) = self.learning_cycle(&mut state).await {
                error!(error = %e, "learning_loop.cycle_error");
            }

            let cfg = self.deps.config.load();
            let sleep_duration = std::time::Duration::from_mins(cfg.learning.interval_minutes);
            tokio::select! {
                () = tokio::time::sleep(sleep_duration) => {},
                () = self.stop_notify.notified() => {
                    break;
                }
            }
        }

        info!("learning_loop.stopped");
    }

    pub(crate) fn stop(&self) {
        info!("learning_loop.stopping");
        self.running
            .store(false, std::sync::atomic::Ordering::Release);
        self.stop_notify.notify_one();
    }

    async fn learning_cycle(
        &self,
        state: &mut LearningState,
    ) -> Result<(), crate::error::AppError> {
        let start = std::time::Instant::now();
        debug!("learning_loop.cycle_start");

        let mut state_changed = false;

        let (conv_count, conv_memories, conv_goals, conv_changed) =
            processors::process_closed_conversations(&self.deps, state).await;
        if conv_changed {
            state_changed = true;
        }

        let (ctx_items, ctx_memories, ctx_changed) =
            processors::process_accumulated_context(&self.deps, state).await;
        if ctx_changed {
            state_changed = true;
        }

        let (consolidated, consolidation_changed) =
            maintenance::daily_consolidation_if_needed(&self.deps, state).await;
        if consolidation_changed {
            state_changed = true;
        }

        let (pruned, pruning_changed) =
            maintenance::scheduled_pruning_if_needed(&self.deps, state).await;
        if pruning_changed {
            state_changed = true;
        }

        let re_embed_stats = maintenance::re_embed_null_records(&self.deps).await;

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
}

#[cfg(test)]
mod tests {
    use crate::models::memory::Memory;
    use crate::models::types::{MemorySource, MemoryType};
    use crate::runtime::learning::maintenance;

    #[test]
    fn excludes_visual_diary_rollup_entries() {
        let memory = Memory::new(
            "# Visual Memory 2026-02-03 PM\n\n- 12:03 [abc123] User edited Rust code".into(),
            MemoryType::ShortTerm,
            MemorySource::VisualDiary,
            "observation".into(),
        );

        assert!(!maintenance::is_consolidation_candidate(&memory));
    }

    #[test]
    fn keeps_non_diary_visual_entries() {
        let memory = Memory::new(
            "User often reviews screenshots before morning standup.".into(),
            MemoryType::ShortTerm,
            MemorySource::VisualDiary,
            "pattern".into(),
        );

        assert!(maintenance::is_consolidation_candidate(&memory));
    }
}
