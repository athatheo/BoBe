use axum::{
    Router, middleware as axum_middleware,
    routing::{delete, get, post},
};
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::timeout::TimeoutLayer;

use super::handlers;
use super::middleware::{AllowedHosts, host_validation, request_logging};
use crate::app_state::AppState;

/// Build the complete Axum router with all middleware.
pub fn build_router(state: Arc<AppState>) -> Router {
    let cfg = state.config();

    let origins: Vec<axum::http::HeaderValue> = cfg
        .cors_origins_vec()
        .iter()
        .filter_map(|o| o.parse().ok())
        .collect();

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::PATCH,
            axum::http::Method::DELETE,
        ])
        .allow_headers([axum::http::header::CONTENT_TYPE])
        .allow_credentials(true);

    let allowed_hosts = AllowedHosts::new(&cfg.server.host, cfg.server.port);

    Router::new()
        // Health & Status
        .route("/health", get(handlers::health::health_check))
        .route("/status", get(handlers::health::get_status))
        // SSE Events
        .route("/events", get(handlers::events::stream_events))
        // Conversation
        .route("/message", post(handlers::conversation::send_message))
        .route("/message/dismiss", post(handlers::conversation::dismiss_message))
        // Capture
        .route("/capture/start", post(handlers::capture::start_capture))
        .route("/capture/stop", post(handlers::capture::stop_capture))
        .route("/capture/once", post(handlers::capture::capture_once))
        // Goals
        .route("/goals", get(handlers::goals::list_goals).post(handlers::goals::create_goal))
        .route(
            "/goals/{goal_id}",
            get(handlers::goals::get_goal)
                .patch(handlers::goals::update_goal)
                .delete(handlers::goals::delete_goal),
        )
        .route(
            "/goals/{goal_id}/complete",
            post(handlers::goals::complete_goal),
        )
        .route(
            "/goals/{goal_id}/archive",
            post(handlers::goals::archive_goal),
        )
        // Memories
        .route(
            "/memories",
            get(handlers::memories::list_memories).post(handlers::memories::create_memory),
        )
        .route(
            "/memories/search",
            post(handlers::memories::search_memories),
        )
        .route(
            "/memories/{memory_id}",
            get(handlers::memories::get_memory)
                .patch(handlers::memories::update_memory)
                .delete(handlers::memories::delete_memory),
        )
        .route(
            "/memories/{memory_id}/enable",
            post(handlers::memories::enable_memory),
        )
        .route(
            "/memories/{memory_id}/disable",
            post(handlers::memories::disable_memory),
        )
        // Souls
        .route(
            "/souls",
            get(handlers::souls::list_souls).post(handlers::souls::create_soul),
        )
        .route(
            "/souls/by-name/{name}",
            get(handlers::souls::get_soul_by_name),
        )
        .route(
            "/souls/{soul_id}",
            get(handlers::souls::get_soul)
                .patch(handlers::souls::update_soul)
                .delete(handlers::souls::delete_soul),
        )
        .route(
            "/souls/{soul_id}/enable",
            post(handlers::souls::enable_soul),
        )
        .route(
            "/souls/{soul_id}/disable",
            post(handlers::souls::disable_soul),
        )
        // User Profiles
        .route(
            "/user-profiles",
            get(handlers::user_profile::list_profiles)
                .post(handlers::user_profile::create_profile),
        )
        .route(
            "/user-profiles/by-name/{name}",
            get(handlers::user_profile::get_profile_by_name),
        )
        .route(
            "/user-profiles/{profile_id}",
            get(handlers::user_profile::get_profile)
                .patch(handlers::user_profile::update_profile)
                .delete(handlers::user_profile::delete_profile),
        )
        .route(
            "/user-profiles/{profile_id}/enable",
            post(handlers::user_profile::enable_profile),
        )
        .route(
            "/user-profiles/{profile_id}/disable",
            post(handlers::user_profile::disable_profile),
        )
        // Settings
        .route(
            "/settings",
            get(handlers::settings::get_settings)
                .patch(handlers::settings::update_settings),
        )
        // Models
        .route("/models", get(handlers::models::list_models))
        .route("/models/pull", post(handlers::models::pull_model))
        .route("/models/registry", get(handlers::models::list_registry_models))
        .route("/models/{model_name}", delete(handlers::models::delete_model))
        // Onboarding & Setup
        .route(
            "/onboarding/status",
            get(handlers::onboarding::onboarding_status),
        )
        .route(
            "/onboarding/mark-complete",
            post(handlers::onboarding::mark_complete),
        )
        .route(
            "/onboarding/options",
            get(handlers::setup::get_options),
        )
        .route(
            "/onboarding/setup",
            post(handlers::setup::create_setup_job),
        )
        .route(
            "/onboarding/setup/{job_id}",
            get(handlers::setup::get_setup_status)
                .delete(handlers::setup::cancel_setup_job),
        )
        // Tools (MCP full-document config routes)
        .route(
            "/tools/mcp/config",
            get(handlers::tools_mcp::get_mcp_config)
                .put(handlers::tools_mcp::save_mcp_config)
                .delete(handlers::tools_mcp::reset_mcp_config),
        )
        .route(
            "/tools/mcp/config/validate",
            post(handlers::tools_mcp::validate_mcp_config),
        )
        .route("/tools", get(handlers::tools::list_tools))
        .route(
            "/tools/{tool_name}",
            axum::routing::patch(handlers::tools::update_tool),
        )
        .route(
            "/tools/{tool_name}/enable",
            post(handlers::tools::enable_tool),
        )
        .route(
            "/tools/{tool_name}/disable",
            post(handlers::tools::disable_tool),
        )
        // Goal Worker
        .route(
            "/goal-plans",
            get(handlers::goal_worker::list_goal_plans),
        )
        .route(
            "/goal-plans/pause",
            post(handlers::goal_worker::pause_goal),
        )
        .route(
            "/goal-plans/resume",
            post(handlers::goal_worker::resume_goal),
        )
        .route(
            "/goal-plans/status",
            get(handlers::goal_worker::goal_worker_status),
        )
        .route(
            "/goal-plans/{plan_id}",
            get(handlers::goal_worker::get_goal_plan),
        )
        .route(
            "/goal-plans/{plan_id}/approve",
            post(handlers::goal_worker::approve_goal_plan),
        )
        .route(
            "/goal-plans/{plan_id}/reject",
            post(handlers::goal_worker::reject_goal_plan),
        )
        // Middleware (order: outermost applied first, innermost runs first)
        .layer(axum_middleware::from_fn(request_logging))
        .layer(axum_middleware::from_fn(host_validation))
        .layer(axum::Extension(allowed_hosts))
        .layer(cors)
        // Global request timeout (30s). SSE is unaffected — its handler returns
        // the Sse response immediately; the background stream runs independently.
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::GATEWAY_TIMEOUT,
            std::time::Duration::from_secs(30),
        ))
        // Cap concurrent in-flight requests to prevent resource exhaustion.
        .layer(tower::limit::ConcurrencyLimitLayer::new(64))
        .with_state(state)
}
