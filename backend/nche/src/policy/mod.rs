//! Policy engine for action evaluation.
//!
//! Evaluates actions against policies based on:
//! - Tool type
//! - Autonomy level
//! - Action parameters
//! - Tenant configuration (internal domains)
//! - Global configuration (blocked domains)
//!
//! ## Semantic Tools
//!
//! The policy engine supports 20 semantic tools with built-in policies:
//!
//! | Category | Tools |
//! |----------|-------|
//! | Communication | email_send, slack_message, sms_send, notification_push |
//! | HTTP/API | http_request, graphql_execute |
//! | Calendar | calendar_event_create, calendar_event_cancel |
//! | Files | file_upload, file_delete |
//! | Database | database_query |
//! | Ticketing | ticket_create, ticket_update, ticket_reply |
//! | Financial | payment_charge, invoice_send |
//! | Documents | document_sign_request, form_submit |
//! | Code/DevOps | git_issue_create, git_pr_merge |

mod email;
mod http;
pub mod schema;
pub mod webhook;

pub use webhook::{PolicyWebhookClient, WebhookPolicyResult};

use crate::domain::{Action, AutonomyLevel, PolicyResult, Session, Tenant};

/// Context for policy evaluation containing configuration and tenant info.
#[derive(Default)]
pub struct PolicyContext<'a> {
    /// Blocked email domains from global config.
    /// Emails to these domains are always denied.
    pub blocked_email_domains: &'a [String],

    /// The tenant making the request (for internal domain checking).
    pub tenant: Option<&'a Tenant>,
}

impl<'a> PolicyContext<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_blocked_domains(mut self, domains: &'a [String]) -> Self {
        self.blocked_email_domains = domains;
        self
    }

    pub fn with_tenant(mut self, tenant: &'a Tenant) -> Self {
        self.tenant = Some(tenant);
        self
    }
}

pub struct PolicyEngine;

impl PolicyEngine {
    /// Evaluate policy without additional context (backwards compatible).
    pub fn evaluate(session: &Session, action: &Action) -> PolicyDecision {
        Self::evaluate_with_context(session, action, &PolicyContext::default())
    }

    /// Evaluate policy with context for blocked domains and tenant info.
    pub fn evaluate_with_context(
        session: &Session,
        action: &Action,
        ctx: &PolicyContext,
    ) -> PolicyDecision {
        // Dispatch to tool-specific evaluator based on tool name
        match action.tool.as_str() {
            // Communication tools
            "send_email" | "email_send" => email::evaluate(session, action, ctx),
            "slack_message" => Self::evaluate_slack_message(session, action),
            "sms_send" => Self::evaluate_sms_send(session, action),
            "notification_push" => Self::evaluate_notification_push(session, action),

            // HTTP/API tools
            "http_request" => http::evaluate(session, action),
            "graphql_execute" => Self::evaluate_graphql_execute(session, action),

            // Calendar tools
            "calendar_event_create" => Self::evaluate_calendar_event_create(session, action),
            "calendar_event_cancel" => Self::evaluate_calendar_event_cancel(session, action),

            // File tools
            "file_upload" => Self::evaluate_file_upload(session, action),
            "file_delete" => Self::evaluate_file_delete(session, action),

            // Database tools
            "database_query" => Self::evaluate_database_query(session, action),

            // Ticketing tools
            "ticket_create" => Self::evaluate_ticket_create(session, action),
            "ticket_update" => Self::evaluate_ticket_update(session, action),
            "ticket_reply" => Self::evaluate_ticket_reply(session, action),

            // Financial tools
            "payment_charge" => Self::evaluate_payment_charge(session, action),
            "invoice_send" => Self::evaluate_invoice_send(session, action),

            // Document tools
            "document_sign_request" => Self::evaluate_document_sign_request(session, action),
            "form_submit" => Self::evaluate_form_submit(session, action),

            // Code/DevOps tools
            "git_issue_create" => Self::evaluate_git_issue_create(session, action),
            "git_pr_merge" => Self::evaluate_git_pr_merge(session, action),

            // Unknown tool - safe default is to require approval
            _ => match session.autonomy_level {
                AutonomyLevel::Full => {
                    PolicyDecision::require_approval("Unknown tool requires approval even in full autonomy")
                }
                _ => PolicyDecision::require_approval("Unknown tool requires approval"),
            },
        }
    }

