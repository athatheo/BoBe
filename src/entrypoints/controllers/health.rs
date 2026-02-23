use axum::Json;
use axum::extract::State;
use serde::Serialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::app_state::AppState;

// ── Schemas ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    status: &'static str,
    services: ServiceHealth,
    version: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ServiceHealth {
    database: &'static str,
    llm: &'static str,
}

pub async fn health_check(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    let db_ok = sqlx::query("SELECT 1").fetch_one(&state.db).await.is_ok();

    let llm_ok = state.llm_provider.health_check().await;

    let all_ok = db_ok && llm_ok;
    let status = if all_ok { "healthy" } else { "degraded" };

    Json(HealthResponse {
        status,
        services: ServiceHealth {
            database: if db_ok { "ok" } else { "error" },
            llm: if llm_ok { "ok" } else { "error" },
        },
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// GET /api/status — runtime status for the UI.
pub async fn get_status(State(state): State<Arc<AppState>>) -> Json<Value> {
    let mut status = state.runtime_session.get_status();
    if let Some(obj) = status.as_object_mut() {
        obj.insert("version".to_owned(), json!(env!("CARGO_PKG_VERSION")));
    }
    Json(status)
}
