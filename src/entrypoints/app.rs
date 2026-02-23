use std::sync::Arc;
use axum::{Router, routing::get};
use tower_http::cors::{CorsLayer, AllowOrigin};
use tower_http::trace::TraceLayer;

use crate::app_state::AppState;
use super::controllers;

/// Build the complete Axum router with all middleware.
pub fn build_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::exact(
            "http://localhost:5173".parse().unwrap(),
        ))
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::PATCH,
            axum::http::Method::DELETE,
        ])
        .allow_headers([axum::http::header::CONTENT_TYPE]);

    Router::new()
        .route("/health", get(controllers::health::health_check))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
