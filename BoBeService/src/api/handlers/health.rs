use axum::Json;
use axum::extract::State;
use serde::Serialize;
use std::sync::Arc;

use crate::app_state::AppState;
use crate::runtime::session::RuntimeStatus;

#[derive(Debug, Serialize)]
pub(crate) struct HealthResponse {
    status: &'static str,
    services: ServiceHealth,
    version: &'static str,
}

#[derive(Debug, Serialize)]
pub(crate) struct ServiceHealth {
    database: &'static str,
}

pub(crate) async fn health_check(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    let db_ok = sqlx::query("SELECT 1")
        .fetch_one(&state.infra.db)
        .await
        .is_ok();
    let status = if db_ok { "healthy" } else { "degraded" };

    Json(HealthResponse {
        status,
        services: ServiceHealth {
            database: if db_ok { "ok" } else { "error" },
        },
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[derive(Debug, Serialize)]
pub(crate) struct StatusResponse {
    #[serde(flatten)]
    runtime: RuntimeStatus,
    version: &'static str,
}

pub(crate) async fn get_status(State(state): State<Arc<AppState>>) -> Json<StatusResponse> {
    Json(StatusResponse {
        runtime: state.runtime.runtime_session.get_status(),
        version: env!("CARGO_PKG_VERSION"),
    })
}
