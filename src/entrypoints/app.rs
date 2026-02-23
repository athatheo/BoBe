use std::sync::Arc;
use axum::{Router, routing::{get, post, put}, middleware as axum_middleware};
use tower_http::cors::{CorsLayer, AllowOrigin};
use tower_http::trace::TraceLayer;

use crate::app_state::AppState;
use super::controllers;
use super::middleware::{AllowedHosts, host_validation};

/// Build the complete Axum router with all middleware.
pub fn build_router(state: Arc<AppState>) -> Router {
    let cfg = state.config();

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

    let allowed_hosts = AllowedHosts::new(&cfg.host, cfg.port);

    Router::new()
        // Health
        .route("/health", get(controllers::health::health_check))
        .route("/api/status", get(controllers::health::get_status))
        // Conversation
        .route(
            "/api/conversation/message",
            post(controllers::conversation::send_message),
        )
        // Capture
        .route("/api/capture/start", post(controllers::capture::start_capture))
        .route("/api/capture/stop", post(controllers::capture::stop_capture))
        .route("/api/capture/once", post(controllers::capture::capture_once))
        // Goals
        .route("/api/goals", get(controllers::goals::list_goals).post(controllers::goals::create_goal))
        .route(
            "/api/goals/{goal_id}",
            get(controllers::goals::get_goal)
                .put(controllers::goals::update_goal)
                .delete(controllers::goals::delete_goal),
        )
        .route(
            "/api/goals/{goal_id}/complete",
            post(controllers::goals::complete_goal),
        )
        .route(
            "/api/goals/{goal_id}/archive",
            post(controllers::goals::archive_goal),
        )
        // Memories
        .route(
            "/api/memories",
            get(controllers::memories::list_memories).post(controllers::memories::create_memory),
        )
        .route(
            "/api/memories/search",
            post(controllers::memories::search_memories),
        )
        .route(
            "/api/memories/{memory_id}",
            get(controllers::memories::get_memory)
                .put(controllers::memories::update_memory)
                .delete(controllers::memories::delete_memory),
        )
        // Souls
        .route(
            "/api/souls",
            get(controllers::souls::list_souls).post(controllers::souls::create_soul),
        )
        .route(
            "/api/souls/{soul_id}",
            get(controllers::souls::get_soul)
                .put(controllers::souls::update_soul)
                .delete(controllers::souls::delete_soul),
        )
        // User Profiles
        .route(
            "/api/user-profiles",
            get(controllers::user_profile::list_profiles)
                .post(controllers::user_profile::create_profile),
        )
        .route(
            "/api/user-profiles/{profile_id}",
            put(controllers::user_profile::update_profile),
        )
        // MCP Configs
        .route(
            "/api/mcp-configs",
            get(controllers::mcp_configs::list_configs)
                .post(controllers::mcp_configs::create_config),
        )
        .route(
            "/api/mcp-configs/{config_id}",
            put(controllers::mcp_configs::update_config)
                .delete(controllers::mcp_configs::delete_config),
        )
        // Settings
        .route(
            "/api/settings",
            get(controllers::settings::get_settings)
                .put(controllers::settings::update_settings),
        )
        // Models
        .route("/api/models", get(controllers::models::list_models))
        // Onboarding
        .route(
            "/api/onboarding/status",
            get(controllers::onboarding::onboarding_status),
        )
        .route(
            "/api/onboarding/complete",
            post(controllers::onboarding::mark_complete),
        )
        // Tools
        .route("/api/tools", get(controllers::tools::list_tools))
        // SSE Events
        .route("/api/events", get(controllers::events::stream_events))
        // Middleware
        .layer(axum_middleware::from_fn(host_validation))
        .layer(axum::Extension(allowed_hosts))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
