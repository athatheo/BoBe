use axum::{
    Router, middleware as axum_middleware,
    routing::{delete, get, post},
};
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;

use super::handlers;
use super::middleware::{AllowedHosts, host_validation};
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

    let allowed_hosts = AllowedHosts::new(&cfg.host, cfg.port);

    Router::new()
        // Health
        .route("/health", get(handlers::health::health_check))
        .route("/api/status", get(handlers::health::get_status))
        // Conversation
        .route(
            "/api/conversation/message",
            post(handlers::conversation::send_message),
        )
        // Capture
        .route("/api/capture/start", post(handlers::capture::start_capture))
        .route("/api/capture/stop", post(handlers::capture::stop_capture))
        .route("/api/capture/once", post(handlers::capture::capture_once))
        // Goals
        .route("/api/goals", get(handlers::goals::list_goals).post(handlers::goals::create_goal))
        .route(
            "/api/goals/{goal_id}",
            get(handlers::goals::get_goal)
                .patch(handlers::goals::update_goal)
                .delete(handlers::goals::delete_goal),
        )
        .route(
            "/api/goals/{goal_id}/complete",
            post(handlers::goals::complete_goal),
        )
        .route(
            "/api/goals/{goal_id}/archive",
            post(handlers::goals::archive_goal),
        )
        // Memories
        .route(
            "/api/memories",
            get(handlers::memories::list_memories).post(handlers::memories::create_memory),
        )
        .route(
            "/api/memories/search",
            post(handlers::memories::search_memories),
        )
        .route(
            "/api/memories/{memory_id}",
            get(handlers::memories::get_memory)
                .patch(handlers::memories::update_memory)
                .delete(handlers::memories::delete_memory),
        )
        .route(
            "/api/memories/{memory_id}/enable",
            post(handlers::memories::enable_memory),
        )
        .route(
            "/api/memories/{memory_id}/disable",
            post(handlers::memories::disable_memory),
        )
        // Souls
        .route(
            "/api/souls",
            get(handlers::souls::list_souls).post(handlers::souls::create_soul),
        )
        .route(
            "/api/souls/by-name/{name}",
            get(handlers::souls::get_soul_by_name),
        )
        .route(
            "/api/souls/{soul_id}",
            get(handlers::souls::get_soul)
                .patch(handlers::souls::update_soul)
                .delete(handlers::souls::delete_soul),
        )
        .route(
            "/api/souls/{soul_id}/enable",
            post(handlers::souls::enable_soul),
        )
        .route(
            "/api/souls/{soul_id}/disable",
            post(handlers::souls::disable_soul),
        )
        // User Profiles
        .route(
            "/api/user-profiles",
            get(handlers::user_profile::list_profiles)
                .post(handlers::user_profile::create_profile),
        )
        .route(
            "/api/user-profiles/by-name/{name}",
            get(handlers::user_profile::get_profile_by_name),
        )
        .route(
            "/api/user-profiles/{profile_id}",
            get(handlers::user_profile::get_profile)
                .patch(handlers::user_profile::update_profile)
                .delete(handlers::user_profile::delete_profile),
        )
        .route(
            "/api/user-profiles/{profile_id}/enable",
            post(handlers::user_profile::enable_profile),
        )
        .route(
            "/api/user-profiles/{profile_id}/disable",
            post(handlers::user_profile::disable_profile),
        )
        // MCP Configs
        .route(
            "/api/mcp-configs",
            get(handlers::mcp_configs::list_configs)
                .post(handlers::mcp_configs::create_config),
        )
        .route(
            "/api/mcp-configs/{config_id}",
            get(handlers::mcp_configs::get_config)
                .patch(handlers::mcp_configs::update_config)
                .delete(handlers::mcp_configs::delete_config),
        )
        .route(
            "/api/mcp-configs/{config_id}/enable",
            post(handlers::mcp_configs::enable_config),
        )
        .route(
            "/api/mcp-configs/{config_id}/disable",
            post(handlers::mcp_configs::disable_config),
        )
        .route(
            "/api/mcp-configs/{config_id}/connect",
            post(handlers::mcp_configs::connect_server),
        )
        .route(
            "/api/mcp-configs/{config_id}/disconnect",
            post(handlers::mcp_configs::disconnect_server),
        )
        // Settings
        .route(
            "/api/settings",
            get(handlers::settings::get_settings)
                .patch(handlers::settings::update_settings),
        )
        // Models
        .route("/api/models", get(handlers::models::list_models))
        .route("/api/models/pull", post(handlers::models::pull_model))
        .route("/api/models/registry", get(handlers::models::list_registry_models))
        .route("/api/models/{model_name}", delete(handlers::models::delete_model))
        // Onboarding
        .route(
            "/api/onboarding/status",
            get(handlers::onboarding::onboarding_status),
        )
        .route(
            "/api/onboarding/mark-complete",
            post(handlers::onboarding::mark_complete),
        )
        .route(
            "/api/onboarding/configure-llm",
            post(handlers::onboarding::configure_llm),
        )
        .route(
            "/api/onboarding/pull-model",
            post(handlers::onboarding::pull_model),
        )
        .route(
            "/api/onboarding/warmup-embedding",
            post(handlers::onboarding::warmup_embedding),
        )
        // Tools (MCP sub-routes before wildcard)
        .route(
            "/api/tools/mcp",
            get(handlers::tools::list_mcp_servers)
                .post(handlers::tools::add_mcp_server),
        )
        .route(
            "/api/tools/mcp/{server_name}",
            delete(handlers::tools::remove_mcp_server),
        )
        .route(
            "/api/tools/mcp/{server_name}/reconnect",
            post(handlers::tools::reconnect_mcp_server),
        )
        .route("/api/tools", get(handlers::tools::list_tools))
        .route(
            "/api/tools/{tool_name}",
            axum::routing::patch(handlers::tools::update_tool),
        )
        .route(
            "/api/tools/{tool_name}/enable",
            post(handlers::tools::enable_tool),
        )
        .route(
            "/api/tools/{tool_name}/disable",
            post(handlers::tools::disable_tool),
        )
        // SSE Events
        .route("/api/events", get(handlers::events::stream_events))
        // Goal Worker
        .route(
            "/api/goal-worker/action-response",
            post(handlers::goal_worker::submit_action_response),
        )
        .route(
            "/api/goal-plans",
            get(handlers::goal_worker::list_goal_plans),
        )
        .route(
            "/api/goal-plans/pause",
            post(handlers::goal_worker::pause_goal),
        )
        .route(
            "/api/goal-plans/resume",
            post(handlers::goal_worker::resume_goal),
        )
        .route(
            "/api/goal-plans/status",
            get(handlers::goal_worker::goal_worker_status),
        )
        .route(
            "/api/goal-plans/{plan_id}",
            get(handlers::goal_worker::get_goal_plan),
        )
        .route(
            "/api/goal-plans/{plan_id}/approve",
            post(handlers::goal_worker::approve_goal_plan),
        )
        .route(
            "/api/goal-plans/{plan_id}/reject",
            post(handlers::goal_worker::reject_goal_plan),
        )
        // Middleware
        .layer(axum_middleware::from_fn(host_validation))
        .layer(axum::Extension(allowed_hosts))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