    // === Stub implementations for semantic tools ===
    // These will be fully implemented in Phase 2

    fn evaluate_slack_message(session: &Session, _action: &Action) -> PolicyDecision {
        match session.autonomy_level {
            AutonomyLevel::Full => PolicyDecision::allow("Full autonomy"),
            _ => PolicyDecision::require_approval("Slack message requires approval"),
        }
    }

    fn evaluate_sms_send(session: &Session, _action: &Action) -> PolicyDecision {
        match session.autonomy_level {
            AutonomyLevel::Full => PolicyDecision::allow("Full autonomy"),
            _ => PolicyDecision::require_approval("SMS requires approval"),
        }
    }

    fn evaluate_notification_push(session: &Session, _action: &Action) -> PolicyDecision {
        match session.autonomy_level {
            AutonomyLevel::Full => PolicyDecision::allow("Full autonomy"),
            AutonomyLevel::Supervised => PolicyDecision::allow("Internal notification"),
            AutonomyLevel::Restricted => PolicyDecision::require_approval("Push notification requires approval"),
        }
    }

    fn evaluate_graphql_execute(session: &Session, action: &Action) -> PolicyDecision {
        let is_mutation = action.params.get("query")
            .and_then(|q| q.as_str())
            .map(|q| q.trim().to_lowercase().starts_with("mutation"))
            .unwrap_or(false);

        match session.autonomy_level {
            AutonomyLevel::Full => PolicyDecision::allow("Full autonomy"),
            AutonomyLevel::Supervised if !is_mutation => PolicyDecision::allow("GraphQL query allowed"),
            _ => PolicyDecision::require_approval("GraphQL mutation requires approval"),
        }
    }

    fn evaluate_calendar_event_create(session: &Session, _action: &Action) -> PolicyDecision {
        match session.autonomy_level {
            AutonomyLevel::Full => PolicyDecision::allow("Full autonomy"),
            _ => PolicyDecision::require_approval("Calendar event requires approval"),
        }
    }

    fn evaluate_calendar_event_cancel(session: &Session, _action: &Action) -> PolicyDecision {
        match session.autonomy_level {
            AutonomyLevel::Full => PolicyDecision::allow("Full autonomy"),
            _ => PolicyDecision::require_approval("Calendar cancellation requires approval"),
        }
    }

    fn evaluate_file_upload(session: &Session, action: &Action) -> PolicyDecision {
        let size_bytes = action.params.get("size_bytes")
            .and_then(|s| s.as_i64())
            .unwrap_or(0);
        let is_public = action.params.get("bucket")
            .and_then(|b| b.as_str())
            .map(|b| b.contains("public"))
            .unwrap_or(false);

        match session.autonomy_level {
            AutonomyLevel::Full => PolicyDecision::allow("Full autonomy"),
            AutonomyLevel::Supervised if !is_public && size_bytes < 100_000_000 => {
                PolicyDecision::allow("Private upload under 100MB")
            }
            _ => PolicyDecision::require_approval("File upload requires approval"),
        }
    }

    fn evaluate_file_delete(session: &Session, action: &Action) -> PolicyDecision {
        let is_temp = action.params.get("path")
            .and_then(|p| p.as_str())
            .map(|p| p.contains("temp") || p.contains("tmp"))
            .unwrap_or(false);

        match session.autonomy_level {
            AutonomyLevel::Full => PolicyDecision::allow("Full autonomy"),
            AutonomyLevel::Supervised if is_temp => PolicyDecision::allow("Temp file deletion"),
            _ => PolicyDecision::require_approval("File deletion requires approval"),
        }
    }

