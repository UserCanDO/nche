use axum::{
    extract::{Request, State},
    http::{header, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

use crate::domain::{AgentId, DashboardSessionId, DashboardUserId, TenantId};

use super::AppState;

/// Context for authenticated agent API requests
#[derive(Debug, Clone)]
pub struct AgentAuthContext {
    pub tenant_id: TenantId,
    pub agent_id: AgentId,
    pub agent_name: String,
}

/// Context for authenticated dashboard requests
#[derive(Debug, Clone)]
pub struct DashboardAuthContext {
    pub tenant_id: TenantId,
    pub user_id: DashboardUserId,
    pub session_id: DashboardSessionId,
}

#[derive(Serialize)]
struct AuthError {
    error: String,
    code: &'static str,
}

fn auth_error(status: StatusCode, message: &str, code: &'static str) -> Response {
    (status, Json(AuthError { error: message.to_string(), code })).into_response()
}

/// Middleware for agent API authentication via API key
///
/// Expects: `Authorization: Bearer nche_<agent_id>_<secret>`
pub async fn agent_auth_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    // Extract Authorization header
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    let api_key = match auth_header {
        Some(h) if h.starts_with("Bearer ") => &h[7..],
        Some(_) => {
            return auth_error(
                StatusCode::UNAUTHORIZED,
                "Invalid authorization header format. Expected: Bearer <api_key>",
                "invalid_auth_header",
            );
        }
        None => {
            return auth_error(
                StatusCode::UNAUTHORIZED,
                "Missing Authorization header",
                "missing_auth_header",
            );
        }
    };

    // Verify API key
    let (agent, tenant_id) = match state.db.get_agent_by_api_key(api_key).await {
        Ok(Some(result)) => result,
        Ok(None) => {
            return auth_error(
                StatusCode::UNAUTHORIZED,
                "Invalid API key",
                "invalid_api_key",
            );
        }
        Err(e) => {
            tracing::error!("Failed to verify API key: {}", e);
            return auth_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Authentication failed",
                "auth_error",
            );
        }
    };

    // Insert auth context into request extensions
    let auth_ctx = AgentAuthContext {
        tenant_id,
        agent_id: agent.id,
        agent_name: agent.name,
    };

    request.extensions_mut().insert(auth_ctx);

    next.run(request).await
}

/// Middleware for dashboard authentication via session cookie
///
/// Expects cookie: `nche_session=<session_id>`
pub async fn dashboard_auth_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    // Extract session from cookie
    let session_id = request
        .headers()
        .get(header::COOKIE)
        .and_then(|h| h.to_str().ok())
        .and_then(|cookies| {
            cookies
                .split(';')
                .find_map(|c| c.trim().strip_prefix("nche_session="))
        });

    let session_id = match session_id {
        Some(id) => DashboardSessionId::from_string(id.to_string()),
        None => {
            return auth_error(
                StatusCode::UNAUTHORIZED,
                "Missing session cookie",
                "missing_session",
            );
        }
    };

    // Verify session
    let session = match state.db.get_dashboard_session(&session_id).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            return auth_error(
                StatusCode::UNAUTHORIZED,
                "Invalid or expired session",
                "invalid_session",
            );
        }
        Err(e) => {
            tracing::error!("Failed to verify session: {}", e);
            return auth_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Authentication failed",
                "auth_error",
            );
        }
    };

    // Insert auth context into request extensions
    let auth_ctx = DashboardAuthContext {
        tenant_id: session.tenant_id,
        user_id: session.user_id,
        session_id: session.id,
    };

    request.extensions_mut().insert(auth_ctx);

    next.run(request).await
}

/// CSRF protection middleware for dashboard mutations
///
/// Requires `X-Session-Id` header matching the session cookie for:
/// POST, PUT, PATCH, DELETE requests
///
/// This prevents CSRF attacks by requiring a custom header that browsers
/// won't send in cross-origin requests without CORS preflight approval.
pub async fn csrf_protected_middleware(request: Request, next: Next) -> Response {
    // Only check mutations
    let needs_csrf = matches!(
        *request.method(),
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    );

    if !needs_csrf {
        return next.run(request).await;
    }

    // Extract session ID from cookie
    let cookie_session_id = request
        .headers()
        .get(header::COOKIE)
        .and_then(|h| h.to_str().ok())
        .and_then(|cookies| {
            cookies
                .split(';')
                .find_map(|c| c.trim().strip_prefix("nche_session="))
        })
        .map(|s| s.to_string());

    // Extract session ID from X-Session-Id header
    let header_session_id = request
        .headers()
        .get("X-Session-Id")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    // Validate they match
    match (cookie_session_id, header_session_id) {
        (Some(cookie_id), Some(header_id)) if cookie_id == header_id => {
            // CSRF check passed
            next.run(request).await
        }
        (Some(_), Some(_)) => {
            auth_error(
                StatusCode::FORBIDDEN,
                "X-Session-Id header does not match session cookie",
                "csrf_mismatch",
            )
        }
        (_, None) => {
            auth_error(
                StatusCode::FORBIDDEN,
                "Missing X-Session-Id header for mutation request",
                "csrf_missing_header",
            )
        }
        (None, _) => {
            // No cookie means not authenticated - let auth middleware handle it
            next.run(request).await
        }
    }
}

