use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use serde::Serialize;

use crate::app_state::AppState;
use crate::error::AppError;

// ── Schemas ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct OnboardingStatusResponse {
    pub complete: bool,
    pub needs_onboarding: bool,
    /// Always empty — kept for JSON shape compatibility with the Swift client.
    #[allow(clippy::zero_sized_map_values)]
    pub steps: std::collections::HashMap<String, ()>,
}

#[derive(Debug, Serialize)]
pub struct MarkCompleteResponse {
    pub ok: bool,
}

// ── Handlers ────────────────────────────────────────────────────────────────

/// GET /onboarding/status
///
/// Local-only check: DB reachable + `setup_completed` flag.
/// Never makes HTTP calls to external LLM services.
#[allow(clippy::zero_sized_map_values)]
pub async fn onboarding_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<OnboardingStatusResponse>, AppError> {
    let cfg = state.config();
    let setup_completed = cfg.setup_completed;
    drop(cfg);

    let db_ok = sqlx::query("SELECT 1").fetch_one(&state.db).await.is_ok();

    Ok(Json(OnboardingStatusResponse {
        complete: db_ok && setup_completed,
        needs_onboarding: !setup_completed,
        steps: std::collections::HashMap::new(),
    }))
}

/// POST /onboarding/mark-complete
///
/// Persists `setup_completed = true` so future launches don't re-trigger the
/// onboarding wizard when the LLM backend is temporarily unreachable.
pub async fn mark_complete(
    State(state): State<Arc<AppState>>,
) -> Result<Json<MarkCompleteResponse>, AppError> {
    let mut changes = std::collections::HashMap::new();
    changes.insert("setup_completed".to_string(), serde_json::Value::Bool(true));
    state.config_manager.update(&changes);
    tracing::info!("onboarding.marked_complete");
    Ok(Json(MarkCompleteResponse { ok: true }))
}
