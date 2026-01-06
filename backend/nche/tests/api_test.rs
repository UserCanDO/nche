//! API integration tests.
//!
//! Tests the HTTP API endpoints against a real server instance.
//!
//! # Prerequisites
//!
//! Set the TEST_DATABASE_URL environment variable or use the default:
//! `postgres://postgres:postgres@localhost:5432/nche_test`
//!
//! # Running
//!
//! ```bash
//! cargo test --test api_test
//! ```

mod common;

use axum::http::{Method, StatusCode};
use nche::domain::*;

// ============================================================================
// Health Check Tests
// ============================================================================

#[tokio::test]
async fn test_health_check() {
    let ctx = common::TestContext::new().await;

    let (status, _body) = ctx.request(Method::GET, "/health", None).await;

    assert_eq!(status, StatusCode::OK);
    // Health check returns plain text "ok", not JSON
    ctx.cleanup().await;
}

// ============================================================================
// Authentication Tests
// ============================================================================

#[tokio::test]
async fn test_auth_missing_header() {
    let ctx = common::TestContext::new().await;

    let (status, body) = ctx.request(Method::GET, "/v1/sessions/sess_test", None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body["error"].as_str().unwrap_or("").contains("Missing")
        || body["error"].as_str().unwrap_or("").contains("authorization"));

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_auth_invalid_api_key() {
    let ctx = common::TestContext::new().await;

    // Make request with invalid API key using raw request builder
    let request = axum::http::Request::builder()
        .method(Method::GET)
        .uri("/v1/sessions/sess_test")
        .header("Authorization", "Bearer nche_agt_invalid_invalidinvalidinvalid")
        .header("Content-Type", "application/json")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = tower::ServiceExt::oneshot(ctx.router.clone(), request)
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_auth_valid_api_key() {
    let ctx = common::TestContext::new().await;

    // Create a session first so we have something to query
    let session = ctx.create_session(AutonomyLevel::Supervised).await;

    let (status, body) = ctx
        .agent_request(Method::GET, &format!("/v1/sessions/{}", session.id.0), None)
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], session.id.0);

    ctx.cleanup().await;
}

// ============================================================================
// Session API Tests
// ============================================================================

