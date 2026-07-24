use std::collections::HashSet;
use std::sync::Arc;

use axum::Json;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderValue, Method, Request, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use secrecy::{ExposeSecret, SecretString};

use crate::constants::MILLIS_PER_SECOND;
use crate::{app_state::AppState, error::AppError};

pub(crate) async fn personal_data_exclusion(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    if !is_personal_data_mutation(request.method(), request.uri().path()) {
        return Ok(next.run(request).await);
    }
    let _guard = state
        .runtime
        .runtime_session
        .try_begin_personal_data_access()
        .map_err(|message| AppError::Conflict(message.into()))?;
    Ok(next.run(request).await)
}

fn is_personal_data_mutation(method: &Method, path: &str) -> bool {
    *method != Method::GET
        && [
            "/goals",
            "/memory",
            "/souls",
            "/user-profiles",
            "/tools/mcp/config",
        ]
        .iter()
        .any(|prefix| path == *prefix || path.starts_with(&format!("{prefix}/")))
}

#[cfg(test)]
mod personal_data_tests {
    use super::*;

    #[test]
    fn gates_personal_mutations_but_not_reads_or_operational_settings() {
        assert!(is_personal_data_mutation(&Method::PUT, "/memory"));
        assert!(is_personal_data_mutation(
            &Method::DELETE,
            "/goals/00000000-0000-0000-0000-000000000000"
        ));
        assert!(is_personal_data_mutation(
            &Method::POST,
            "/tools/mcp/config/validate"
        ));
        assert!(!is_personal_data_mutation(&Method::GET, "/memory"));
        assert!(!is_personal_data_mutation(&Method::PATCH, "/settings"));
        assert!(!is_personal_data_mutation(&Method::POST, "/message"));
    }
}

#[derive(Clone)]
pub(crate) struct AllowedHosts {
    hosts: Arc<HashSet<String>>,
}

impl AllowedHosts {
    pub(crate) fn new(host: &str, port: u16, configured: &[String]) -> Self {
        let mut set = HashSet::new();
        for value in [host, "localhost", "127.0.0.1", "[::1]"]
            .into_iter()
            .chain(configured.iter().map(String::as_str))
        {
            set.insert(value.to_lowercase());
            set.insert(format!("{}:{port}", value.to_lowercase()));
        }

        Self {
            hosts: Arc::new(set),
        }
    }
}

#[derive(Clone)]
pub(crate) struct AllowedOrigins {
    origins: Arc<HashSet<HeaderValue>>,
}

impl AllowedOrigins {
    pub(crate) fn new(configured: &[String]) -> Self {
        Self {
            origins: Arc::new(
                configured
                    .iter()
                    .filter_map(|value| value.parse().ok())
                    .collect(),
            ),
        }
    }

    pub(crate) fn contains(&self, origin: &HeaderValue) -> bool {
        self.origins.contains(origin)
    }
}

#[derive(Clone)]
pub(crate) struct ApiToken(Arc<SecretString>);

impl ApiToken {
    pub(crate) fn new(token: SecretString) -> Self {
        Self(Arc::new(token))
    }
}

pub(crate) async fn bearer_auth(
    axum::extract::Extension(token): axum::extract::Extension<ApiToken>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let expected = token.0.expose_secret().as_bytes();
    if expected.is_empty() {
        return next.run(req).await;
    }
    let supplied = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::as_bytes);
    if !supplied.is_some_and(|value| constant_time_eq(value, expected)) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    next.run(req).await
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    let max_len = left.len().max(right.len());
    for index in 0..max_len {
        difference |= usize::from(
            left.get(index).copied().unwrap_or(0) ^ right.get(index).copied().unwrap_or(0),
        );
    }
    difference == 0
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_origins_accept_exact_configured_origin() {
        let allowed = AllowedOrigins::new(&["https://bobe.example.test".into()]);
        assert!(allowed.contains(&HeaderValue::from_static("https://bobe.example.test")));
        assert!(!allowed.contains(&HeaderValue::from_static("https://evil.example.test")));
    }
}
