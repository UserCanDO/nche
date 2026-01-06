pub mod auth;
mod records;

use axum::{
    middleware,
    routing::{delete, get, patch, post},
    Router,
};
use std::sync::Arc;

use crate::dashboard::dashboard_routes;
use crate::db::Database;

pub use auth::{
    agent_auth_middleware, csrf_protected_middleware, dashboard_auth_middleware,
    AgentAuthContext, DashboardAuthContext,
};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub blocked_email_domains: Vec<String>,
}

/// Build the complete router
pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Health check (no auth)
        .route("/health", get(health_check))
        // Agent API routes (API key auth)
        .nest("/v1", agent_api_routes(state.clone()))
        // Dashboard API routes (session auth)
        .nest("/dashboard/api", dashboard_api_routes(state.clone()))
        // Dashboard auth routes (no auth required)
        .nest("/dashboard", dashboard_auth_routes())
        // Static dashboard UI (fallback for unmatched routes)
        .fallback_service(dashboard_routes())
        .with_state(state)
}

/// Agent API routes - requires API key authentication
fn agent_api_routes(state: AppState) -> Router<AppState> {
    Router::new()
        // Sessions
        .route("/sessions", post(handlers::create_session))
        .route("/sessions/{id}", get(handlers::get_session))
        .route("/sessions/{id}", delete(handlers::end_session))
        // Actions
        .route("/actions", post(handlers::create_action))
        .route("/actions", get(handlers::list_actions))
        .route("/actions/{id}", get(handlers::get_action))
        .route("/actions/{id}/result", post(handlers::report_action_result))
        // Approvals (agents can view their approvals)
        .route("/approvals", get(handlers::list_approvals))
        .route("/approvals/{id}", patch(handlers::update_approval))
        // Tenant configuration
        .route("/tenant/config", get(handlers::get_tenant_config).patch(handlers::update_tenant_config))
        // NCHE-Native Records: Tasks
        .route("/records/tasks", post(records::create_task))
        .route("/records/tasks", get(records::list_tasks))
        .route("/records/tasks/archived", get(records::list_archived_tasks))
        .route("/records/tasks/{id}", get(records::get_task))
        .route("/records/tasks/{id}/archive", post(records::archive_task))
        .route("/records/tasks/{id}/unarchive", post(records::unarchive_task))
        // NCHE-Native Records: Cases
        .route("/records/cases", post(records::create_case))
        .route("/records/cases", get(records::list_cases))
        .route("/records/cases/archived", get(records::list_archived_cases))
        .route("/records/cases/{id}", get(records::get_case))
        .route("/records/cases/{id}/archive", post(records::archive_case))
        .route("/records/cases/{id}/unarchive", post(records::unarchive_case))
        // NCHE-Native Records: Documents
        .route("/records/documents", post(records::create_document))
        .route("/records/documents", get(records::list_documents))
        .route("/records/documents/archived", get(records::list_archived_documents))
        .route("/records/documents/{id}", get(records::get_document))
        .route("/records/documents/{id}/archive", post(records::archive_document))
        .route("/records/documents/{id}/unarchive", post(records::unarchive_document))
        // NCHE-Native Records: Links
        .route("/records/links", post(records::create_link))
        .route("/records/links", get(records::list_links))
        .route("/records/links/archived", get(records::list_archived_links))
        .route("/records/links/{id}", get(records::get_link))
        .route("/records/links/{id}/archive", post(records::archive_link))
        .route("/records/links/{id}/unarchive", post(records::unarchive_link))
        .layer(middleware::from_fn_with_state(
            state,
            agent_auth_middleware,
        ))
}

/// Dashboard API routes - requires session authentication
fn dashboard_api_routes(state: AppState) -> Router<AppState> {
    Router::new()
        // Approvals
        .route("/approvals", get(handlers::list_pending_approvals))
        .route("/approvals/{id}", get(handlers::get_approval_detail))
        .route("/approvals/{id}", patch(handlers::dashboard_update_approval))
        // Agents management
        .route("/agents", get(handlers::dashboard_list_agents))
        .route("/agents", post(handlers::dashboard_create_agent))
        // Sessions management
        .route("/sessions", get(handlers::dashboard_list_sessions))
        // Actions management
        .route("/actions", get(handlers::dashboard_list_actions))
        .route("/actions/{id}", get(handlers::dashboard_get_action))
        // Dashboard stats
        .route("/stats", get(handlers::dashboard_stats))
        // Audit log
        .route("/events", get(handlers::list_events))
        // Session info
        .route("/me", get(handlers::get_current_user))
        // Logout (requires auth to know which session to delete)
        .route("/logout", post(handlers::dashboard_logout))
        // Tenant configuration
        .route("/tenant/config", get(handlers::dashboard_get_tenant_config).patch(handlers::dashboard_update_tenant_config))
        // CSRF protection for mutations (must be after auth to have session context)
        .layer(middleware::from_fn(csrf_protected_middleware))
        .layer(middleware::from_fn_with_state(
            state,
            dashboard_auth_middleware,
        ))
}

/// Dashboard auth routes - no authentication required
fn dashboard_auth_routes() -> Router<AppState> {
    Router::new()
        .route("/login", post(handlers::dashboard_login))
}

async fn health_check() -> &'static str {
    "ok"
}

