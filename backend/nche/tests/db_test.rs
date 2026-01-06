//! Database integration tests.
//!
//! Tests the database layer CRUD operations against a real PostgreSQL database.
//!
//! # Prerequisites
//!
//! Set the TEST_DATABASE_URL environment variable or use the default:
//! `postgres://postgres:postgres@localhost:5432/nche_test`
//!
//! # Running
//!
//! ```bash
//! cargo test --test db_test
//! ```

mod common;

use nche::domain::*;

// ============================================================================
// Tenant Tests
// ============================================================================

#[tokio::test]
async fn test_tenant_crud() {
    let ctx = common::TestContext::new().await;

    // Create tenant (already done in TestContext)
    assert!(!ctx.tenant.id.0.is_empty());
    assert!(ctx.tenant.name.starts_with("Test Tenant"));

    // Get tenant
    let fetched = ctx.db.get_tenant(&ctx.tenant.id).await.unwrap();
    assert!(fetched.is_some());
    let fetched = fetched.unwrap();
    assert_eq!(fetched.id.0, ctx.tenant.id.0);
    assert_eq!(fetched.name, ctx.tenant.name);

    // Update tenant
    let updated = ctx
        .db
        .update_tenant(
            &ctx.tenant.id,
            Some("Updated Tenant Name"),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    assert!(updated.is_some());
    assert_eq!(updated.unwrap().name, "Updated Tenant Name");

    // List tenants
    let tenants = ctx.db.list_tenants(100).await.unwrap();
    assert!(tenants.iter().any(|t| t.id.0 == ctx.tenant.id.0));

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_tenant_internal_domains() {
    let ctx = common::TestContext::new().await;

    // Tenant was created with internal domains
    let tenant = ctx.db.get_tenant(&ctx.tenant.id).await.unwrap().unwrap();
    assert!(tenant.internal_domains.is_some());

    let domains = tenant.get_internal_domains();
    assert!(domains.contains(&"internal.test.com".to_string()));

    // Check is_internal_email
    assert!(tenant.is_internal_email("user@internal.test.com"));
    assert!(!tenant.is_internal_email("user@external.com"));

    ctx.cleanup().await;
}

// ============================================================================
// Agent Tests
// ============================================================================

#[tokio::test]
async fn test_agent_crud() {
    let ctx = common::TestContext::new().await;

    // Agent already created in TestContext
    assert!(!ctx.agent.id.0.is_empty());
    assert!(ctx.agent.name.starts_with("Test Agent"));

    // Get agent
    let fetched = ctx
        .db
        .get_agent(&ctx.tenant.id, &ctx.agent.id)
        .await
        .unwrap();
    assert!(fetched.is_some());
    let fetched = fetched.unwrap();
    assert_eq!(fetched.id.0, ctx.agent.id.0);

    // List agents
    let agents = ctx.db.list_agents(&ctx.tenant.id, 100).await.unwrap();
    assert!(!agents.is_empty());
    assert!(agents.iter().any(|a| a.id.0 == ctx.agent.id.0));

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_agent_api_key_verification() {
    let ctx = common::TestContext::new().await;

    // Verify with correct API key
    let result = ctx.db.get_agent_by_api_key(&ctx.api_key).await.unwrap();
    assert!(result.is_some());
    let (agent, tenant_id) = result.unwrap();
    assert_eq!(agent.id.0, ctx.agent.id.0);
    assert_eq!(tenant_id.0, ctx.tenant.id.0);

    // Verify with invalid API key
    let result = ctx
        .db
        .get_agent_by_api_key("nche_agt_invalid_invalidinvalidinvalid")
        .await
        .unwrap();
    assert!(result.is_none());

    // Verify with malformed API key
    let result = ctx.db.get_agent_by_api_key("not-a-valid-key").await.unwrap();
    assert!(result.is_none());

    ctx.cleanup().await;
}

// ============================================================================
// Session Tests
// ============================================================================

#[tokio::test]
async fn test_session_crud() {
    let ctx = common::TestContext::new().await;

    // Create session
    let session = ctx.create_session(AutonomyLevel::Supervised).await;
    assert!(!session.id.0.is_empty());
    assert_eq!(session.tenant_id.0, ctx.tenant.id.0);
    assert_eq!(session.agent_id.0, ctx.agent.id.0);
    assert!(session.ended_at.is_none());

    // Get session
    let fetched = ctx
        .db
        .get_session(&ctx.tenant.id, &session.id)
        .await
        .unwrap();
    assert!(fetched.is_some());
    let fetched = fetched.unwrap();
    assert_eq!(fetched.id.0, session.id.0);
    assert_eq!(fetched.autonomy_level, AutonomyLevel::Supervised);

    // List sessions (active only)
    let sessions = ctx
        .db
        .list_sessions(&ctx.tenant.id, None, true, 100)
        .await
        .unwrap();
    assert!(sessions.iter().any(|s| s.id.0 == session.id.0));

    // End session
    let ended = ctx.db.end_session(&ctx.tenant.id, &session.id).await.unwrap();
    assert!(ended);

    // Verify session is ended
    let fetched = ctx
        .db
        .get_session(&ctx.tenant.id, &session.id)
        .await
        .unwrap()
        .unwrap();
    assert!(fetched.ended_at.is_some());

    // Active-only list should not include ended session
    let active = ctx
        .db
        .list_sessions(&ctx.tenant.id, None, true, 100)
        .await
        .unwrap();
    assert!(!active.iter().any(|s| s.id.0 == session.id.0));

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_session_autonomy_levels() {
    let ctx = common::TestContext::new().await;

    // Create sessions with different autonomy levels
    let full = ctx.create_session(AutonomyLevel::Full).await;
    let supervised = ctx.create_session(AutonomyLevel::Supervised).await;
    let restricted = ctx.create_session(AutonomyLevel::Restricted).await;

    // Verify autonomy levels are stored correctly
    let full_fetched = ctx
        .db
        .get_session(&ctx.tenant.id, &full.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(full_fetched.autonomy_level, AutonomyLevel::Full);

    let supervised_fetched = ctx
        .db
        .get_session(&ctx.tenant.id, &supervised.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(supervised_fetched.autonomy_level, AutonomyLevel::Supervised);

    let restricted_fetched = ctx
        .db
        .get_session(&ctx.tenant.id, &restricted.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(restricted_fetched.autonomy_level, AutonomyLevel::Restricted);

    ctx.cleanup().await;
}

// ============================================================================
// Action Tests
// ============================================================================

#[tokio::test]
async fn test_action_crud() {
    let ctx = common::TestContext::new().await;
    let session = ctx.create_session(AutonomyLevel::Supervised).await;

    // Create action
    let action = ctx
        .create_action(
            &session,
            "send_email",
            serde_json::json!({
                "to": "test@example.com",
                "subject": "Test",
                "body": "Hello"
            }),
        )
        .await;
    assert!(!action.id.0.is_empty());
    assert_eq!(action.tool, "send_email");
    assert_eq!(action.state, ActionState::Proposed);

    // Get action
    let fetched = ctx
        .db
        .get_action(&ctx.tenant.id, &action.id)
        .await
        .unwrap();
    assert!(fetched.is_some());
    let fetched = fetched.unwrap();
    assert_eq!(fetched.id.0, action.id.0);
    assert_eq!(fetched.params["to"], "test@example.com");

    // List actions
    let actions = ctx
        .db
        .list_actions(&ctx.tenant.id, None, None, 100, 0)
        .await
        .unwrap();
    assert!(actions.iter().any(|a| a.id.0 == action.id.0));

    // Update action state
    ctx.db
        .update_action_state(&ctx.tenant.id, &action.id, ActionState::PausedForApproval)
        .await
        .unwrap();

    let updated = ctx
        .db
        .get_action(&ctx.tenant.id, &action.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.state, ActionState::PausedForApproval);

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_action_policy_update() {
    let ctx = common::TestContext::new().await;
    let session = ctx.create_session(AutonomyLevel::Supervised).await;

    let action = ctx
        .create_action(&session, "http_request", serde_json::json!({"method": "GET", "url": "https://api.test.com"}))
        .await;

    // Update policy
    ctx.db
        .update_action_policy(
            &ctx.tenant.id,
            &action.id,
            ActionState::PausedForApproval,
            PolicyResult::RequireApproval,
            "External API call",
        )
        .await
        .unwrap();

    let updated = ctx
        .db
        .get_action(&ctx.tenant.id, &action.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.policy_result, Some(PolicyResult::RequireApproval));
    assert_eq!(updated.policy_reason, Some("External API call".to_string()));

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_action_execution_complete() {
    let ctx = common::TestContext::new().await;
    let session = ctx.create_session(AutonomyLevel::Full).await;

    let action = ctx
        .create_action(&session, "http_request", serde_json::json!({"method": "GET"}))
        .await;

    // Mark as pending_execution (simulating after execution webhook dispatch)
    ctx.db
        .update_action_state(&ctx.tenant.id, &action.id, ActionState::PendingExecution)
        .await
        .unwrap();

    // Record execution result (success = true)
    let result = serde_json::json!({"status": 200, "body": "OK"});
    ctx.db
        .record_execution_result(
            &ctx.tenant.id,
            &action.id,
            true,
            Some(result.clone()),
            None,
            "test_executor",
        )
        .await
        .unwrap();

    let completed = ctx
        .db
        .get_action(&ctx.tenant.id, &action.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completed.state, ActionState::Executed);
    assert_eq!(completed.execution_result, Some(result));
    assert!(completed.error.is_none());
    assert_eq!(completed.executed_by, Some("test_executor".to_string()));

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_action_execution_failed() {
    let ctx = common::TestContext::new().await;
    let session = ctx.create_session(AutonomyLevel::Full).await;

    let action = ctx
        .create_action(&session, "http_request", serde_json::json!({"method": "POST"}))
        .await;

    ctx.db
        .update_action_state(&ctx.tenant.id, &action.id, ActionState::PendingExecution)
        .await
        .unwrap();

    // Record execution result (success = false)
    ctx.db
        .record_execution_result(
            &ctx.tenant.id,
            &action.id,
            false,
            None,
            Some("Connection timeout"),
            "test_executor",
        )
        .await
        .unwrap();

    let failed = ctx
        .db
        .get_action(&ctx.tenant.id, &action.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed.state, ActionState::Failed);
    assert_eq!(failed.error, Some("Connection timeout".to_string()));
    assert_eq!(failed.executed_by, Some("test_executor".to_string()));

    ctx.cleanup().await;
}

// ============================================================================
// Approval Tests
// ============================================================================

#[tokio::test]
async fn test_approval_crud() {
    let ctx = common::TestContext::new().await;
    let session = ctx.create_session(AutonomyLevel::Supervised).await;

    let action = ctx
        .create_action(&session, "send_email", serde_json::json!({"to": "external@test.com"}))
        .await;

    // Create approval
    let approval = ctx
        .db
        .create_approval(&ctx.tenant.id, &action.id)
        .await
        .unwrap();
    assert!(!approval.id.0.is_empty());
    assert_eq!(approval.action_id.0, action.id.0);
    assert_eq!(approval.status, ApprovalStatus::Pending);

    // Get approval
    let fetched = ctx
        .db
        .get_approval(&ctx.tenant.id, &approval.id)
        .await
        .unwrap();
    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().id.0, approval.id.0);

    // Get approval by action
    let by_action = ctx
        .db
        .get_approval_by_action(&ctx.tenant.id, &action.id)
        .await
        .unwrap();
    assert!(by_action.is_some());
    assert_eq!(by_action.unwrap().id.0, approval.id.0);

    // List pending approvals
    let pending = ctx
        .db
        .list_approvals(&ctx.tenant.id, Some(ApprovalStatus::Pending), 100, 0)
        .await
        .unwrap();
    assert!(pending.iter().any(|a| a.id.0 == approval.id.0));

    // Count pending
    let count = ctx.db.count_pending_approvals(&ctx.tenant.id).await.unwrap();
    assert!(count >= 1);

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_approval_decide_approved() {
    let ctx = common::TestContext::new().await;
    let session = ctx.create_session(AutonomyLevel::Supervised).await;

    let action = ctx
        .create_action(&session, "send_email", serde_json::json!({"to": "test@external.com"}))
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

    // Approve (approved = true)
    let decided = ctx
        .db
        .decide_approval(
            &ctx.tenant.id,
            &approval.id,
            true, // approved
            "test_approver",
            Some("Looks good"),
        )
        .await
        .unwrap();
    assert!(decided.is_some());
    let (updated_approval, new_state) = decided.unwrap();
    assert_eq!(updated_approval.status, ApprovalStatus::Approved);
    assert_eq!(updated_approval.approver_id, Some("test_approver".to_string()));
    assert_eq!(updated_approval.approver_note, Some("Looks good".to_string()));
    assert_eq!(new_state, ActionState::ReadyToExecute);

    // Verify action state changed
    let action = ctx
        .db
        .get_action(&ctx.tenant.id, &action.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(action.state, ActionState::ReadyToExecute);

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_approval_decide_denied() {
    let ctx = common::TestContext::new().await;
    let session = ctx.create_session(AutonomyLevel::Supervised).await;

    let action = ctx
        .create_action(&session, "send_email", serde_json::json!({"to": "suspicious@external.com"}))
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

    // Deny (approved = false)
    let decided = ctx
        .db
        .decide_approval(
            &ctx.tenant.id,
            &approval.id,
            false, // denied
            "security_team",
            Some("Suspicious recipient"),
        )
        .await
        .unwrap();
    assert!(decided.is_some());
    let (updated_approval, new_state) = decided.unwrap();
    assert_eq!(updated_approval.status, ApprovalStatus::Denied);
    assert_eq!(new_state, ActionState::Denied);

    // Verify action state changed
    let action = ctx
        .db
        .get_action(&ctx.tenant.id, &action.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(action.state, ActionState::Denied);

    ctx.cleanup().await;
}

// ============================================================================
// Event Tests
// ============================================================================

#[tokio::test]
async fn test_event_logging() {
    let ctx = common::TestContext::new().await;
    let session = ctx.create_session(AutonomyLevel::Supervised).await;

    let action = ctx
        .create_action(&session, "send_email", serde_json::json!({"to": "test@example.com"}))
        .await;

    // Create event
    let event = ctx
        .db
        .create_event(
            &ctx.tenant.id,
            Some(&session.id),
            Some(&action.id),
            "action_created",
            serde_json::json!({"tool": "send_email"}),
        )
        .await
        .unwrap();
    assert!(!event.id.0.is_empty());
    assert_eq!(event.event_type, "action_created");

    // List events for action
    let events = ctx
        .db
        .get_action_events(&ctx.tenant.id, &action.id)
        .await
        .unwrap();
    assert!(!events.is_empty());
    assert!(events.iter().any(|e| e.id.0 == event.id.0));

    // List events for tenant
    let all_events = ctx
        .db
        .list_events(&ctx.tenant.id, None, None, 100, 0)
        .await
        .unwrap();
    assert!(all_events.iter().any(|e| e.id.0 == event.id.0));

    ctx.cleanup().await;
}

// ============================================================================
// Task Tests (NCHE-Native Records)
// ============================================================================

#[tokio::test]
async fn test_task_crud() {
    let ctx = common::TestContext::new().await;
    let session = ctx.create_session(AutonomyLevel::Supervised).await;

    // Create task
    let task = ctx
        .db
        .create_task(
            &ctx.tenant.id,
            Some(&session.id),
            "Test Task",
            Some(TaskStatus::Open),
            Some(serde_json::json!({"priority": "high"})),
        )
        .await
        .unwrap();
    assert!(!task.id.0.is_empty());
    assert_eq!(task.title, "Test Task");
    assert_eq!(task.status, TaskStatus::Open);

    // Get task
    let fetched = ctx.db.get_task(&ctx.tenant.id, &task.id).await.unwrap();
    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().title, "Test Task");

    // List tasks
    let tasks = ctx
        .db
        .list_tasks(&ctx.tenant.id, None, None, 100, 0)
        .await
        .unwrap();
    assert!(tasks.iter().any(|t| t.id.0 == task.id.0));

    // Update task
    let updated = ctx
        .db
        .update_task(
            &ctx.tenant.id,
            &task.id,
            Some("Updated Task"),
            Some(TaskStatus::InProgress),
            None,
        )
        .await
        .unwrap();
    assert!(updated.is_some());
    let updated = updated.unwrap();
    assert_eq!(updated.title, "Updated Task");
    assert_eq!(updated.status, TaskStatus::InProgress);

    // Archive task
    let archived = ctx.db.archive_task(&ctx.tenant.id, &task.id).await.unwrap();
    assert!(archived);

    // Verify not in active list
    let active_tasks = ctx
        .db
        .list_tasks(&ctx.tenant.id, None, None, 100, 0)
        .await
        .unwrap();
    assert!(!active_tasks.iter().any(|t| t.id.0 == task.id.0));

    // Verify in archived list
    let archived_tasks = ctx
        .db
        .list_archived_tasks(&ctx.tenant.id, 100, 0)
        .await
        .unwrap();
    assert!(archived_tasks.iter().any(|t| t.id.0 == task.id.0));

    // Unarchive
    let unarchived = ctx.db.unarchive_task(&ctx.tenant.id, &task.id).await.unwrap();
    assert!(unarchived);

    ctx.cleanup().await;
}

// ============================================================================
// Case Tests (NCHE-Native Records)
// ============================================================================

#[tokio::test]
async fn test_case_crud() {
    let ctx = common::TestContext::new().await;

    // Create case
    let case = ctx
        .db
        .create_case(
            &ctx.tenant.id,
            None,
            "Security Incident",
            Some(CaseStatus::Open),
            Some(Severity::High),
            Some(serde_json::json!({"ip": "192.168.1.1"})),
            Some("EXT-123"),
        )
        .await
        .unwrap();
    assert!(!case.id.0.is_empty());
    assert_eq!(case.title, "Security Incident");
    assert_eq!(case.status, CaseStatus::Open);
    assert_eq!(case.severity, Severity::High);
    assert_eq!(case.external_ref, Some("EXT-123".to_string()));

    // Get case
    let fetched = ctx.db.get_case(&ctx.tenant.id, &case.id).await.unwrap();
    assert!(fetched.is_some());

    // List cases by severity
    let high_cases = ctx
        .db
        .list_cases(&ctx.tenant.id, None, None, Some(Severity::High), 100, 0)
        .await
        .unwrap();
    assert!(high_cases.iter().any(|c| c.id.0 == case.id.0));

    // Archive and verify
    ctx.db.archive_case(&ctx.tenant.id, &case.id).await.unwrap();
    let archived = ctx
        .db
        .list_archived_cases(&ctx.tenant.id, 100, 0)
        .await
        .unwrap();
    assert!(archived.iter().any(|c| c.id.0 == case.id.0));

    ctx.cleanup().await;
}

// ============================================================================
// Document Tests (NCHE-Native Records)
// ============================================================================

#[tokio::test]
async fn test_document_crud() {
    let ctx = common::TestContext::new().await;

    // Create document
    let doc = ctx
        .db
        .create_document(
            &ctx.tenant.id,
            None,
            "report",
            Some("quarterly_report.pdf"),
            Some("abc123hash"),
            Some("s3://bucket/quarterly_report.pdf"),
            Some(serde_json::json!(["finance", "q4"])),
        )
        .await
        .unwrap();
    assert!(!doc.id.0.is_empty());
    assert_eq!(doc.doc_type, "report");
    assert_eq!(doc.filename, Some("quarterly_report.pdf".to_string()));

    // Get document
    let fetched = ctx.db.get_document(&ctx.tenant.id, &doc.id).await.unwrap();
    assert!(fetched.is_some());

    // List documents by type
    let reports = ctx
        .db
        .list_documents(&ctx.tenant.id, None, Some("report"), 100, 0)
        .await
        .unwrap();
    assert!(reports.iter().any(|d| d.id.0 == doc.id.0));

    ctx.cleanup().await;
}

// ============================================================================
// Link Tests (NCHE-Native Records)
// ============================================================================

#[tokio::test]
async fn test_link_crud() {
    let ctx = common::TestContext::new().await;
    let session = ctx.create_session(AutonomyLevel::Supervised).await;

    let action = ctx
        .create_action(&session, "http_request", serde_json::json!({}))
        .await;

    let task = ctx
        .db
        .create_task(&ctx.tenant.id, None, "Related Task", None, None)
        .await
        .unwrap();

    // Create link
    let link = ctx
        .db
        .create_link(
            &ctx.tenant.id,
            RecordType::Action,
            &action.id.0,
            RecordType::Task,
            &task.id.0,
            "triggered",
        )
        .await
        .unwrap();
    assert!(!link.id.0.is_empty());
    assert_eq!(link.relation, "triggered");

    // Get link
    let fetched = ctx.db.get_link(&ctx.tenant.id, &link.id).await.unwrap();
    assert!(fetched.is_some());

    // List links by source
    let links = ctx
        .db
        .list_links(
            &ctx.tenant.id,
            Some(RecordType::Action),
            Some(&action.id.0),
            None,
            None,
            100,
            0,
        )
        .await
        .unwrap();
    assert!(links.iter().any(|l| l.id.0 == link.id.0));

    ctx.cleanup().await;
}
