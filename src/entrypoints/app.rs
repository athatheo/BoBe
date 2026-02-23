use std::sync::Arc;
use axum::{Router, routing::{delete, get, post}, middleware as axum_middleware};
use tower_http::cors::{CorsLayer, AllowOrigin};
use tower_http::trace::TraceLayer;

use crate::app_state::AppState;
use super::controllers;
use super::middleware::{AllowedHosts, host_validation};

/// Build the complete Axum router with all middleware.
pub fn build_router(state: Arc<AppState>) -> Router {
    let cfg = state.config();

    let origins: Vec<axum::http::HeaderValue> = cfg.cors_origins_vec()
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
                .patch(controllers::goals::update_goal)
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
                .patch(controllers::memories::update_memory)
                .delete(controllers::memories::delete_memory),
        )
        .route(
            "/api/memories/{memory_id}/enable",
            post(controllers::memories::enable_memory),
        )
        .route(
            "/api/memories/{memory_id}/disable",
            post(controllers::memories::disable_memory),
        )
        // Souls
        .route(
            "/api/souls",
            get(controllers::souls::list_souls).post(controllers::souls::create_soul),
        )
        .route(
            "/api/souls/by-name/{name}",
            get(controllers::souls::get_soul_by_name),
        )
        .route(
            "/api/souls/{soul_id}",
            get(controllers::souls::get_soul)
                .patch(controllers::souls::update_soul)
                .delete(controllers::souls::delete_soul),
        )
        .route(
            "/api/souls/{soul_id}/enable",
            post(controllers::souls::enable_soul),
        )
        .route(
            "/api/souls/{soul_id}/disable",
            post(controllers::souls::disable_soul),
        )
        // User Profiles
        .route(
            "/api/user-profiles",
            get(controllers::user_profile::list_profiles)
                .post(controllers::user_profile::create_profile),
        )
        .route(
            "/api/user-profiles/by-name/{name}",
            get(controllers::user_profile::get_profile_by_name),
        )
        .route(
            "/api/user-profiles/{profile_id}",
            get(controllers::user_profile::get_profile)
                .patch(controllers::user_profile::update_profile)
                .delete(controllers::user_profile::delete_profile),
        )
        .route(
            "/api/user-profiles/{profile_id}/enable",
            post(controllers::user_profile::enable_profile),
        )
        .route(
            "/api/user-profiles/{profile_id}/disable",
            post(controllers::user_profile::disable_profile),
        )
        // MCP Configs
        .route(
            "/api/mcp-configs",
            get(controllers::mcp_configs::list_configs)
                .post(controllers::mcp_configs::create_config),
        )
        .route(
            "/api/mcp-configs/{config_id}",
            get(controllers::mcp_configs::get_config)
                .patch(controllers::mcp_configs::update_config)
                .delete(controllers::mcp_configs::delete_config),
        )
        .route(
            "/api/mcp-configs/{config_id}/enable",
            post(controllers::mcp_configs::enable_config),
        )
        .route(
            "/api/mcp-configs/{config_id}/disable",
            post(controllers::mcp_configs::disable_config),
        )
        .route(
            "/api/mcp-configs/{config_id}/connect",
            post(controllers::mcp_configs::connect_server),
        )
        .route(
            "/api/mcp-configs/{config_id}/disconnect",
            post(controllers::mcp_configs::disconnect_server),
        )
        // Settings
        .route(
            "/api/settings",
            get(controllers::settings::get_settings)
                .patch(controllers::settings::update_settings),
        )
        // Models
        .route("/api/models", get(controllers::models::list_models))
        .route("/api/models/pull", post(controllers::models::pull_model))
        .route("/api/models/registry", get(controllers::models::list_registry_models))
        .route("/api/models/{model_name}", delete(controllers::models::delete_model))
        // Onboarding
        .route(
            "/api/onboarding/status",
            get(controllers::onboarding::onboarding_status),
        )
        .route(
            "/api/onboarding/complete",
            post(controllers::onboarding::mark_complete),
        )
        .route(
            "/api/onboarding/configure-llm",
            post(controllers::onboarding::configure_llm),
        )
        .route(
            "/api/onboarding/pull-model",
            post(controllers::onboarding::pull_model),
        )
        .route(
            "/api/onboarding/warmup-embedding",
            post(controllers::onboarding::warmup_embedding),
        )
        // Tools (MCP sub-routes before wildcard)
        .route(
            "/api/tools/mcp",
            get(controllers::tools::list_mcp_servers)
                .post(controllers::tools::add_mcp_server),
        )
        .route(
            "/api/tools/mcp/{server_name}",
            delete(controllers::tools::remove_mcp_server),
        )
        .route(
            "/api/tools/mcp/{server_name}/reconnect",
            post(controllers::tools::reconnect_mcp_server),
        )
        .route("/api/tools", get(controllers::tools::list_tools))
        .route(
            "/api/tools/{tool_name}",
            axum::routing::patch(controllers::tools::update_tool),
        )
        .route(
            "/api/tools/{tool_name}/enable",
            post(controllers::tools::enable_tool),
        )
        .route(
            "/api/tools/{tool_name}/disable",
            post(controllers::tools::disable_tool),
        )
        // SSE Events
        .route("/api/events", get(controllers::events::stream_events))
        // Middleware
        .layer(axum_middleware::from_fn(host_validation))
        .layer(axum::Extension(allowed_hosts))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