mod handlers {
    use axum::{
        extract::{Path, Query, State},
        http::StatusCode,
        response::IntoResponse,
        Extension, Json,
    };
    use serde::{Deserialize, Serialize};

    use super::{AgentAuthContext, AppState, DashboardAuthContext};
    use crate::domain::*;
    use crate::error::Result;

    // === Session Handlers ===

    #[derive(Deserialize)]
    pub struct CreateSessionRequest {
        pub actor_id: String,
        pub actor_type: ActorType,
        pub autonomy_level: AutonomyLevel,
    }

    pub async fn create_session(
        State(state): State<AppState>,
        Extension(auth): Extension<AgentAuthContext>,
        Json(req): Json<CreateSessionRequest>,
    ) -> Result<impl IntoResponse> {
        let session = state
            .db
            .create_session(
                &auth.tenant_id,
                &auth.agent_id,
                &req.actor_id,
                req.actor_type,
                req.autonomy_level,
            )
            .await?;

        Ok((StatusCode::CREATED, Json(session)))
    }

    pub async fn get_session(
        State(state): State<AppState>,
        Extension(auth): Extension<AgentAuthContext>,
        Path(id): Path<String>,
    ) -> Result<impl IntoResponse> {
        let session_id = SessionId::from_string(id);
        let session = state
            .db
            .get_session(&auth.tenant_id, &session_id)
            .await?
            .ok_or_else(|| crate::error::NcheError::NotFound {
                entity: "session",
                id: session_id.to_string(),
            })?;

        Ok(Json(session))
    }

    pub async fn end_session(
        State(state): State<AppState>,
        Extension(auth): Extension<AgentAuthContext>,
        Path(id): Path<String>,
    ) -> Result<impl IntoResponse> {
        let session_id = SessionId::from_string(id);

        // Verify session exists and belongs to this agent
        let session = state
            .db
            .get_session(&auth.tenant_id, &session_id)
            .await?
            .ok_or_else(|| crate::error::NcheError::NotFound {
                entity: "session",
                id: session_id.to_string(),
            })?;

        if session.agent_id != auth.agent_id {
            return Err(crate::error::NcheError::Forbidden {
                message: "Session belongs to a different agent".to_string(),
            });
        }

        if session.ended_at.is_some() {
            return Err(crate::error::NcheError::BadRequest {
                message: "Session is already ended".to_string(),
            });
        }

        state.db.end_session(&auth.tenant_id, &session_id).await?;

        // Log event
        state
            .db
            .create_event(
                &auth.tenant_id,
                Some(&session_id),
                None,
                "session.ended",
                serde_json::json!({
                    "agent_id": auth.agent_id.to_string(),
                }),
            )
            .await?;

        // Fetch updated session
        let session = state
            .db
            .get_session(&auth.tenant_id, &session_id)
            .await?
            .unwrap();

        Ok(Json(session))
    }

    // === Action Handlers ===

    #[derive(Deserialize)]
    pub struct CreateActionRequest {
        pub session_id: String,
        pub tool: String,
        pub params: serde_json::Value,
    }

    #[derive(Serialize)]
    pub struct ActionResponse {
        #[serde(flatten)]
        pub action: Action,
        pub approval: Option<Approval>,
    }

    pub async fn create_action(
        State(state): State<AppState>,
        Extension(auth): Extension<AgentAuthContext>,
        Json(req): Json<CreateActionRequest>,
    ) -> Result<impl IntoResponse> {
        let session_id = SessionId::from_string(req.session_id);

        // Verify session exists and belongs to this agent
        let session = state
            .db
            .get_session(&auth.tenant_id, &session_id)
            .await?
            .ok_or_else(|| crate::error::NcheError::NotFound {
                entity: "session",
                id: session_id.to_string(),
            })?;

        if session.agent_id != auth.agent_id {
            return Err(crate::error::NcheError::Forbidden {
                message: "Session belongs to a different agent".to_string(),
            });
        }

        // Fetch tenant for internal domain checking
        let tenant = state.db.get_tenant(&auth.tenant_id).await?;

        // Create action in proposed state
        let action = state
            .db
            .create_action(&auth.tenant_id, &session_id, &req.tool, req.params)
            .await?;

        // Build policy context with blocked domains and tenant
        let policy_ctx = crate::policy::PolicyContext::new()
            .with_blocked_domains(&state.blocked_email_domains);
        let policy_ctx = if let Some(ref t) = tenant {
            policy_ctx.with_tenant(t)
        } else {
            policy_ctx
        };

        // Evaluate policy with context
        let decision = crate::policy::PolicyEngine::evaluate_with_context(&session, &action, &policy_ctx);

        // Apply policy result to action state
        let new_state = action.state.apply_policy(decision.result)?;

        // Update action with policy result
        state
            .db
            .update_action_policy(
                &auth.tenant_id,
                &action.id,
                new_state,
                decision.result,
                &decision.reason,
            )
            .await?;

        // Create approval record if needed
        let approval = if new_state == ActionState::PausedForApproval {
            Some(state.db.create_approval(&auth.tenant_id, &action.id).await?)
        } else {
            None
        };

        // Log event
        state
            .db
            .create_event(
                &auth.tenant_id,
                Some(&session_id),
                Some(&action.id),
                "action.proposed",
                serde_json::json!({
                    "tool": req.tool,
                    "policy_result": decision.result,
                    "policy_reason": decision.reason,
                    "new_state": new_state,
                }),
            )
            .await?;

        // Queue webhook if approval required
        if new_state == ActionState::PausedForApproval {
            if let Err(e) = crate::webhooks::queue_webhook(
                &state.db,
                &auth.tenant_id,
                crate::domain::WebhookEventType::ApprovalRequired,
                serde_json::json!({
                    "action_id": action.id.to_string(),
                    "session_id": session_id.to_string(),
                    "tool": req.tool,
                    "params": action.params,
                    "policy_reason": decision.reason,
                }),
            )
            .await
            {
                tracing::warn!("Failed to queue approval_required webhook: {}", e);
            }
        }

        // Fetch updated action
        let action = state
            .db
            .get_action(&auth.tenant_id, &action.id)
            .await?
            .unwrap();

        Ok((StatusCode::CREATED, Json(ActionResponse { action, approval })))
    }