#[tokio::test]
async fn test_create_session() {
    let ctx = common::TestContext::new().await;

    let (status, body) = ctx
        .agent_request(
            Method::POST,
            "/v1/sessions",
            Some(serde_json::json!({
                "actor_id": "user_123",
                "actor_type": "user",
                "autonomy_level": "supervised"
            })),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED);
    assert!(body["id"].as_str().unwrap().starts_with("sess_"));
    assert_eq!(body["actor_id"], "user_123");
    assert_eq!(body["actor_type"], "user");
    assert_eq!(body["autonomy_level"], "supervised");

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_create_session_invalid_autonomy() {
    let ctx = common::TestContext::new().await;

    let (status, _body) = ctx
        .agent_request(
            Method::POST,
            "/v1/sessions",
            Some(serde_json::json!({
                "actor_id": "user_123",
                "actor_type": "user",
                "autonomy_level": "invalid_level"
            })),
        )
        .await;

    // Should fail with bad request or unprocessable entity
    assert!(status == StatusCode::BAD_REQUEST || status == StatusCode::UNPROCESSABLE_ENTITY);

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_get_session() {
    let ctx = common::TestContext::new().await;
    let session = ctx.create_session(AutonomyLevel::Full).await;

    let (status, body) = ctx
        .agent_request(Method::GET, &format!("/v1/sessions/{}", session.id.0), None)
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], session.id.0);
    assert_eq!(body["autonomy_level"], "full");

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_get_session_not_found() {
    let ctx = common::TestContext::new().await;

    let (status, _body) = ctx
        .agent_request(Method::GET, "/v1/sessions/sess_nonexistent", None)
        .await;

    assert_eq!(status, StatusCode::NOT_FOUND);

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_end_session() {
    let ctx = common::TestContext::new().await;
    let session = ctx.create_session(AutonomyLevel::Supervised).await;

    let (status, _body) = ctx
        .agent_request(Method::DELETE, &format!("/v1/sessions/{}", session.id.0), None)
        .await;

    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify session is ended
    let ended = ctx
        .db
        .get_session(&ctx.tenant.id, &session.id)
        .await
        .unwrap()
        .unwrap();
    assert!(ended.ended_at.is_some());

    ctx.cleanup().await;
}

// ============================================================================
// Action API Tests
// ============================================================================

#[tokio::test]
async fn test_create_action_full_autonomy() {
    let ctx = common::TestContext::new().await;
    let session = ctx.create_session(AutonomyLevel::Full).await;

    let (status, body) = ctx
        .agent_request(
            Method::POST,
            "/v1/actions",
            Some(serde_json::json!({
                "session_id": session.id.0,
                "tool": "send_email",
                "params": {
                    "to": "test@example.com",
                    "subject": "Test Email",
                    "body": "Hello World"
                }
            })),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED);
    assert!(body["id"].as_str().unwrap().starts_with("act_"));
    assert_eq!(body["tool"], "send_email");
    // Full autonomy should allow execution
    assert!(
        body["state"] == "ready_to_execute"
        || body["state"] == "executing"
        || body["state"] == "executed"
    );

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_create_action_supervised_requires_approval() {
    let ctx = common::TestContext::new().await;
    let session = ctx.create_session(AutonomyLevel::Supervised).await;

    let (status, body) = ctx
        .agent_request(
            Method::POST,
            "/v1/actions",
            Some(serde_json::json!({
                "session_id": session.id.0,
                "tool": "send_email",
                "params": {
                    "to": "external@example.com",
                    "subject": "External Email",
                    "body": "Hello"
                }
            })),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED);
    // External email in supervised mode should require approval
    assert_eq!(body["state"], "paused_for_approval");
    assert_eq!(body["policy_result"], "require_approval");

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_create_action_blocked_domain() {
    let ctx = common::TestContext::new().await;
    let session = ctx.create_session(AutonomyLevel::Full).await;

    let (status, body) = ctx
        .agent_request(
            Method::POST,
            "/v1/actions",
            Some(serde_json::json!({
                "session_id": session.id.0,
                "tool": "send_email",
                "params": {
                    "to": "user@blocked.com",
                    "subject": "Test",
                    "body": "Hello"
                }
            })),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED);
    // Should be denied due to blocked domain
    assert_eq!(body["state"], "denied");
    assert_eq!(body["policy_result"], "deny");
    assert!(body["policy_reason"].as_str().unwrap().contains("blocked"));

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_create_action_http_get_supervised() {
    let ctx = common::TestContext::new().await;
    let session = ctx.create_session(AutonomyLevel::Supervised).await;

    let (status, body) = ctx
        .agent_request(
            Method::POST,
            "/v1/actions",
            Some(serde_json::json!({
                "session_id": session.id.0,
                "tool": "http_request",
                "params": {
                    "method": "GET",
                    "url": "https://api.example.com/data"
                }
            })),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED);
    // GET requests should be auto-approved in supervised mode
    assert!(
        body["state"] == "ready_to_execute"
        || body["state"] == "executing"
        || body["state"] == "executed"
    );

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_create_action_http_post_supervised() {
    let ctx = common::TestContext::new().await;
    let session = ctx.create_session(AutonomyLevel::Supervised).await;

    let (status, body) = ctx
        .agent_request(
            Method::POST,
            "/v1/actions",
            Some(serde_json::json!({
                "session_id": session.id.0,
                "tool": "http_request",
                "params": {
                    "method": "POST",
                    "url": "https://api.example.com/submit"
                }
            })),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED);
    // POST requests should require approval in supervised mode
    assert_eq!(body["state"], "paused_for_approval");

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_get_action() {
    let ctx = common::TestContext::new().await;
    let session = ctx.create_session(AutonomyLevel::Full).await;
    let action = ctx
        .create_action(&session, "http_request", serde_json::json!({"method": "GET"}))
        .await;

    let (status, body) = ctx
        .agent_request(Method::GET, &format!("/v1/actions/{}", action.id.0), None)
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], action.id.0);
    assert_eq!(body["tool"], "http_request");

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_list_actions() {
    let ctx = common::TestContext::new().await;
    let session = ctx.create_session(AutonomyLevel::Full).await;

    // Create some actions
    ctx.create_action(&session, "http_request", serde_json::json!({"method": "GET"}))
        .await;
    ctx.create_action(&session, "send_email", serde_json::json!({"to": "test@example.com"}))
        .await;

    let (status, body) = ctx
        .agent_request(Method::GET, "/v1/actions?limit=10", None)
        .await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["data"].as_array().unwrap().len() >= 2);

    ctx.cleanup().await;
}

// ============================================================================
// Approval API Tests
// ============================================================================

#[tokio::test]
async fn test_list_approvals() {
    let ctx = common::TestContext::new().await;
    let session = ctx.create_session(AutonomyLevel::Supervised).await;

    // Create action that requires approval
    let action = ctx
        .create_action(&session, "send_email", serde_json::json!({"to": "external@test.com"}))
        .await;
    ctx.db
        .update_action_state(&ctx.tenant.id, &action.id, ActionState::PausedForApproval)
        .await
        .unwrap();
    ctx.db
        .create_approval(&ctx.tenant.id, &action.id)
        .await
        .unwrap();

    let (status, body) = ctx
        .agent_request(Method::GET, "/v1/approvals?status=pending", None)
        .await;

    assert_eq!(status, StatusCode::OK);
    assert!(!body["data"].as_array().unwrap().is_empty());

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_approve_action() {
    let ctx = common::TestContext::new().await;
    let session = ctx.create_session(AutonomyLevel::Supervised).await;

    // Create action and approval
    let action = ctx
        .create_action(&session, "send_email", serde_json::json!({"to": "external@test.com"}))
        .await;
    ctx.db
        .update_action_state(&ctx.tenant.id, &action.id, ActionState::PausedForApproval)
        .await
        .unwrap();
    let approval = ctx
        .db
        .create_approval(&ctx.tenant.id, &action.id)
        .await
        .unwrap();

    let (status, body) = ctx
        .agent_request(
            Method::PATCH,
            &format!("/v1/approvals/{}", approval.id.0),
            Some(serde_json::json!({
                "decision": "approved",
                "decided_by": "test_approver",
                "note": "Looks good"
            })),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["approval"]["status"], "approved");
    assert_eq!(body["new_state"], "ready_to_execute");

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_deny_action() {
    let ctx = common::TestContext::new().await;
    let session = ctx.create_session(AutonomyLevel::Supervised).await;

    // Create action and approval
    let action = ctx
        .create_action(&session, "send_email", serde_json::json!({"to": "suspicious@test.com"}))
        .await;
    ctx.db
        .update_action_state(&ctx.tenant.id, &action.id, ActionState::PausedForApproval)
        .await
        .unwrap();
    let approval = ctx
        .db
        .create_approval(&ctx.tenant.id, &action.id)
        .await
        .unwrap();

    let (status, body) = ctx
        .agent_request(
            Method::PATCH,
            &format!("/v1/approvals/{}", approval.id.0),
            Some(serde_json::json!({
                "decision": "denied",
                "decided_by": "security_team",
                "note": "Suspicious recipient"
            })),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["approval"]["status"], "denied");
    assert_eq!(body["new_state"], "denied");

    ctx.cleanup().await;
}

// ============================================================================
// Records API Tests (Tasks)
// ============================================================================

#[tokio::test]
async fn test_create_task() {
    let ctx = common::TestContext::new().await;

    let (status, body) = ctx
        .agent_request(
            Method::POST,
            "/v1/records/tasks",
            Some(serde_json::json!({
                "title": "Test Task",
                "status": "open",
                "notes": {"priority": "high"}
            })),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED);
    assert!(body["id"].as_str().unwrap().starts_with("task_"));
    assert_eq!(body["title"], "Test Task");
    assert_eq!(body["status"], "open");

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_list_tasks() {
    let ctx = common::TestContext::new().await;

    // Create a task
    ctx.agent_request(
        Method::POST,
        "/v1/records/tasks",
        Some(serde_json::json!({
            "title": "Task 1",
            "status": "open"
        })),
    )
    .await;

    let (status, body) = ctx
        .agent_request(Method::GET, "/v1/records/tasks", None)
        .await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["data"].as_array().unwrap().len() >= 1);

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_archive_task() {
    let ctx = common::TestContext::new().await;

    // Create a task
    let (_, created) = ctx
        .agent_request(
            Method::POST,
            "/v1/records/tasks",
            Some(serde_json::json!({
                "title": "Task to Archive",
                "status": "completed"
            })),
        )
        .await;
    let task_id = created["id"].as_str().unwrap();

    // Archive it
    let (status, _) = ctx
        .agent_request(Method::POST, &format!("/v1/records/tasks/{}/archive", task_id), None)
        .await;

    assert_eq!(status, StatusCode::OK);

    // Verify it's in archived list
    let (_, archived) = ctx
        .agent_request(Method::GET, "/v1/records/tasks/archived", None)
        .await;
    assert!(archived["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["id"] == task_id));

    // Verify it's not in active list
    let (_, active) = ctx
        .agent_request(Method::GET, "/v1/records/tasks", None)
        .await;
    assert!(!active["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["id"] == task_id));

    ctx.cleanup().await;
}

// ============================================================================
// Records API Tests (Cases)
// ============================================================================

#[tokio::test]
async fn test_create_case() {
    let ctx = common::TestContext::new().await;

    let (status, body) = ctx
        .agent_request(
            Method::POST,
            "/v1/records/cases",
            Some(serde_json::json!({
                "title": "Security Incident",
                "status": "open",
                "severity": "high",
                "external_ref": "INC-001"
            })),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED);
    assert!(body["id"].as_str().unwrap().starts_with("case_"));
    assert_eq!(body["title"], "Security Incident");
    assert_eq!(body["severity"], "high");

    ctx.cleanup().await;
}

// ============================================================================
// Records API Tests (Documents)
// ============================================================================

#[tokio::test]
async fn test_create_document() {
    let ctx = common::TestContext::new().await;

    let (status, body) = ctx
        .agent_request(
            Method::POST,
            "/v1/records/documents",
            Some(serde_json::json!({
                "doc_type": "report",
                "filename": "quarterly_report.pdf",
                "storage_uri": "s3://bucket/reports/q4.pdf"
            })),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED);
    assert!(body["id"].as_str().unwrap().starts_with("doc_"));
    assert_eq!(body["doc_type"], "report");
    assert_eq!(body["filename"], "quarterly_report.pdf");

    ctx.cleanup().await;
}

// ============================================================================
// Records API Tests (Links)
// ============================================================================

#[tokio::test]
async fn test_create_link() {
    let ctx = common::TestContext::new().await;

    // Create a task and case to link
    let (_, task) = ctx
        .agent_request(
            Method::POST,
            "/v1/records/tasks",
            Some(serde_json::json!({"title": "Linked Task"})),
        )
        .await;
    let (_, case) = ctx
        .agent_request(
            Method::POST,
            "/v1/records/cases",
            Some(serde_json::json!({"title": "Linked Case"})),
        )
        .await;

    let (status, body) = ctx
        .agent_request(
            Method::POST,
            "/v1/records/links",
            Some(serde_json::json!({
                "source_type": "task",
                "source_id": task["id"],
                "target_type": "case",
                "target_id": case["id"],
                "relation": "related_to"
            })),
        )
        .await;

    assert_eq!(status, StatusCode::CREATED);
    assert!(body["id"].as_str().unwrap().starts_with("link_"));
    assert_eq!(body["relation"], "related_to");

    ctx.cleanup().await;
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[tokio::test]
async fn test_invalid_json() {
    let ctx = common::TestContext::new().await;

    // Send malformed JSON
    let request = axum::http::Request::builder()
        .method(Method::POST)
        .uri("/v1/sessions")
        .header("Authorization", format!("Bearer {}", ctx.api_key))
        .header("Content-Type", "application/json")
        .body(axum::body::Body::from("{invalid json}"))
        .unwrap();

    let response = tower::ServiceExt::oneshot(ctx.router.clone(), request)
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_missing_required_field() {
    let ctx = common::TestContext::new().await;

    let (status, _body) = ctx
        .agent_request(
            Method::POST,
            "/v1/sessions",
            Some(serde_json::json!({
                // Missing actor_id
                "actor_type": "user",
                "autonomy_level": "supervised"
            })),
        )
        .await;

    // Should fail with bad request or unprocessable entity
    assert!(status == StatusCode::BAD_REQUEST || status == StatusCode::UNPROCESSABLE_ENTITY);

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_action_for_ended_session() {
    let ctx = common::TestContext::new().await;
    let session = ctx.create_session(AutonomyLevel::Full).await;

    // End the session
    ctx.db.end_session(&ctx.tenant.id, &session.id).await.unwrap();

    // Try to create action for ended session
    let (status, _body) = ctx
        .agent_request(
            Method::POST,
            "/v1/actions",
            Some(serde_json::json!({
                "session_id": session.id.0,
                "tool": "http_request",
                "params": {"method": "GET"}
            })),
        )
        .await;

    // Should fail - session is ended
    assert!(status == StatusCode::BAD_REQUEST || status == StatusCode::NOT_FOUND);

    ctx.cleanup().await;
}
