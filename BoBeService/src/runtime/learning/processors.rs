//! Extracts memories/goals from closed conversations and distills observations.

use tracing::{debug, warn};

use crate::models::memory::Memory;
use crate::models::types::{MemoryType, ObservationSource};

use super::learning_loop::LearningDeps;

/// Returns `(conversations_found, memories_created, goals_created, state_changed)`.
pub(crate) async fn process_closed_conversations(
    deps: &LearningDeps,
    state: &mut crate::models::learning_state::LearningState,
) -> (usize, usize, usize, bool) {
    let conversations = match deps
        .conversation
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

    debug!(
        count = conversations.len(),
        "learning_loop.processing_conversations"
    );

    let existing_memories = get_all_memories(deps).await;
    let existing_goals = deps.goals_service.get_active(100).await.unwrap_or_default();

    const MAX_ACCUMULATED_ITEMS: usize = 500;

    let mut total_memories = 0usize;
    let mut total_goals = 0usize;
    let mut all_memories = existing_memories;
    let mut all_goals = existing_goals;
    let mut processed_closed_times: Vec<chrono::DateTime<chrono::Utc>> = Vec::new();

    for conv in &conversations {
        if all_memories.len() >= MAX_ACCUMULATED_ITEMS {
            warn!(
                cap = MAX_ACCUMULATED_ITEMS,
                accumulated = all_memories.len(),
                remaining = conversations.len() - processed_closed_times.len(),
                "learning_loop.memory_cap_reached — stopping conversation processing this cycle"
            );
            break;
        }
        let turns = match deps.conversation.get_conversation_turns(conv.id, 100).await {
            Ok(t) => t,
            Err(e) => {
                warn!(conversation_id = %conv.id, error = %e, "learning_loop.conversation_turns_failed");
                continue;
            }
        };

        let turn_tuples: Vec<(String, String)> = turns
            .iter()
            .map(|t| (t.role.as_str().to_owned(), t.content.clone()))
            .collect();

        if turn_tuples.is_empty() {
            if let Some(closed) = conv.closed_at {
                processed_closed_times.push(closed);
            }
            continue;
        }

        let memories = deps
            .memory_learner
            .distill_from_conversation(&turn_tuples, &all_memories)
            .await;
        total_memories += memories.len();
        all_memories.extend(memories);
        if all_memories.len() > MAX_ACCUMULATED_ITEMS {
            warn!("memory_cap_exceeded, truncating");
            all_memories.truncate(MAX_ACCUMULATED_ITEMS);
        }

        let goals = deps
            .goal_learner
            .extract_from_conversation(&turn_tuples, &all_goals)
            .await;
        total_goals += goals.len();
        all_goals.extend(goals);

        if let Some(closed) = conv.closed_at {
            processed_closed_times.push(closed);
        }
    }

    let mut changed = false;
    if let Some(&latest) = processed_closed_times.iter().max() {
        state.last_conversation_processed_at = Some(latest);
        changed = true;
    }

    (conversations.len(), total_memories, total_goals, changed)
}

/// Returns `(items_processed, memories_created, state_changed)`.
pub(crate) async fn process_accumulated_context(
    deps: &LearningDeps,
    state: &mut crate::models::learning_state::LearningState,
) -> (usize, usize, bool) {
    let cfg = deps.config.load();
    let observations = match deps
        .observation_repo
        .find_since(
            state.last_context_processed_at,
            Some(cfg.learning.max_context_per_cycle as i64 * 2),
        )
        .await
    {
        Ok(o) => o,
        Err(e) => {
            warn!(error = %e, "learning_loop.get_observations_failed");
            return (0, 0, false);
        }
    };

    let observations: Vec<_> = observations
        .into_iter()
        .filter(|obs| {
            obs.source != ObservationSource::UserMessage && obs.source != ObservationSource::Screen
        })
        .collect();

    if (observations.len() as u32) < cfg.learning.min_context_items {
        return (0, 0, false);
    }

    let to_process = &observations[..observations
        .len()
        .min(cfg.learning.max_context_per_cycle as usize)];
    let existing_memories = get_all_memories(deps).await;
    let goals = deps.goals_service.get_active(100).await.unwrap_or_default();

    let memories = deps
        .memory_learner
        .distill_from_observations(to_process, &existing_memories, &goals)
        .await;

    // Advance timestamp regardless of whether memories were produced
    let mut changed = false;
    let created_times: Vec<_> = to_process.iter().map(|o| o.created_at).collect();
    if let Some(&latest) = created_times.iter().max() {
        state.last_context_processed_at = Some(latest);
        changed = true;
    }

    (to_process.len(), memories.len(), changed)
}

async fn get_all_memories(deps: &LearningDeps) -> Vec<Memory> {
    let mut all = Vec::new();
    if let Ok(st) = deps
        .memory_repo
        .find_by_type(MemoryType::ShortTerm, false, None)
        .await
    {
        all.extend(st);
    }
    if let Ok(lt) = deps
        .memory_repo
        .find_by_type(MemoryType::LongTerm, false, None)
        .await
    {
        all.extend(lt);
    }
    all
}
