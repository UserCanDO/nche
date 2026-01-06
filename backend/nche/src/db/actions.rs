//! Action database operations.
//!
//! Actions represent tool calls proposed by agents. They follow a state machine:
//! Proposed -> [policy] -> ReadyToExecute/PausedForApproval/Denied
//! PausedForApproval -> [approval] -> ReadyToExecute/Denied
//! ReadyToExecute -> [webhook sent] -> PendingExecution
//! PendingExecution -> [tenant reports] -> Executed/Failed

use sqlx::Row;
use time::OffsetDateTime;

use crate::domain::{Action, ActionId, ActionState, PolicyResult, SessionId, TenantId};
use crate::error::{NcheError, Result};

use super::Database;

impl Database {
    pub async fn create_action(
        &self,
        tenant_id: &TenantId,
        session_id: &SessionId,
        tool: &str,
        params: serde_json::Value,
    ) -> Result<Action> {
        let id = ActionId::new();
        let now = OffsetDateTime::now_utc();
        let state = ActionState::Proposed;

        sqlx::query(
            r#"
            INSERT INTO actions (id, tenant_id, session_id, tool, params, state, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(&id.0)
        .bind(&tenant_id.0)
        .bind(&session_id.0)
        .bind(tool)
        .bind(&params)
        .bind(action_state_to_str(state))
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(Action {
            id,
            tenant_id: tenant_id.clone(),
            session_id: session_id.clone(),
            tool: tool.to_string(),
            params,
            state,
            policy_result: None,
            policy_reason: None,
            result: None,
            error: None,
            execution_result: None,
            executed_by: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn get_action(
        &self,
        tenant_id: &TenantId,
        action_id: &ActionId,
    ) -> Result<Option<Action>> {
        let row = sqlx::query(
            r#"
            SELECT id, tenant_id, session_id, tool, params, state, policy_result, policy_reason,
                   result, error, execution_result, executed_by, created_at, updated_at
            FROM actions WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(&tenant_id.0)
        .bind(&action_id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(row.map(|r| row_to_action(r)))
    }

    pub async fn list_actions(
        &self,
        tenant_id: &TenantId,
        session_id: Option<&SessionId>,
        state: Option<ActionState>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Action>> {
        let mut query = String::from(
            "SELECT id, tenant_id, session_id, tool, params, state, policy_result, policy_reason,
                    result, error, execution_result, executed_by, created_at, updated_at
             FROM actions WHERE tenant_id = $1",
        );

        let mut param_idx = 2;

        if session_id.is_some() {
            query.push_str(&format!(" AND session_id = ${}", param_idx));
            param_idx += 1;
        }

        if state.is_some() {
            query.push_str(&format!(" AND state = ${}", param_idx));
            param_idx += 1;
        }

        query.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut q = sqlx::query(&query).bind(&tenant_id.0);

        if let Some(sid) = session_id {
            q = q.bind(&sid.0);
        }

        if let Some(s) = state {
            q = q.bind(action_state_to_str(s));
        }

        q = q.bind(limit).bind(offset);

        let rows = q.fetch_all(&self.pool).await.map_err(NcheError::Database)?;

        Ok(rows.into_iter().map(row_to_action).collect())
    }

    /// Update action after policy evaluation
    pub async fn update_action_policy(
        &self,
        tenant_id: &TenantId,
        action_id: &ActionId,
        new_state: ActionState,
        policy_result: PolicyResult,
        policy_reason: &str,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE actions
            SET state = $3, policy_result = $4, policy_reason = $5, updated_at = now()
            WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(&tenant_id.0)
        .bind(&action_id.0)
        .bind(action_state_to_str(new_state))
        .bind(policy_result_to_str(policy_result))
        .bind(policy_reason)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(result.rows_affected() > 0)
    }

    /// Update action state (for approval decisions, etc.)
    pub async fn update_action_state(
        &self,
        tenant_id: &TenantId,
        action_id: &ActionId,
        new_state: ActionState,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE actions SET state = $3, updated_at = now()
            WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(&tenant_id.0)
        .bind(&action_id.0)
        .bind(action_state_to_str(new_state))
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(result.rows_affected() > 0)
    }

    /// Atomically lock ready actions for execution webhook dispatch.
    /// Transitions from ready_to_execute -> pending_execution.
    pub async fn lock_ready_actions(&self, limit: i64) -> Result<Vec<Action>> {
        let rows = sqlx::query(
            r#"
            UPDATE actions
            SET state = 'pending_execution', updated_at = now()
            WHERE id IN (
                SELECT id FROM actions
                WHERE state = 'ready_to_execute'
                ORDER BY created_at ASC
                LIMIT $1
                FOR UPDATE SKIP LOCKED
            )
            RETURNING id, tenant_id, session_id, tool, params, state, policy_result, policy_reason,
                      result, error, execution_result, executed_by, created_at, updated_at
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(rows.into_iter().map(row_to_action).collect())
    }

    /// Record execution result reported by tenant.
    /// Action must be in pending_execution state.
    pub async fn record_execution_result(
        &self,
        tenant_id: &TenantId,
        action_id: &ActionId,
        success: bool,
        execution_result: Option<serde_json::Value>,
        error: Option<&str>,
        executed_by: &str,
    ) -> Result<bool> {
        let state = if success {
            ActionState::Executed
        } else {
            ActionState::Failed
        };

        let res = sqlx::query(
            r#"
            UPDATE actions
            SET state = $3, execution_result = $4, error = $5, executed_by = $6, updated_at = now()
            WHERE tenant_id = $1 AND id = $2 AND state = 'pending_execution'
            "#,
        )
        .bind(&tenant_id.0)
        .bind(&action_id.0)
        .bind(action_state_to_str(state))
        .bind(&execution_result)
        .bind(error)
        .bind(executed_by)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(res.rows_affected() > 0)
    }

    /// Legacy method - kept for backwards compatibility.
    /// For new code, use record_execution_result instead.
    pub async fn complete_action_execution(
        &self,
        action_id: &ActionId,
        success: bool,
        result: Option<serde_json::Value>,
        error: Option<&str>,
    ) -> Result<bool> {
        let state = if success {
            ActionState::Executed
        } else {
            ActionState::Failed
        };

        let res = sqlx::query(
            r#"
            UPDATE actions
            SET state = $2, result = $3, error = $4, updated_at = now()
            WHERE id = $1
            "#,
        )
        .bind(&action_id.0)
        .bind(action_state_to_str(state))
        .bind(&result)
        .bind(error)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(res.rows_affected() > 0)
    }
}

fn row_to_action(r: sqlx::postgres::PgRow) -> Action {
    Action {
        id: ActionId::from_string(r.get("id")),
        tenant_id: TenantId::from_string(r.get("tenant_id")),
        session_id: SessionId::from_string(r.get("session_id")),
        tool: r.get("tool"),
        params: r.get("params"),
        state: str_to_action_state(r.get("state")),
        policy_result: r
            .get::<Option<String>, _>("policy_result")
            .map(|s| str_to_policy_result(&s)),
        policy_reason: r.get("policy_reason"),
        result: r.get("result"),
        error: r.get("error"),
        execution_result: r.get("execution_result"),
        executed_by: r.get("executed_by"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    }
}

fn action_state_to_str(s: ActionState) -> &'static str {
    match s {
        ActionState::Proposed => "proposed",
        ActionState::PausedForApproval => "paused_for_approval",
        ActionState::ReadyToExecute => "ready_to_execute",
        ActionState::PendingExecution => "pending_execution",
        ActionState::Executed => "executed",
        ActionState::Denied => "denied",
        ActionState::Failed => "failed",
    }
}

fn str_to_action_state(s: &str) -> ActionState {
    match s {
        "proposed" => ActionState::Proposed,
        "paused_for_approval" => ActionState::PausedForApproval,
        "ready_to_execute" => ActionState::ReadyToExecute,
        "pending_execution" => ActionState::PendingExecution,
        // Legacy: map old "executing" to PendingExecution
        "executing" => ActionState::PendingExecution,
        "executed" => ActionState::Executed,
        "denied" => ActionState::Denied,
        "failed" => ActionState::Failed,
        _ => ActionState::Proposed,
    }
}

fn policy_result_to_str(p: PolicyResult) -> &'static str {
    match p {
        PolicyResult::Allow => "allow",
        PolicyResult::Deny => "deny",
        PolicyResult::RequireApproval => "require_approval",
    }
}

fn str_to_policy_result(s: &str) -> PolicyResult {
    match s {
        "allow" => PolicyResult::Allow,
        "deny" => PolicyResult::Deny,
        "require_approval" => PolicyResult::RequireApproval,
        _ => PolicyResult::RequireApproval,
    }
}