    #[derive(Deserialize)]
    pub struct ListActionsQuery {
        pub session_id: Option<String>,
        pub state: Option<String>,
        pub limit: Option<i64>,
        pub offset: Option<i64>,
    }

    pub async fn list_actions(
        State(state): State<AppState>,
        Extension(auth): Extension<AgentAuthContext>,
        Query(query): Query<ListActionsQuery>,
    ) -> Result<impl IntoResponse> {
        let session_id = query.session_id.map(SessionId::from_string);
        let action_state = query.state.and_then(|s| match s.as_str() {
            "proposed" => Some(ActionState::Proposed),
            "paused_for_approval" => Some(ActionState::PausedForApproval),
            "ready_to_execute" => Some(ActionState::ReadyToExecute),
            "pending_execution" => Some(ActionState::PendingExecution),
            "executed" => Some(ActionState::Executed),
            "denied" => Some(ActionState::Denied),
            "failed" => Some(ActionState::Failed),
            _ => None,
        });

        let limit = query.limit.unwrap_or(50);
        let offset = query.offset.unwrap_or(0);

        // Fetch limit + 1 to determine if there are more items
        let actions = state
            .db
            .list_actions(
                &auth.tenant_id,
                session_id.as_ref(),
                action_state,
                limit + 1,
                offset,
            )
            .await?;

        Ok(Json(PaginatedResponse::from_items(actions, limit, offset)))
    }

    #[derive(Serialize)]
    pub struct ActionStatusResponse {
        #[serde(flatten)]
        pub action: Action,
        /// Whether the action is in a terminal state (executed, denied, or failed)
        pub is_complete: bool,
        /// Whether the action is waiting for human approval
        pub needs_approval: bool,
        /// Whether the action is ready to be executed
        pub ready_to_execute: bool,
        /// Associated approval record, if any
        pub approval: Option<Approval>,
    }

    pub async fn get_action(
        State(state): State<AppState>,
        Extension(auth): Extension<AgentAuthContext>,
        Path(id): Path<String>,
    ) -> Result<impl IntoResponse> {
        let action_id = ActionId::from_string(id);
        let action = state
            .db
            .get_action(&auth.tenant_id, &action_id)
            .await?
            .ok_or_else(|| crate::error::NcheError::NotFound {
                entity: "action",
                id: action_id.to_string(),
            })?;

        // Fetch approval if action is/was paused for approval
        let approval = if action.state == ActionState::PausedForApproval
            || action.policy_result == Some(PolicyResult::RequireApproval)
        {
            state
                .db
                .get_approval_by_action(&auth.tenant_id, &action_id)
                .await?
        } else {
            None
        };

        let response = ActionStatusResponse {
            is_complete: action.state.is_terminal(),
            needs_approval: action.state.is_pending_approval(),
            ready_to_execute: action.state == ActionState::ReadyToExecute,
            action,
            approval,
        };

        Ok(Json(response))
    }

    // === Execution Result Reporting ===

    #[derive(Deserialize)]
    pub struct ReportResultRequest {
        /// Whether the tool execution was successful
        pub success: bool,
        /// Optional result data from execution
        pub result: Option<serde_json::Value>,
        /// Optional error message if execution failed
        pub error: Option<String>,
        /// Identifier of who/what executed the action
        pub executed_by: String,
    }

