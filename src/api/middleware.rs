use std::collections::HashSet;
use std::sync::Arc;

use axum::Json;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::constants::MILLIS_PER_SECOND;

#[derive(Clone)]
pub(crate) struct AllowedHosts {
    hosts: Arc<HashSet<String>>,
}

impl AllowedHosts {
    /// Build allowed-hosts set. `0.0.0.0` allows all interfaces; otherwise localhost-only.
    pub(crate) fn new(host: &str, port: u16) -> Self {
        let mut set = HashSet::new();
        set.insert(format!("127.0.0.1:{port}"));
        set.insert(format!("localhost:{port}"));
        set.insert("127.0.0.1".into());
        set.insert("localhost".into());

        if host == "0.0.0.0" {
            set.insert(format!("0.0.0.0:{port}"));
            set.insert("0.0.0.0".into());
        }

        Self {
            hosts: Arc::new(set),
        }
    }
}

/// Host header validation middleware to block DNS rebinding attacks.
pub(crate) async fn host_validation(
    axum::extract::Extension(allowed): axum::extract::Extension<AllowedHosts>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let host = req
        .headers()
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    if !allowed.hosts.contains(&host) {
        tracing::warn!(host = %host, "security.dns_rebinding_blocked");
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "detail": "Forbidden: invalid host" })),
        )
            .into_response();
    }

    next.run(req).await
}

/// Request logging middleware with unique request ID.
pub(crate) async fn request_logging(req: Request<Body>, next: Next) -> Response {
    let request_id = uuid::Uuid::new_v4().to_string()[..12].to_string();
    let method = req.method().clone();
    let uri = req.uri().path().to_owned();
    let start = std::time::Instant::now();

    tracing::info!(
        request_id = %request_id,
        method = %method,
        path = %uri,
        "http.request_start"
    );

    let response = next.run(req).await;

    let duration_ms = start.elapsed().as_secs_f64() * MILLIS_PER_SECOND;
    let status = response.status().as_u16();

    if status >= 500 {
        tracing::error!(
            request_id = %request_id,
            method = %method,
            path = %uri,
            status,
            duration_ms = format!("{duration_ms:.1}"),
            "http.request_error"
        );
    } else if status >= 400 {
        tracing::warn!(
            request_id = %request_id,
            method = %method,
            path = %uri,
            status,
            duration_ms = format!("{duration_ms:.1}"),
            "http.request_client_error"
        );
    } else {
        tracing::info!(
            request_id = %request_id,
            method = %method,
            path = %uri,
            status,
            duration_ms = format!("{duration_ms:.1}"),
            "http.request_complete"
        );
    }

    response
}
