use std::sync::Arc;
use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::app_state::AppState;

pub async fn health_check(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let db_ok = sqlx::query("SELECT 1")
        .fetch_one(&state.db)
        .await
        .is_ok();

    let status = if db_ok { "healthy" } else { "unhealthy" };

    Json(json!({
        "status": status,
        "services": {
            "database": if db_ok { "ok" } else { "error" },
        },
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// GET /api/status — runtime status for the UI.
pub async fn get_status(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let mut status = state.runtime_session.get_status();
    status.as_object_mut().map(|obj| {
        obj.insert("version".to_owned(), json!(env!("CARGO_PKG_VERSION")));
    });
    Json(status)
}