    /// Report the result of an action execution.
    /// Called by tenants after they receive the execution webhook and execute the tool.
    pub async fn report_action_result(
        State(state): State<AppState>,
        Extension(auth): Extension<AgentAuthContext>,
        Path(id): Path<String>,
        Json(req): Json<ReportResultRequest>,
    ) -> Result<impl IntoResponse> {
        let action_id = ActionId::from_string(id);

        // Verify action exists and is in pending_execution state
        let action = state
            .db
            .get_action(&auth.tenant_id, &action_id)
            .await?
            .ok_or_else(|| crate::error::NcheError::NotFound {
                entity: "action",
                id: action_id.to_string(),
            })?;

        if action.state != ActionState::PendingExecution {
            return Err(crate::error::NcheError::BadRequest {
                message: format!(
                    "Action is in '{:?}' state, expected 'pending_execution'",
                    action.state
                ),
            });
        }

        // Record the execution result
        let updated = state
            .db
            .record_execution_result(
                &auth.tenant_id,
                &action_id,
                req.success,
                req.result.clone(),
                req.error.as_deref(),
                &req.executed_by,
            )
            .await?;

        if !updated {
            return Err(crate::error::NcheError::BadRequest {
                message: "Failed to update action - it may have already been updated".to_string(),
            });
        }

        // Log event
        let event_type = if req.success {
            "action.executed"
        } else {
            "action.failed"
        };
        state
            .db
            .create_event(
                &auth.tenant_id,
                Some(&action.session_id),
                Some(&action_id),
                event_type,
                serde_json::json!({
                    "tool": action.tool,
                    "success": req.success,
                    "executed_by": req.executed_by,
                    "result": req.result,
                    "error": req.error,
                }),
            )
            .await?;

        // Queue webhook for execution result
        let webhook_event_type = if req.success {
            crate::domain::WebhookEventType::ActionExecuted
        } else {
            crate::domain::WebhookEventType::ActionFailed
        };
        if let Err(e) = crate::webhooks::queue_webhook(
            &state.db,
            &auth.tenant_id,
            webhook_event_type,
            serde_json::json!({
                "action_id": action_id.to_string(),
                "session_id": action.session_id.to_string(),
                "tool": action.tool,
                "success": req.success,
                "executed_by": req.executed_by,
                "result": req.result,
                "error": req.error,
            }),
        )
        .await
        {
            tracing::warn!("Failed to queue {} webhook: {}", event_type, e);
        }

        // Fetch updated action
        let action = state
            .db
            .get_action(&auth.tenant_id, &action_id)
            .await?
            .unwrap();

        Ok(Json(action))
    }

    // === Approval Handlers ===

    #[derive(Deserialize)]
    pub struct ListApprovalsQuery {
        pub status: Option<String>,
        pub limit: Option<i64>,
        pub offset: Option<i64>,
    }

    pub async fn list_approvals(
        State(state): State<AppState>,
        Extension(auth): Extension<AgentAuthContext>,
        Query(query): Query<ListApprovalsQuery>,
    ) -> Result<impl IntoResponse> {
        let status = query.status.and_then(|s| match s.as_str() {
            "pending" => Some(ApprovalStatus::Pending),
            "approved" => Some(ApprovalStatus::Approved),
            "denied" => Some(ApprovalStatus::Denied),
            _ => None,
        });

        let limit = query.limit.unwrap_or(50);
        let offset = query.offset.unwrap_or(0);

        let approvals = state
            .db
            .list_approvals(&auth.tenant_id, status, limit + 1, offset)
            .await?;

        Ok(Json(PaginatedResponse::from_items(approvals, limit, offset)))
    }

    #[derive(Deserialize)]
    pub struct UpdateApprovalRequest {
        pub approved: bool,
        pub approver_id: String,
        pub note: Option<String>,
    }

    pub async fn update_approval(
        State(state): State<AppState>,
        Extension(auth): Extension<AgentAuthContext>,
        Path(id): Path<String>,
        Json(req): Json<UpdateApprovalRequest>,
    ) -> Result<impl IntoResponse> {
        let approval_id = ApprovalId::from_string(id);

        // Atomically update approval and action state
        let (approval, new_state) = state
            .db
            .decide_approval(
                &auth.tenant_id,
                &approval_id,
                req.approved,
                &req.approver_id,
                req.note.as_deref(),
            )
            .await?
            .ok_or_else(|| crate::error::NcheError::NotFound {
                entity: "approval",
                id: approval_id.to_string(),
            })?;

        // Get action for session_id (for event logging)
        let action = state
            .db
            .get_action(&auth.tenant_id, &approval.action_id)
            .await?
            .ok_or_else(|| crate::error::NcheError::NotFound {
                entity: "action",
                id: approval.action_id.to_string(),
            })?;

        // Log event
        let event_type = if req.approved {
            "action.approved"
        } else {
            "action.denied"
        };
        state
            .db
            .create_event(
                &auth.tenant_id,
                Some(&action.session_id),
                Some(&action.id),
                event_type,
                serde_json::json!({
                    "approver_id": req.approver_id,
                    "note": req.note,
                    "new_state": format!("{:?}", new_state),
                }),
            )
            .await?;

        // Queue webhook for approval decision
        let webhook_event_type = if req.approved {
            crate::domain::WebhookEventType::ActionApproved
        } else {
            crate::domain::WebhookEventType::ActionDenied
        };
        if let Err(e) = crate::webhooks::queue_webhook(
            &state.db,
            &auth.tenant_id,
            webhook_event_type,
            serde_json::json!({
                "action_id": action.id.to_string(),
                "session_id": action.session_id.to_string(),
                "tool": action.tool,
                "approver_id": req.approver_id,
                "note": req.note,
            }),
        )
        .await
        {
            tracing::warn!("Failed to queue {} webhook: {}", event_type, e);
        }

        Ok(Json(approval))
    }

    // === Dashboard Handlers ===

    #[derive(Deserialize)]
    pub struct LoginRequest {
        pub email: String,
        pub password: String,
    }

    #[derive(Serialize)]
    pub struct LoginResponse {
        pub success: bool,
        pub user_id: String,
        pub tenant_id: String,
    }

