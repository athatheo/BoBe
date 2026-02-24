use std::collections::HashSet;
use std::sync::Arc;

use axum::Json;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

/// Shared allowed-hosts set for the middleware layer.
#[derive(Clone)]
pub struct AllowedHosts {
    hosts: Arc<HashSet<String>>,
}

impl AllowedHosts {
    /// Build an allowed-hosts set for the given bind address.
    ///
    /// When host is `127.0.0.1` or `localhost`: strict localhost-only.
    /// When host is `0.0.0.0`: also allows all local network interface IPs
    /// (LAN devices).
    pub fn new(host: &str, port: u16) -> Self {
        let mut set = HashSet::new();
        // Always allow localhost variants
        set.insert(format!("127.0.0.1:{port}"));
        set.insert(format!("localhost:{port}"));
        set.insert("127.0.0.1".into());
        set.insert("localhost".into());

        if host == "0.0.0.0" {
            // Allow any host when explicitly binding to all interfaces.
            // Full LAN IP discovery will be wired later.
            set.insert(format!("0.0.0.0:{port}"));
            set.insert("0.0.0.0".into());
        }

        Self {
            hosts: Arc::new(set),
        }
    }
}

/// Axum middleware that validates the `Host` header to block DNS rebinding
/// attacks. Rejects requests whose Host header is not in the allowed set.
pub async fn host_validation(
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

/// Axum middleware that logs request lifecycle with a unique request ID.
pub async fn request_logging(req: Request<Body>, next: Next) -> Response {
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

    let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
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
