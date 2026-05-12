use axum::{
    Router, middleware as axum_middleware,
    routing::{get, post},
};
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::timeout::TimeoutLayer;

use super::handlers;
use super::middleware::{AllowedHosts, host_validation, request_logging};
use crate::app_state::AppState;

pub(crate) fn build_router(state: Arc<AppState>) -> Router {
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
        .route("/health", get(handlers::health::health_check))
        .route("/metrics", get(handlers::metrics::metrics))
        .route("/status", get(handlers::health::get_status))
        .route("/events", get(handlers::events::stream_events))
        .route("/message", post(handlers::conversation::send_message))
        .route("/capture/start", post(handlers::capture::start_capture))
        .route("/capture/stop", post(handlers::capture::stop_capture))
        .route("/capture/once", post(handlers::capture::capture_once))
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
        .route(
            "/memory",
            get(handlers::memories::get_memory).put(handlers::memories::update_memory),
        )
        .route(
            "/souls",
            get(handlers::souls::list_souls).post(handlers::souls::create_soul),
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
        .route(
            "/user-profiles",
            get(handlers::user_profile::list_profiles)
                .post(handlers::user_profile::create_profile),
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
        .route(
            "/settings",
            get(handlers::settings::get_settings)
                .patch(handlers::settings::update_settings),
        )
        .route("/auth/status", get(handlers::engine::get_auth_status))
        .route("/models", get(handlers::engine::list_models))
        .route(
            "/local-runtime/install",
            post(handlers::local_runtime::start_install),
        )
        .route(
            "/local-runtime/cancel",
            post(handlers::local_runtime::cancel_install),
        )
        .route(
            "/local-runtime/status",
            get(handlers::local_runtime::install_status_stream),
        )
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
        .route("/voice/stream", get(handlers::voice::voice_stream))
        .layer(axum_middleware::from_fn(request_logging))
        .layer(axum_middleware::from_fn(host_validation))
        .layer(axum::Extension(allowed_hosts))
        .layer(cors)
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::GATEWAY_TIMEOUT,
            std::time::Duration::from_secs(30),
        ))
        .layer(tower::limit::ConcurrencyLimitLayer::new(64))
        .with_state(state)
}