    pub async fn dashboard_login(
        State(state): State<AppState>,
        Json(req): Json<LoginRequest>,
    ) -> Result<impl IntoResponse> {
        // Look up user by email and verify password
        // The user's tenant_id comes from their user record
        let user = state
            .db
            .verify_dashboard_user(&req.email, &req.password)
            .await?
            .ok_or_else(|| crate::error::NcheError::Unauthorized {
                message: "Invalid email or password".to_string(),
            })?;

        // Create session (8 hours) - tenant_id comes from user
        let expires_at = time::OffsetDateTime::now_utc() + time::Duration::hours(8);
        let session = state
            .db
            .create_dashboard_session(&user.id, &user.tenant_id, expires_at)
            .await?;

        // Set cookie
        let cookie = format!(
            "nche_session={}; HttpOnly; SameSite=Lax; Path=/; Max-Age=28800",
            session.id.0
        );

        Ok((
            [(axum::http::header::SET_COOKIE, cookie)],
            Json(LoginResponse {
                success: true,
                user_id: user.id.to_string(),
                tenant_id: user.tenant_id.to_string(),
            }),
        ))
    }

    pub async fn dashboard_logout(
        State(state): State<AppState>,
        Extension(auth): Extension<DashboardAuthContext>,
    ) -> Result<impl IntoResponse> {
        state.db.delete_dashboard_session(&auth.session_id).await?;

        // Clear cookie
        let cookie = "nche_session=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0";

        Ok((
            [(axum::http::header::SET_COOKIE, cookie.to_string())],
            Json(serde_json::json!({ "success": true })),
        ))
    }

    pub async fn list_pending_approvals(
        State(state): State<AppState>,
        Extension(auth): Extension<DashboardAuthContext>,
        Query(query): Query<ListApprovalsQuery>,
    ) -> Result<impl IntoResponse> {
        let limit = query.limit.unwrap_or(50);
        let offset = query.offset.unwrap_or(0);

        let approvals = state
            .db
            .list_approvals(&auth.tenant_id, Some(ApprovalStatus::Pending), limit + 1, offset)
            .await?;

        Ok(Json(PaginatedResponse::from_items(approvals, limit, offset)))
    }

    #[derive(Serialize)]
    pub struct ApprovalDetail {
        pub approval: Approval,
        pub action: Action,
        pub events: Vec<Event>,
    }

    pub async fn get_approval_detail(
        State(state): State<AppState>,
        Extension(auth): Extension<DashboardAuthContext>,
        Path(id): Path<String>,
    ) -> Result<impl IntoResponse> {
        let approval_id = ApprovalId::from_string(id);

        let approval = state
            .db
            .get_approval(&auth.tenant_id, &approval_id)
            .await?
            .ok_or_else(|| crate::error::NcheError::NotFound {
                entity: "approval",
                id: approval_id.to_string(),
            })?;

        let action = state
            .db
            .get_action(&auth.tenant_id, &approval.action_id)
            .await?
            .ok_or_else(|| crate::error::NcheError::NotFound {
                entity: "action",
                id: approval.action_id.to_string(),
            })?;

        let events = state
            .db
            .get_action_events(&auth.tenant_id, &approval.action_id)
            .await?;

        Ok(Json(ApprovalDetail {
            approval,
            action,
            events,
        }))
    }

    pub async fn dashboard_update_approval(
        State(state): State<AppState>,
        Extension(auth): Extension<DashboardAuthContext>,
        Path(id): Path<String>,
        Json(req): Json<UpdateApprovalRequest>,
    ) -> Result<impl IntoResponse> {
        let approval_id = ApprovalId::from_string(id);

        // Use the user_id as approver
        let approver_id = auth.user_id.to_string();

        // Atomically update approval and action state
        let (approval, new_state) = state
            .db
            .decide_approval(
                &auth.tenant_id,
                &approval_id,
                req.approved,
                &approver_id,
                req.note.as_deref(),
            )
            .await?
            .ok_or_else(|| crate::error::NcheError::NotFound {
                entity: "approval",
                id: approval_id.to_string(),
            })?;

        // Get action for session_id (for event logging)
        let action = state
            .db
            .get_action(&auth.tenant_id, &approval.action_id)
            .await?
            .ok_or_else(|| crate::error::NcheError::NotFound {
                entity: "action",
                id: approval.action_id.to_string(),
            })?;

        // Log event
        let event_type = if req.approved {
            "action.approved"
        } else {
            "action.denied"
        };
        state
            .db
            .create_event(
                &auth.tenant_id,
                Some(&action.session_id),
                Some(&action.id),
                event_type,
                serde_json::json!({
                    "approver_id": approver_id,
                    "note": req.note,
                    "via": "dashboard",
                    "new_state": format!("{:?}", new_state),
                }),
            )
            .await?;

        // Queue webhook for approval decision
        let webhook_event_type = if req.approved {
            crate::domain::WebhookEventType::ActionApproved
        } else {
            crate::domain::WebhookEventType::ActionDenied
        };
        if let Err(e) = crate::webhooks::queue_webhook(
            &state.db,
            &auth.tenant_id,
            webhook_event_type,
            serde_json::json!({
                "action_id": action.id.to_string(),
                "session_id": action.session_id.to_string(),
                "tool": action.tool,
                "approver_id": approver_id,
                "note": req.note,
                "via": "dashboard",
            }),
        )
        .await
        {
            tracing::warn!("Failed to queue {} webhook: {}", event_type, e);
        }

        Ok(Json(approval))
    }

