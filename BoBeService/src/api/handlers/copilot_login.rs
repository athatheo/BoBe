//! `/auth/copilot/login` endpoints — in-app driver for the installed
//! `copilot login` CLI's device flow.
//!
//!   POST  /auth/copilot/login/start    — kick off (conflict if in flight)
//!   GET   /auth/copilot/login/events   — SSE phase stream (watch channel)
//!   POST  /auth/copilot/login/cancel   — kill child + emit Canceled
//!
//! The Welcome wizard's CloudAuth step and Settings → Engine present a
//! native sheet that consumes these. The CLI is GitHub's own approved
//! OAuth app — BoBe never sees the token (it's written directly to the
//! macOS Keychain by the CLI). After Completed, callers should refresh
//! `/auth/status` and reload the SDK client so subsequent spawns pick up
//! the new Keychain entry.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::{Stream, StreamExt};
use tokio_stream::wrappers::WatchStream;

use crate::app_state::AppState;
use crate::error::AppError;

pub(crate) async fn start_login(
    State(state): State<Arc<AppState>>,
) -> Result<StatusCode, AppError> {
    // Start the SDK first so CLI resolution completes before login launches.
    let _client = state
        .runtime
        .workers
        .client_handle()
        .ensure_started()
        .await
        .map_err(|e| {
            AppError::Internal(format!("copilot_login.start: client start failed: {e}"))
        })?;

    let cli_path = crate::copilot::client::installed_cli_path().ok_or_else(|| {
        AppError::Internal("copilot_login.start: Copilot CLI path unavailable".to_string())
    })?;

    state.auth.copilot_login.start(cli_path).await?;
    Ok(StatusCode::ACCEPTED)
}

pub(crate) async fn cancel_login(State(state): State<Arc<AppState>>) -> StatusCode {
    state.auth.copilot_login.cancel().await;
    StatusCode::ACCEPTED
}

pub(crate) async fn events(
    State(state): State<Arc<AppState>>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, &'static str)> {
    let Some(rx) = state.auth.copilot_login.subscribe().await else {
        return Err((StatusCode::NOT_FOUND, "no login session in flight"));
    };
    let stream = WatchStream::new(rx).map(|phase| {
        let payload = serde_json::to_string(&phase).unwrap_or_else(|_| "{}".into());
        Ok::<_, Infallible>(Event::default().event("login").data(payload))
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}