    fn evaluate_database_query(session: &Session, action: &Action) -> PolicyDecision {
        let query = action.params.get("query")
            .and_then(|q| q.as_str())
            .unwrap_or("")
            .to_uppercase();

        let is_destructive = query.contains("DELETE") || query.contains("DROP") || query.contains("TRUNCATE");
        let is_select = query.trim().starts_with("SELECT");

        if is_destructive {
            return PolicyDecision::deny("Destructive database operations are not permitted");
        }

        match session.autonomy_level {
            AutonomyLevel::Full => PolicyDecision::allow("Full autonomy"),
            AutonomyLevel::Supervised if is_select => PolicyDecision::allow("SELECT query allowed"),
            _ => PolicyDecision::require_approval("Database write requires approval"),
        }
    }

    fn evaluate_ticket_create(session: &Session, action: &Action) -> PolicyDecision {
        // Check if ticket will be visible to customers
        let is_customer_visible = action.params.get("customer_visible")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Check if project is customer-facing (common patterns)
        let project = action.params.get("project")
            .and_then(|p| p.as_str())
            .unwrap_or("")
            .to_lowercase();
        let is_customer_project = project.contains("customer")
            || project.contains("external")
            || project.contains("support-public");

        match session.autonomy_level {
            AutonomyLevel::Full => PolicyDecision::allow("Full autonomy"),
            AutonomyLevel::Supervised if !is_customer_visible && !is_customer_project => {
                PolicyDecision::allow("Internal ticket creation")
            }
            _ => PolicyDecision::require_approval("Customer-visible ticket requires approval"),
        }
    }

    fn evaluate_ticket_update(session: &Session, action: &Action) -> PolicyDecision {
        let is_comment_only = action.params.get("comment").is_some()
            && action.params.get("status").is_none()
            && action.params.get("assignee").is_none();

        match session.autonomy_level {
            AutonomyLevel::Full => PolicyDecision::allow("Full autonomy"),
            AutonomyLevel::Supervised if is_comment_only => PolicyDecision::allow("Adding comment"),
            _ => PolicyDecision::require_approval("Ticket update requires approval"),
        }
    }

    fn evaluate_ticket_reply(session: &Session, action: &Action) -> PolicyDecision {
        let is_internal = action.params.get("internal")
            .and_then(|i| i.as_bool())
            .unwrap_or(false);

        match session.autonomy_level {
            AutonomyLevel::Full => PolicyDecision::allow("Full autonomy"),
            AutonomyLevel::Supervised if is_internal => PolicyDecision::allow("Internal note"),
            _ => PolicyDecision::require_approval("Customer-facing reply requires approval"),
        }
    }

    fn evaluate_payment_charge(session: &Session, action: &Action) -> PolicyDecision {
        let amount_cents = action.params.get("amount_cents")
            .and_then(|a| a.as_i64())
            .unwrap_or(0);

        match session.autonomy_level {
            AutonomyLevel::Full if amount_cents <= 10000 => {
                PolicyDecision::allow("Payment under $100 in full autonomy")
            }
            _ => PolicyDecision::require_approval("Payment requires approval"),
        }
    }

    fn evaluate_invoice_send(_session: &Session, _action: &Action) -> PolicyDecision {
        PolicyDecision::require_approval("Invoice sending requires approval")
    }

    fn evaluate_document_sign_request(_session: &Session, _action: &Action) -> PolicyDecision {
        PolicyDecision::require_approval("Document signing always requires approval")
    }

    fn evaluate_form_submit(session: &Session, action: &Action) -> PolicyDecision {
        let is_external = action.params.get("submit_to")
            .and_then(|s| s.as_str())
            .map(|s| s.contains("gov") || s.contains("external"))
            .unwrap_or(false);

        match session.autonomy_level {
            AutonomyLevel::Full => PolicyDecision::allow("Full autonomy"),
            AutonomyLevel::Supervised if !is_external => PolicyDecision::allow("Internal form"),
            _ => PolicyDecision::require_approval("Form submission requires approval"),
        }
    }