    #[derive(Deserialize)]
    pub struct ListEventsQuery {
        pub session_id: Option<String>,
        pub action_id: Option<String>,
        pub limit: Option<i64>,
        pub offset: Option<i64>,
    }

    pub async fn list_events(
        State(state): State<AppState>,
        Extension(auth): Extension<DashboardAuthContext>,
        Query(query): Query<ListEventsQuery>,
    ) -> Result<impl IntoResponse> {
        let session_id = query.session_id.map(SessionId::from_string);
        let action_id = query.action_id.map(ActionId::from_string);

        let limit = query.limit.unwrap_or(100);
        let offset = query.offset.unwrap_or(0);

        let events = state
            .db
            .list_events(
                &auth.tenant_id,
                session_id.as_ref(),
                action_id.as_ref(),
                limit + 1,
                offset,
            )
            .await?;

        Ok(Json(PaginatedResponse::from_items(events, limit, offset)))
    }

    #[derive(Serialize)]
    pub struct CurrentUser {
        pub user_id: String,
        pub tenant_id: String,
    }

    pub async fn get_current_user(Extension(auth): Extension<DashboardAuthContext>) -> impl IntoResponse {
        Json(CurrentUser {
            user_id: auth.user_id.to_string(),
            tenant_id: auth.tenant_id.to_string(),
        })
    }

    // === Dashboard Management Handlers ===

    #[derive(Serialize)]
    pub struct AgentListItem {
        pub id: String,
        pub name: String,
        pub api_key_prefix: String,
        pub created_at: time::OffsetDateTime,
    }

    pub async fn dashboard_list_agents(
        State(state): State<AppState>,
        Extension(auth): Extension<DashboardAuthContext>,
        Query(query): Query<PaginationQuery>,
    ) -> Result<impl IntoResponse> {
        let agents = state
            .db
            .list_agents(&auth.tenant_id, query.limit.unwrap_or(50))
            .await?;

        let items: Vec<AgentListItem> = agents
            .into_iter()
            .map(|a| AgentListItem {
                id: a.id.to_string(),
                name: a.name,
                api_key_prefix: a.api_key_prefix,
                created_at: a.created_at,
            })
            .collect();

        Ok(Json(items))
    }

    #[derive(Deserialize)]
    pub struct CreateAgentRequest {
        pub name: String,
    }

    #[derive(Serialize)]
    pub struct CreateAgentResponse {
        pub id: String,
        pub name: String,
        pub api_key: String,
        pub api_key_prefix: String,
    }

    pub async fn dashboard_create_agent(
        State(state): State<AppState>,
        Extension(auth): Extension<DashboardAuthContext>,
        Json(req): Json<CreateAgentRequest>,
    ) -> Result<impl IntoResponse> {
        let (agent, api_key) = state
            .db
            .create_agent_with_key(&auth.tenant_id, &req.name)
            .await?;

        Ok((
            StatusCode::CREATED,
            Json(CreateAgentResponse {
                id: agent.id.to_string(),
                name: agent.name,
                api_key,
                api_key_prefix: agent.api_key_prefix,
            }),
        ))
    }

    #[derive(Deserialize)]
    pub struct ListSessionsQuery {
        pub agent_id: Option<String>,
        pub active_only: Option<bool>,
        pub limit: Option<i64>,
    }

    pub async fn dashboard_list_sessions(
        State(state): State<AppState>,
        Extension(auth): Extension<DashboardAuthContext>,
        Query(query): Query<ListSessionsQuery>,
    ) -> Result<impl IntoResponse> {
        let agent_id = query.agent_id.map(AgentId::from_string);

        let sessions = state
            .db
            .list_sessions(
                &auth.tenant_id,
                agent_id.as_ref(),
                query.active_only.unwrap_or(false),
                query.limit.unwrap_or(50),
            )
            .await?;

        Ok(Json(sessions))
    }

    #[derive(Deserialize)]
    pub struct DashboardListActionsQuery {
        pub session_id: Option<String>,
        pub state: Option<String>,
        pub limit: Option<i64>,
        pub offset: Option<i64>,
    }

    pub async fn dashboard_list_actions(
        State(state): State<AppState>,
        Extension(auth): Extension<DashboardAuthContext>,
        Query(query): Query<DashboardListActionsQuery>,
    ) -> Result<impl IntoResponse> {
        let session_id = query.session_id.map(SessionId::from_string);
        let action_state = query.state.and_then(|s| match s.as_str() {
            "proposed" => Some(ActionState::Proposed),
            "paused_for_approval" => Some(ActionState::PausedForApproval),
            "ready_to_execute" => Some(ActionState::ReadyToExecute),
            "pending_execution" => Some(ActionState::PendingExecution),
            "executed" => Some(ActionState::Executed),
            "denied" => Some(ActionState::Denied),
            "failed" => Some(ActionState::Failed),
            _ => None,
        });

        let limit = query.limit.unwrap_or(50);
        let offset = query.offset.unwrap_or(0);

        let actions = state
            .db
            .list_actions(
                &auth.tenant_id,
                session_id.as_ref(),
                action_state,
                limit + 1,
                offset,
            )
            .await?;

        Ok(Json(PaginatedResponse::from_items(actions, limit, offset)))
    }

