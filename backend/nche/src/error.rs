use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

use crate::domain::ActionState;

#[derive(Debug, thiserror::Error)]
pub enum NcheError {
    #[error("Not found: {entity} with id {id}")]
    NotFound { entity: &'static str, id: String },

    #[error("Invalid state transition from {from:?}: {action}")]
    InvalidStateTransition { from: ActionState, action: String },

    #[error("Unauthorized: {message}")]
    Unauthorized { message: String },

    #[error("Forbidden: {message}")]
    Forbidden { message: String },

    #[error("Bad request: {message}")]
    BadRequest { message: String },

    #[error("Conflict: {message}")]
    Conflict { message: String },

    #[error("Tool execution failed: {message}")]
    ToolExecution { message: String },

    #[error("Webhook delivery failed: {message}")]
    WebhookDelivery { message: String },

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, NcheError>;

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    code: &'static str,
}

impl IntoResponse for NcheError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            Self::NotFound { .. } => (StatusCode::NOT_FOUND, "not_found"),
            Self::InvalidStateTransition { .. } => (StatusCode::CONFLICT, "invalid_state"),
            Self::Unauthorized { .. } => (StatusCode::UNAUTHORIZED, "unauthorized"),
            Self::Forbidden { .. } => (StatusCode::FORBIDDEN, "forbidden"),
            Self::BadRequest { .. } => (StatusCode::BAD_REQUEST, "bad_request"),
            Self::Conflict { .. } => (StatusCode::CONFLICT, "conflict"),
            Self::ToolExecution { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "tool_error"),
            Self::WebhookDelivery { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "webhook_error"),
            Self::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "database_error"),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
        };

        let body = Json(ErrorResponse {
            error: self.to_string(),
            code,
        });

        (status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Error Message Tests ===

    #[test]
    fn test_not_found_error_message() {
        let err = NcheError::NotFound {
            entity: "Action",
            id: "act_123".to_string(),
        };
        assert_eq!(err.to_string(), "Not found: Action with id act_123");
    }

    #[test]
    fn test_invalid_state_transition_message() {
        let err = NcheError::InvalidStateTransition {
            from: ActionState::Proposed,
            action: "execute".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Invalid state transition from Proposed: execute"
        );
    }

    #[test]
    fn test_unauthorized_error_message() {
        let err = NcheError::Unauthorized {
            message: "Invalid API key".to_string(),
        };
        assert_eq!(err.to_string(), "Unauthorized: Invalid API key");
    }

    #[test]
    fn test_forbidden_error_message() {
        let err = NcheError::Forbidden {
            message: "Access denied to resource".to_string(),
        };
        assert_eq!(err.to_string(), "Forbidden: Access denied to resource");
    }

    #[test]
    fn test_bad_request_error_message() {
        let err = NcheError::BadRequest {
            message: "Missing required field".to_string(),
        };
        assert_eq!(err.to_string(), "Bad request: Missing required field");
    }

    #[test]
    fn test_conflict_error_message() {
        let err = NcheError::Conflict {
            message: "Resource already exists".to_string(),
        };
        assert_eq!(err.to_string(), "Conflict: Resource already exists");
    }

    #[test]
    fn test_tool_execution_error_message() {
        let err = NcheError::ToolExecution {
            message: "Tool timed out".to_string(),
        };
        assert_eq!(err.to_string(), "Tool execution failed: Tool timed out");
    }

    #[test]
    fn test_webhook_delivery_error_message() {
        let err = NcheError::WebhookDelivery {
            message: "Connection refused".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Webhook delivery failed: Connection refused"
        );
    }

    #[test]
    fn test_internal_error_message() {
        let err = NcheError::Internal("Unexpected state".to_string());
        assert_eq!(err.to_string(), "Internal error: Unexpected state");
    }

    // === HTTP Status Code Tests ===

    #[tokio::test]
    async fn test_not_found_status_code() {
        let err = NcheError::NotFound {
            entity: "Session",
            id: "sess_123".to_string(),
        };
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_invalid_state_transition_status_code() {
        let err = NcheError::InvalidStateTransition {
            from: ActionState::Executed,
            action: "approve".to_string(),
        };
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn test_unauthorized_status_code() {
        let err = NcheError::Unauthorized {
            message: "Token expired".to_string(),
        };
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_forbidden_status_code() {
        let err = NcheError::Forbidden {
            message: "Not allowed".to_string(),
        };
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_bad_request_status_code() {
        let err = NcheError::BadRequest {
            message: "Invalid input".to_string(),
        };
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_conflict_status_code() {
        let err = NcheError::Conflict {
            message: "Duplicate entry".to_string(),
        };
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn test_tool_execution_status_code() {
        let err = NcheError::ToolExecution {
            message: "Failed".to_string(),
        };
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn test_webhook_delivery_status_code() {
        let err = NcheError::WebhookDelivery {
            message: "Timeout".to_string(),
        };
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn test_internal_error_status_code() {
        let err = NcheError::Internal("Panic".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    // === Error Debug Trait Test ===

    #[test]
    fn test_error_debug_format() {
        let err = NcheError::NotFound {
            entity: "Agent",
            id: "agt_xyz".to_string(),
        };
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("NotFound"));
        assert!(debug_str.contains("Agent"));
        assert!(debug_str.contains("agt_xyz"));
    }
}