    fn evaluate_git_issue_create(session: &Session, action: &Action) -> PolicyDecision {
        let labels = action.params.get("labels")
            .and_then(|l| l.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
            .unwrap_or_default();

        let has_sensitive_label = labels.iter().any(|l| {
            let l = l.to_lowercase();
            l.contains("security") || l.contains("critical") || l.contains("urgent")
        });

        match session.autonomy_level {
            AutonomyLevel::Full => PolicyDecision::allow("Full autonomy"),
            AutonomyLevel::Supervised if !has_sensitive_label => PolicyDecision::allow("Standard issue"),
            _ => PolicyDecision::require_approval("Issue creation requires approval"),
        }
    }

    fn evaluate_git_pr_merge(_session: &Session, _action: &Action) -> PolicyDecision {
        PolicyDecision::require_approval("PR merge always requires approval")
    }
}

pub struct PolicyDecision {
    pub result: PolicyResult,
    pub reason: String,
}

impl PolicyDecision {
    pub fn allow(reason: impl Into<String>) -> Self {
        Self {
            result: PolicyResult::Allow,
            reason: reason.into(),
        }
    }

    pub fn deny(reason: impl Into<String>) -> Self {
        Self {
            result: PolicyResult::Deny,
            reason: reason.into(),
        }
    }

    pub fn require_approval(reason: impl Into<String>) -> Self {
        Self {
            result: PolicyResult::RequireApproval,
            reason: reason.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        Action, ActionId, ActionState, ActorType, Session, SessionId, TenantId, AgentId,
    };
    use time::OffsetDateTime;

    fn make_session(autonomy: AutonomyLevel) -> Session {
        Session {
            id: SessionId::from_string("sess_test".to_string()),
            tenant_id: TenantId::from_string("ten_test".to_string()),
            agent_id: AgentId::from_string("agt_test".to_string()),
            actor_id: "user_123".to_string(),
            actor_type: ActorType::User,
            autonomy_level: autonomy,
            created_at: OffsetDateTime::now_utc(),
            ended_at: None,
        }
    }

    fn make_action(tool: &str, params: serde_json::Value) -> Action {
        Action {
            id: ActionId::from_string("act_test".to_string()),
            tenant_id: TenantId::from_string("ten_test".to_string()),
            session_id: SessionId::from_string("sess_test".to_string()),
            tool: tool.to_string(),
            params,
            state: ActionState::Proposed,
            policy_result: None,
            policy_reason: None,
            result: None,
            error: None,
            execution_result: None,
            executed_by: None,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        }
    }

    fn make_tenant_with_internal_domains(domains: Vec<&str>) -> Tenant {
        let now = OffsetDateTime::now_utc();
        Tenant {
            id: TenantId::from_string("ten_test".to_string()),
            name: "Test Tenant".to_string(),
            webhook_url: None,
            webhook_secret: None,
            webhook_events: None,
            internal_domains: Some(serde_json::json!(domains)),
            execution_webhook_url: None,
            execution_webhook_secret: None,
            execution_webhook_timeout_ms: None,
            policy_mode: Some("builtin".to_string()),
            policy_webhook_url: None,
            policy_webhook_secret: None,
            policy_webhook_timeout_ms: None,
            created_at: now,
            updated_at: now,
        }
    }

    // === PolicyDecision Tests ===

    #[test]
    fn test_policy_decision_allow() {
        let decision = PolicyDecision::allow("test reason");
        assert_eq!(decision.result, PolicyResult::Allow);
        assert_eq!(decision.reason, "test reason");
    }

    #[test]
    fn test_policy_decision_deny() {
        let decision = PolicyDecision::deny("denied reason");
        assert_eq!(decision.result, PolicyResult::Deny);
        assert_eq!(decision.reason, "denied reason");
    }

    #[test]
    fn test_policy_decision_require_approval() {
        let decision = PolicyDecision::require_approval("needs approval");
        assert_eq!(decision.result, PolicyResult::RequireApproval);
        assert_eq!(decision.reason, "needs approval");
    }

    // === Email Policy Tests ===

    #[test]
    fn test_email_full_autonomy_allowed() {
        let session = make_session(AutonomyLevel::Full);
        let action = make_action("send_email", serde_json::json!({"to": "test@example.com"}));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::Allow);
    }