    pub async fn dashboard_get_action(
        State(state): State<AppState>,
        Extension(auth): Extension<DashboardAuthContext>,
        Path(id): Path<String>,
    ) -> Result<impl IntoResponse> {
        let action_id = ActionId::from_string(id);

        let action = state
            .db
            .get_action(&auth.tenant_id, &action_id)
            .await?
            .ok_or_else(|| crate::error::NcheError::NotFound {
                entity: "action",
                id: action_id.to_string(),
            })?;

        // Get associated approval if any
        let approval = state
            .db
            .get_approval_by_action(&auth.tenant_id, &action_id)
            .await?;

        // Get events for this action
        let events = state
            .db
            .get_action_events(&auth.tenant_id, &action_id)
            .await?;

        Ok(Json(serde_json::json!({
            "action": action,
            "approval": approval,
            "events": events,
        })))
    }

    #[derive(Serialize)]
    pub struct DashboardStats {
        pub pending_approvals: i64,
        pub total_agents: i64,
        pub active_sessions: i64,
        pub actions_today: i64,
        pub actions_by_state: std::collections::HashMap<String, i64>,
    }

    pub async fn dashboard_stats(
        State(state): State<AppState>,
        Extension(auth): Extension<DashboardAuthContext>,
    ) -> Result<impl IntoResponse> {
        // Count pending approvals
        let pending_approvals = state.db.count_pending_approvals(&auth.tenant_id).await?;

        // Count agents
        let agents = state.db.list_agents(&auth.tenant_id, 1000).await?;
        let total_agents = agents.len() as i64;

        // Count active sessions
        let active_sessions = state
            .db
            .list_sessions(&auth.tenant_id, None, true, 1000)
            .await?
            .len() as i64;

        // Get action counts by state (simplified - just counts recent actions)
        let mut actions_by_state = std::collections::HashMap::new();
        for state_name in ["proposed", "paused_for_approval", "ready_to_execute", "pending_execution", "executed", "denied", "failed"] {
            let action_state = match state_name {
                "proposed" => ActionState::Proposed,
                "paused_for_approval" => ActionState::PausedForApproval,
                "ready_to_execute" => ActionState::ReadyToExecute,
                "pending_execution" => ActionState::PendingExecution,
                "executed" => ActionState::Executed,
                "denied" => ActionState::Denied,
                "failed" => ActionState::Failed,
                _ => continue,
            };
            let count = state
                .db
                .list_actions(&auth.tenant_id, None, Some(action_state), 1000, 0)
                .await?
                .len() as i64;
            actions_by_state.insert(state_name.to_string(), count);
        }

        // Actions today (approximate - would need a proper query)
        let actions_today = actions_by_state.values().sum();

        Ok(Json(DashboardStats {
            pending_approvals,
            total_agents,
            active_sessions,
            actions_today,
            actions_by_state,
        }))
    }

    #[derive(Deserialize)]
    pub struct PaginationQuery {
        pub limit: Option<i64>,
        pub offset: Option<i64>,
    }

    /// Generic paginated response wrapper for list endpoints
    #[derive(Serialize)]
    pub struct PaginatedResponse<T: Serialize> {
        /// The items for the current page
        pub data: Vec<T>,
        /// The limit used for this query
        pub limit: i64,
        /// The offset used for this query
        pub offset: i64,
        /// Whether there are more items after this page
        pub has_more: bool,
    }

    impl<T: Serialize> PaginatedResponse<T> {
        /// Create a paginated response by checking if we have more items than requested
        ///
        /// This uses the "fetch N+1" pattern: fetch one more item than the limit
        /// to determine if there are more pages, without needing a COUNT query.
        pub fn from_items(mut items: Vec<T>, limit: i64, offset: i64) -> Self {
            let has_more = items.len() as i64 > limit;
            if has_more {
                items.pop(); // Remove the extra item we fetched
            }
            Self {
                data: items,
                limit,
                offset,
                has_more,
            }
        }
    }

    // === Tenant Config Handlers ===

    /// Response type for tenant configuration.
    /// Only includes config-related fields, not sensitive data like webhook secrets.
    #[derive(Serialize)]
    pub struct TenantConfigResponse {
        pub execution_webhook_url: Option<String>,
        pub execution_webhook_timeout_ms: Option<i32>,
        pub policy_mode: Option<String>,
        pub policy_webhook_url: Option<String>,
        pub policy_webhook_timeout_ms: Option<i32>,
    }

    /// Get current tenant configuration (Agent API)
    pub async fn get_tenant_config(
        State(state): State<AppState>,
        Extension(auth): Extension<AgentAuthContext>,
    ) -> Result<impl IntoResponse> {
        let tenant = state
            .db
            .get_tenant(&auth.tenant_id)
            .await?
            .ok_or_else(|| crate::error::NcheError::NotFound {
                entity: "tenant",
                id: auth.tenant_id.to_string(),
            })?;

        Ok(Json(TenantConfigResponse {
            execution_webhook_url: tenant.execution_webhook_url,
            execution_webhook_timeout_ms: tenant.execution_webhook_timeout_ms,
            policy_mode: tenant.policy_mode,
            policy_webhook_url: tenant.policy_webhook_url,
            policy_webhook_timeout_ms: tenant.policy_webhook_timeout_ms,
        }))
    }

