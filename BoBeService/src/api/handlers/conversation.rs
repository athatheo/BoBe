use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::error::AppError;
use crate::models::ids::{ConversationId, ConversationTurnId};
use crate::models::types::TurnRole;
use crate::runtime::session::UserMessageGuard;

/// RAII counter guard. Increments on construction; decrements on drop
/// (including panic-unwind). Used to track in-flight text-turn tasks so
/// the graceful-shutdown path can wait for them before closing the DB
/// pool — otherwise the spawned task panics mid-sqlx call.
struct InFlightCounter(Arc<AtomicUsize>);

impl InFlightCounter {
    fn new(counter: Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, Ordering::AcqRel);
        Self(counter)
    }
}

impl Drop for InFlightCounter {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

#[derive(Debug, sqlx::FromRow)]
struct MessageRequestRecord {
    content_sha256: Vec<u8>,
    content: String,
    status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessageRequestStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

impl MessageRequestStatus {
    fn parse(value: &str) -> Result<Self, AppError> {
        match value {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            _ => Err(AppError::Internal(format!(
                "Unknown message request status: {value}"
            ))),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ConversationMessageRequest {
    pub(crate) content: String,
    pub(crate) request_id: Uuid,
}

#[derive(Debug, Serialize)]
pub(crate) struct ConversationMessageResponse {
    pub(crate) message_id: String,
    pub(crate) request_status: &'static str,
    pub(crate) replayed: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct ConversationSnapshotResponse {
    pub(crate) conversation_id: Option<ConversationId>,
    pub(crate) turns: Vec<ConversationTurnResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ConversationTurnResponse {
    pub(crate) id: ConversationTurnId,
    pub(crate) message_id: String,
    pub(crate) role: TurnRole,
    pub(crate) content: String,
    pub(crate) is_complete: bool,
    pub(crate) created_at: chrono::DateTime<chrono::Utc>,
}

pub(crate) async fn current_conversation(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ConversationSnapshotResponse>, AppError> {
    let Some((conversation, turns)) = state
        .runtime
        .runtime_session
        .current_conversation(200)
        .await?
    else {
        return Ok(Json(ConversationSnapshotResponse {
            conversation_id: None,
            turns: Vec::new(),
        }));
    };
    Ok(Json(ConversationSnapshotResponse {
        conversation_id: Some(conversation.id),
        turns: turns
            .into_iter()
            .map(|turn| ConversationTurnResponse {
                id: turn.id,
                message_id: crate::models::ids::message_id_for_turn(turn.id),
                role: turn.role,
                content: turn.content,
                is_complete: turn.is_complete,
                created_at: turn.created_at,
            })
            .collect(),
    }))
}

pub(crate) async fn send_message(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ConversationMessageRequest>,
) -> Result<Json<ConversationMessageResponse>, AppError> {
    if body.content.trim().is_empty() {
        return Err(AppError::Validation("content must not be empty".into()));
    }
    let content_sha256 = Sha256::digest(body.content.as_bytes()).to_vec();
    let assistant_turn_id = ConversationTurnId::from(body.request_id);
    let message_id = crate::models::ids::message_id_for_turn(assistant_turn_id);
    let existing = sqlx::query_as::<_, MessageRequestRecord>(
        "SELECT content_sha256, content, status \
         FROM message_requests WHERE request_id = ?1",
    )
    .bind(body.request_id)
    .fetch_optional(&state.infra.db)
    .await?;
    if let Some(existing) = &existing {
        if existing.content_sha256 != content_sha256 || existing.content != body.content {
            return Err(AppError::Conflict(
                "request_id was already used for different content".into(),
            ));
        }
        match MessageRequestStatus::parse(&existing.status)? {
            MessageRequestStatus::Running | MessageRequestStatus::Completed => {
                return Ok(Json(ConversationMessageResponse {
                    message_id,
                    request_status: if existing.status == "completed" {
                        "completed"
                    } else {
                        "running"
                    },
                    replayed: true,
                }));
            }
            MessageRequestStatus::Failed => {
                return Err(unsafe_replay_error());
            }
            MessageRequestStatus::Queued => {}
        }
    } else if request_id_is_tombstoned(&state.infra.db, body.request_id).await? {
        return Err(unsafe_replay_error());
    }

    let session = Arc::clone(&state.runtime.runtime_session);
    let user_message_guard = session
        .try_begin_user_message()
        .map_err(|message| AppError::Conflict(message.into()))?;
    if existing.is_none() {
        sqlx::query(
            "INSERT INTO message_requests \
             (request_id, content_sha256, content, status) \
             VALUES (?1, ?2, ?3, 'queued')",
        )
        .bind(body.request_id)
        .bind(&content_sha256)
        .bind(&body.content)
        .execute(&state.infra.db)
        .await?;
    }
    start_queued_request(&state, body.request_id, body.content, user_message_guard).await?;

    tracing::info!(
        message_id = %message_id,
        request_id = %body.request_id,
        "api.message_accepted"
    );

    Ok(Json(ConversationMessageResponse {
        message_id,
        request_status: "running",
        replayed: existing.is_some(),
    }))
}

pub(crate) async fn recover_message_requests(state: &Arc<AppState>) -> Result<(), AppError> {
    reconcile_interrupted_requests(&state.infra.db).await?;

    let queued = sqlx::query_as::<_, (Uuid, Vec<u8>, String)>(
        "SELECT request_id, content_sha256, content FROM message_requests \
         WHERE status = 'queued' ORDER BY created_at, request_id",
    )
    .fetch_all(&state.infra.db)
    .await?;
    let mut queued = queued.into_iter();
    let Some((request_id, expected_hash, content)) = queued.next() else {
        return Ok(());
    };
    for (extra_request_id, _, _) in queued {
        mark_request_failed(
            state,
            extra_request_id,
            "multiple_queued_requests_violated_single_turn_admission",
        )
        .await;
    }
    if Sha256::digest(content.as_bytes()).as_slice() != expected_hash {
        mark_request_failed(state, request_id, "content_digest_mismatch").await;
        return Ok(());
    }
    let guard = state
        .runtime
        .runtime_session
        .try_begin_user_message()
        .map_err(|message| AppError::Conflict(message.into()))?;
    start_queued_request(state, request_id, content, guard).await?;
    tracing::info!(%request_id, "api.message_recovered");
    Ok(())
}

async fn request_id_is_tombstoned(
    pool: &sqlx::SqlitePool,
    request_id: Uuid,
) -> Result<bool, AppError> {
    Ok(sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM message_request_tombstones WHERE request_id = ?1)",
    )
    .bind(request_id)
    .fetch_one(pool)
    .await?)
}

async fn reconcile_interrupted_requests(pool: &sqlx::SqlitePool) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE message_requests SET status = 'completed', failure_reason = NULL, \
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE status = 'running' AND EXISTS (\
             SELECT 1 FROM conversation_turns \
             WHERE conversation_turns.id = message_requests.request_id \
               AND conversation_turns.is_complete = 1\
         )",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "UPDATE message_requests SET status = 'failed', \
         failure_reason = 'daemon_restarted_during_execution', \
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE status = 'running'",
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn start_queued_request(
    state: &Arc<AppState>,
    request_id: Uuid,
    content: String,
    user_message_guard: UserMessageGuard,
) -> Result<(), AppError> {
    let claimed = sqlx::query(
        "UPDATE message_requests SET status = 'running', failure_reason = NULL, \
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE request_id = ?1 AND status = 'queued'",
    )
    .bind(request_id)
    .execute(&state.infra.db)
    .await?
    .rows_affected();
    if claimed != 1 {
        return Err(AppError::Conflict(
            "message request is no longer queued".into(),
        ));
    }

    let session = Arc::clone(&state.runtime.runtime_session);
    let pool = state.infra.db.clone();
    let assistant_turn_id = ConversationTurnId::from(request_id);
    let message_id = crate::models::ids::message_id_for_turn(assistant_turn_id);
    let in_flight = InFlightCounter::new(Arc::clone(&state.runtime.in_flight_text_turns));
    tokio::spawn(async move {
        let work = tokio::spawn(async move {
            let _user_message_guard = user_message_guard;
            session
                .handle_user_message(&content, &message_id, assistant_turn_id)
                .await
        });
        let (status, failure_reason) = match work.await {
            Ok(true) => ("completed", None),
            Ok(false) => ("failed", Some("turn_failed")),
            Err(error) if error.is_panic() => ("failed", Some("turn_panicked")),
            Err(_) => ("failed", Some("turn_cancelled")),
        };
        if let Err(error) = sqlx::query(
            "UPDATE message_requests SET status = ?1, failure_reason = ?2, \
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
             WHERE request_id = ?3 AND status = 'running'",
        )
        .bind(status)
        .bind(failure_reason)
        .bind(request_id)
        .execute(&pool)
        .await
        {
            tracing::error!(%error, %request_id, "api.message_status_update_failed");
        }
        drop(in_flight);
    });
    Ok(())
}

async fn mark_request_failed(state: &AppState, request_id: Uuid, reason: &str) {
    if let Err(error) = sqlx::query(
        "UPDATE message_requests SET status = 'failed', failure_reason = ?1, \
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE request_id = ?2",
    )
    .bind(reason)
    .bind(request_id)
    .execute(&state.infra.db)
    .await
    {
        tracing::error!(%error, %request_id, "api.message_status_update_failed");
    }
}

fn unsafe_replay_error() -> AppError {
    AppError::RequestReplayUnsafe(
        "This message's earlier delivery was interrupted and cannot be retried safely. \
         Check the conversation, then send it again as a new message if needed."
            .into(),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, reason = "tests panic on failed preconditions")]

    use super::*;
    use crate::db::test_helpers::in_memory_pool;

    #[tokio::test]
    async fn restart_reconciles_completed_and_ambiguous_running_requests() {
        let pool = in_memory_pool().await;
        let conversation_id = ConversationId::new();
        let completed_request = Uuid::new_v4();
        let ambiguous_request = Uuid::new_v4();
        sqlx::query("INSERT INTO conversations (id, state) VALUES (?1, 'active')")
            .bind(conversation_id)
            .execute(&pool)
            .await
            .expect("insert conversation");
        sqlx::query(
            "INSERT INTO conversation_turns \
             (id, role, content, is_complete, conversation_id) \
             VALUES (?1, 'assistant', 'done', 1, ?2)",
        )
        .bind(completed_request)
        .bind(conversation_id)
        .execute(&pool)
        .await
        .expect("insert completed turn");
        for request_id in [completed_request, ambiguous_request] {
            sqlx::query(
                "INSERT INTO message_requests \
                 (request_id, content_sha256, content, status) \
                 VALUES (?1, ?2, 'hello', 'running')",
            )
            .bind(request_id)
            .bind(vec![0_u8; 32])
            .execute(&pool)
            .await
            .expect("insert running request");
        }

        reconcile_interrupted_requests(&pool)
            .await
            .expect("reconcile requests");

        let completed_status: String =
            sqlx::query_scalar("SELECT status FROM message_requests WHERE request_id = ?1")
                .bind(completed_request)
                .fetch_one(&pool)
                .await
                .expect("load completed status");
        let ambiguous_status: String =
            sqlx::query_scalar("SELECT status FROM message_requests WHERE request_id = ?1")
                .bind(ambiguous_request)
                .fetch_one(&pool)
                .await
                .expect("load ambiguous status");
        assert_eq!(completed_status, "completed");
        assert_eq!(ambiguous_status, "failed");
    }

    #[tokio::test]
    async fn retired_request_id_remains_blocked() {
        let pool = in_memory_pool().await;
        let request_id = Uuid::new_v4();
        sqlx::query("INSERT INTO message_request_tombstones (request_id) VALUES (?1)")
            .bind(request_id)
            .execute(&pool)
            .await
            .expect("insert request tombstone");

        assert!(
            request_id_is_tombstoned(&pool, request_id)
                .await
                .expect("query request tombstone")
        );
    }
}