    #[test]
    fn test_email_supervised_requires_approval() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("send_email", serde_json::json!({"to": "test@example.com"}));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    // === HTTP Policy Tests ===

    #[test]
    fn test_http_full_autonomy_allowed() {
        let session = make_session(AutonomyLevel::Full);
        let action = make_action("http_request", serde_json::json!({"method": "POST", "url": "https://api.example.com"}));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::Allow);
    }

    #[test]
    fn test_http_supervised_get_allowed() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("http_request", serde_json::json!({"method": "GET", "url": "https://api.example.com"}));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::Allow);
    }

    #[test]
    fn test_http_supervised_post_requires_approval() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("http_request", serde_json::json!({"method": "POST", "url": "https://api.example.com"}));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    // === Unknown Tool Tests ===

    #[test]
    fn test_unknown_tool_requires_approval() {
        let session = make_session(AutonomyLevel::Full);
        let action = make_action("unknown_tool", serde_json::json!({}));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    // === Blocked Email Domains Tests ===

    #[test]
    fn test_email_blocked_domain_denied() {
        let session = make_session(AutonomyLevel::Full);
        let action = make_action("send_email", serde_json::json!({"to": "user@competitor.com"}));

        let blocked = vec!["competitor.com".to_string()];
        let ctx = PolicyContext::new().with_blocked_domains(&blocked);

        let decision = PolicyEngine::evaluate_with_context(&session, &action, &ctx);
        assert_eq!(decision.result, PolicyResult::Deny);
        assert!(decision.reason.contains("blocked domain"));
    }

    // === Internal Email Domain Tests ===

    #[test]
    fn test_email_internal_domain_auto_approved() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("send_email", serde_json::json!({"to": "colleague@acme.com"}));

        let tenant = make_tenant_with_internal_domains(vec!["acme.com"]);
        let ctx = PolicyContext::new().with_tenant(&tenant);

        let decision = PolicyEngine::evaluate_with_context(&session, &action, &ctx);
        assert_eq!(decision.result, PolicyResult::Allow);
        assert!(decision.reason.contains("internal domain"));
    }

    // === Semantic Tool Tests ===

    #[test]
    fn test_payment_under_100_full_autonomy() {
        let session = make_session(AutonomyLevel::Full);
        let action = make_action("payment_charge", serde_json::json!({"amount_cents": 5000}));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::Allow);
    }

    #[test]
    fn test_payment_over_100_requires_approval() {
        let session = make_session(AutonomyLevel::Full);
        let action = make_action("payment_charge", serde_json::json!({"amount_cents": 15000}));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    #[test]
    fn test_database_select_supervised() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("database_query", serde_json::json!({"query": "SELECT * FROM users"}));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::Allow);
    }

    #[test]
    fn test_database_delete_denied() {
        let session = make_session(AutonomyLevel::Full);
        let action = make_action("database_query", serde_json::json!({"query": "DELETE FROM users"}));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::Deny);
    }

    #[test]
    fn test_pr_merge_always_requires_approval() {
        let session = make_session(AutonomyLevel::Full);
        let action = make_action("git_pr_merge", serde_json::json!({"pr_number": 123}));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    #[test]
    fn test_document_sign_always_requires_approval() {
        let session = make_session(AutonomyLevel::Full);
        let action = make_action("document_sign_request", serde_json::json!({"document_id": "doc_123"}));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    // === File Tool Tests ===

    #[test]
    fn test_file_upload_private_small_supervised() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("file_upload", serde_json::json!({
            "bucket": "private-uploads",
            "path": "/docs/file.pdf",
            "size_bytes": 1024000  // ~1MB
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::Allow);
    }

    #[test]
    fn test_file_upload_public_requires_approval() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("file_upload", serde_json::json!({
            "bucket": "public-assets",
            "path": "/images/logo.png",
            "size_bytes": 50000
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    #[test]
    fn test_file_upload_large_requires_approval() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("file_upload", serde_json::json!({
            "bucket": "private-uploads",
            "path": "/videos/large.mp4",
            "size_bytes": 150000000  // 150MB
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    #[test]
    fn test_file_delete_temp_supervised() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("file_delete", serde_json::json!({
            "bucket": "uploads",
            "path": "/temp/cache-123.dat"
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::Allow);
    }

    #[test]
    fn test_file_delete_permanent_requires_approval() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("file_delete", serde_json::json!({
            "bucket": "documents",
            "path": "/important/contract.pdf"
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    // === Ticketing Tool Tests ===

    #[test]
    fn test_ticket_create_internal_supervised() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("ticket_create", serde_json::json!({
            "project": "INTERNAL",
            "type": "bug",
            "title": "Fix login",
            "description": "Something is broken"
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::Allow);
    }

    #[test]
    fn test_ticket_create_customer_visible_requires_approval() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("ticket_create", serde_json::json!({
            "project": "SUPPORT",
            "type": "bug",
            "title": "Customer issue",
            "description": "Customer can't login",
            "customer_visible": true
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    #[test]
    fn test_ticket_create_customer_project_requires_approval() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("ticket_create", serde_json::json!({
            "project": "CUSTOMER-SUPPORT",
            "type": "feature",
            "title": "New feature request"
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    #[test]
    fn test_ticket_update_comment_only_supervised() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("ticket_update", serde_json::json!({
            "ticket_id": "ISSUE-123",
            "comment": "Added more details"
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::Allow);
    }

    #[test]
    fn test_ticket_update_status_change_requires_approval() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("ticket_update", serde_json::json!({
            "ticket_id": "ISSUE-123",
            "status": "closed"
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    #[test]
    fn test_ticket_update_reassign_requires_approval() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("ticket_update", serde_json::json!({
            "ticket_id": "ISSUE-123",
            "assignee": "new-person@company.com"
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    #[test]
    fn test_ticket_reply_internal_supervised() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("ticket_reply", serde_json::json!({
            "ticket_id": "ISSUE-123",
            "body": "Internal note for team",
            "internal": true
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::Allow);
    }

    #[test]
    fn test_ticket_reply_customer_facing_requires_approval() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("ticket_reply", serde_json::json!({
            "ticket_id": "ISSUE-123",
            "body": "Hi customer, we're looking into this",
            "internal": false
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    // === Financial Tool Tests ===

    #[test]
    fn test_payment_charge_under_limit_full_autonomy() {
        let session = make_session(AutonomyLevel::Full);
        let action = make_action("payment_charge", serde_json::json!({
            "amount_cents": 9999,
            "currency": "USD",
            "customer_id": "cus_123"
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::Allow);
    }

    #[test]
    fn test_payment_charge_over_limit_requires_approval() {
        let session = make_session(AutonomyLevel::Full);
        let action = make_action("payment_charge", serde_json::json!({
            "amount_cents": 10001,
            "currency": "USD",
            "customer_id": "cus_123"
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    #[test]
    fn test_payment_charge_supervised_always_requires_approval() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("payment_charge", serde_json::json!({
            "amount_cents": 500,
            "currency": "USD",
            "customer_id": "cus_123"
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    #[test]
    fn test_invoice_send_always_requires_approval() {
        let session = make_session(AutonomyLevel::Full);
        let action = make_action("invoice_send", serde_json::json!({
            "invoice_id": "inv_123",
            "recipient_email": "customer@example.com"
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    // === Document Tool Tests ===

    #[test]
    fn test_form_submit_internal_supervised() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("form_submit", serde_json::json!({
            "form_id": "feedback-form",
            "fields": {"rating": 5},
            "submit_to": "internal-api"
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::Allow);
    }

    #[test]
    fn test_form_submit_external_requires_approval() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("form_submit", serde_json::json!({
            "form_id": "tax-form",
            "fields": {"ssn": "123-45-6789"},
            "submit_to": "gov-external-api"
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    // === Code/DevOps Tool Tests ===

    #[test]
    fn test_git_issue_create_standard_supervised() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("git_issue_create", serde_json::json!({
            "repo": "org/project",
            "title": "Add feature",
            "body": "Details here",
            "labels": ["enhancement", "documentation"]
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::Allow);
    }

    #[test]
    fn test_git_issue_create_security_label_requires_approval() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("git_issue_create", serde_json::json!({
            "repo": "org/project",
            "title": "Security vulnerability",
            "body": "Found a CVE",
            "labels": ["security", "urgent"]
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    #[test]
    fn test_git_issue_create_critical_label_requires_approval() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("git_issue_create", serde_json::json!({
            "repo": "org/project",
            "title": "Production down",
            "body": "System is down",
            "labels": ["critical"]
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    #[test]
    fn test_git_pr_merge_always_requires_approval() {
        let session = make_session(AutonomyLevel::Full);
        let action = make_action("git_pr_merge", serde_json::json!({
            "repo": "org/project",
            "pr_number": 123,
            "method": "squash"
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    // === GraphQL Tool Tests ===

    #[test]
    fn test_graphql_query_supervised() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("graphql_execute", serde_json::json!({
            "endpoint": "https://api.example.com/graphql",
            "query": "query { users { id name } }"
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::Allow);
    }

    #[test]
    fn test_graphql_mutation_requires_approval() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("graphql_execute", serde_json::json!({
            "endpoint": "https://api.example.com/graphql",
            "query": "mutation { createUser(name: \"test\") { id } }"
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    // === SMS Tool Tests ===

    #[test]
    fn test_sms_full_autonomy() {
        let session = make_session(AutonomyLevel::Full);
        let action = make_action("sms_send", serde_json::json!({
            "to": "+14155551234",
            "body": "Your code is 123456"
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::Allow);
    }

    #[test]
    fn test_sms_supervised_requires_approval() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("sms_send", serde_json::json!({
            "to": "+14155551234",
            "body": "Your code is 123456"
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    // === Slack Tool Tests ===

    #[test]
    fn test_slack_message_full_autonomy() {
        let session = make_session(AutonomyLevel::Full);
        let action = make_action("slack_message", serde_json::json!({
            "channel": "#general",
            "text": "Hello team!"
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::Allow);
    }

    #[test]
    fn test_slack_message_supervised_requires_approval() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("slack_message", serde_json::json!({
            "channel": "#general",
            "text": "Hello team!"
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    // === Push Notification Tests ===

    #[test]
    fn test_notification_push_supervised() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("notification_push", serde_json::json!({
            "user_id": "user_123",
            "title": "New message",
            "body": "You have a new message"
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::Allow);
    }

    #[test]
    fn test_notification_push_restricted_requires_approval() {
        let session = make_session(AutonomyLevel::Restricted);
        let action = make_action("notification_push", serde_json::json!({
            "user_id": "user_123",
            "title": "New message",
            "body": "You have a new message"
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    // === Calendar Tool Tests ===

    #[test]
    fn test_calendar_event_create_full_autonomy() {
        let session = make_session(AutonomyLevel::Full);
        let action = make_action("calendar_event_create", serde_json::json!({
            "title": "Team sync",
            "start": "2024-01-15T10:00:00Z",
            "end": "2024-01-15T11:00:00Z",
            "attendees": ["alice@company.com"]
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::Allow);
    }

    #[test]
    fn test_calendar_event_create_supervised_requires_approval() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("calendar_event_create", serde_json::json!({
            "title": "Team sync",
            "start": "2024-01-15T10:00:00Z",
            "end": "2024-01-15T11:00:00Z",
            "attendees": ["alice@company.com"]
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }

    #[test]
    fn test_calendar_event_cancel_requires_approval() {
        let session = make_session(AutonomyLevel::Supervised);
        let action = make_action("calendar_event_cancel", serde_json::json!({
            "event_id": "evt_123",
            "notify_attendees": true
        }));

        let decision = PolicyEngine::evaluate(&session, &action);
        assert_eq!(decision.result, PolicyResult::RequireApproval);
    }
}