    /// Request to update tenant configuration
    #[derive(Deserialize)]
    pub struct UpdateTenantConfigRequest {
        pub execution_webhook_url: Option<String>,
        pub execution_webhook_secret: Option<String>,
        pub execution_webhook_timeout_ms: Option<i32>,
        pub policy_mode: Option<String>,
        pub policy_webhook_url: Option<String>,
        pub policy_webhook_secret: Option<String>,
        pub policy_webhook_timeout_ms: Option<i32>,
    }

    /// Update tenant configuration (Agent API)
    pub async fn update_tenant_config(
        State(state): State<AppState>,
        Extension(auth): Extension<AgentAuthContext>,
        Json(req): Json<UpdateTenantConfigRequest>,
    ) -> Result<impl IntoResponse> {
        // Validate policy_mode if provided
        if let Some(ref mode) = req.policy_mode {
            if mode != "builtin" && mode != "webhook" {
                return Err(crate::error::NcheError::BadRequest {
                    message: format!(
                        "Invalid policy_mode '{}'. Must be 'builtin' or 'webhook'",
                        mode
                    ),
                });
            }
        }

        let tenant = state
            .db
            .update_tenant_config(
                &auth.tenant_id,
                req.execution_webhook_url.as_ref().map(|u| Some(u.as_str())),
                req.execution_webhook_secret.as_ref().map(|s| Some(s.as_str())),
                req.execution_webhook_timeout_ms.map(Some),
                req.policy_mode.as_deref(),
                req.policy_webhook_url.as_ref().map(|u| Some(u.as_str())),
                req.policy_webhook_secret.as_ref().map(|s| Some(s.as_str())),
                req.policy_webhook_timeout_ms.map(Some),
            )
            .await?
            .ok_or_else(|| crate::error::NcheError::NotFound {
                entity: "tenant",
                id: auth.tenant_id.to_string(),
            })?;

        Ok(Json(TenantConfigResponse {
            execution_webhook_url: tenant.execution_webhook_url,
            execution_webhook_timeout_ms: tenant.execution_webhook_timeout_ms,
            policy_mode: tenant.policy_mode,
            policy_webhook_url: tenant.policy_webhook_url,
            policy_webhook_timeout_ms: tenant.policy_webhook_timeout_ms,
        }))
    }

    /// Get current tenant configuration (Dashboard API)
    pub async fn dashboard_get_tenant_config(
        State(state): State<AppState>,
        Extension(auth): Extension<DashboardAuthContext>,
    ) -> Result<impl IntoResponse> {
        let tenant = state
            .db
            .get_tenant(&auth.tenant_id)
            .await?
            .ok_or_else(|| crate::error::NcheError::NotFound {
                entity: "tenant",
                id: auth.tenant_id.to_string(),
            })?;

        Ok(Json(TenantConfigResponse {
            execution_webhook_url: tenant.execution_webhook_url,
            execution_webhook_timeout_ms: tenant.execution_webhook_timeout_ms,
            policy_mode: tenant.policy_mode,
            policy_webhook_url: tenant.policy_webhook_url,
            policy_webhook_timeout_ms: tenant.policy_webhook_timeout_ms,
        }))
    }

    /// Update tenant configuration (Dashboard API)
    pub async fn dashboard_update_tenant_config(
        State(state): State<AppState>,
        Extension(auth): Extension<DashboardAuthContext>,
        Json(req): Json<UpdateTenantConfigRequest>,
    ) -> Result<impl IntoResponse> {
        // Validate policy_mode if provided
        if let Some(ref mode) = req.policy_mode {
            if mode != "builtin" && mode != "webhook" {
                return Err(crate::error::NcheError::BadRequest {
                    message: format!(
                        "Invalid policy_mode '{}'. Must be 'builtin' or 'webhook'",
                        mode
                    ),
                });
            }
        }

        let tenant = state
            .db
            .update_tenant_config(
                &auth.tenant_id,
                req.execution_webhook_url.as_ref().map(|u| Some(u.as_str())),
                req.execution_webhook_secret.as_ref().map(|s| Some(s.as_str())),
                req.execution_webhook_timeout_ms.map(Some),
                req.policy_mode.as_deref(),
                req.policy_webhook_url.as_ref().map(|u| Some(u.as_str())),
                req.policy_webhook_secret.as_ref().map(|s| Some(s.as_str())),
                req.policy_webhook_timeout_ms.map(Some),
            )
            .await?
            .ok_or_else(|| crate::error::NcheError::NotFound {
                entity: "tenant",
                id: auth.tenant_id.to_string(),
            })?;

        Ok(Json(TenantConfigResponse {
            execution_webhook_url: tenant.execution_webhook_url,
            execution_webhook_timeout_ms: tenant.execution_webhook_timeout_ms,
            policy_mode: tenant.policy_mode,
            policy_webhook_url: tenant.policy_webhook_url,
            policy_webhook_timeout_ms: tenant.policy_webhook_timeout_ms,
        }))
    }
}
