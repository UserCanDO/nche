use sqlx::Row;
use time::OffsetDateTime;

use crate::domain::{ActionId, Approval, ApprovalId, ApprovalStatus, TenantId};
use crate::error::{NcheError, Result};

use super::Database;

impl Database {
    pub async fn create_approval(
        &self,
        tenant_id: &TenantId,
        action_id: &ActionId,
    ) -> Result<Approval> {
        let id = ApprovalId::new();
        let now = OffsetDateTime::now_utc();
        let status = ApprovalStatus::Pending;

        sqlx::query(
            r#"
            INSERT INTO approvals (id, tenant_id, action_id, status, created_at)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(&id.0)
        .bind(&tenant_id.0)
        .bind(&action_id.0)
        .bind(approval_status_to_str(status))
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(Approval {
            id,
            tenant_id: tenant_id.clone(),
            action_id: action_id.clone(),
            status,
            approver_id: None,
            approver_note: None,
            created_at: now,
            decided_at: None,
        })
    }

    pub async fn get_approval(
        &self,
        tenant_id: &TenantId,
        approval_id: &ApprovalId,
    ) -> Result<Option<Approval>> {
        let row = sqlx::query(
            r#"
            SELECT id, tenant_id, action_id, status, approver_id, approver_note, created_at, decided_at
            FROM approvals WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(&tenant_id.0)
        .bind(&approval_id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(row.map(row_to_approval))
    }

    pub async fn get_approval_by_action(
        &self,
        tenant_id: &TenantId,
        action_id: &ActionId,
    ) -> Result<Option<Approval>> {
        let row = sqlx::query(
            r#"
            SELECT id, tenant_id, action_id, status, approver_id, approver_note, created_at, decided_at
            FROM approvals WHERE tenant_id = $1 AND action_id = $2
            "#,
        )
        .bind(&tenant_id.0)
        .bind(&action_id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(row.map(row_to_approval))
    }

    pub async fn list_approvals(
        &self,
        tenant_id: &TenantId,
        status: Option<ApprovalStatus>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Approval>> {
        let query = if status.is_some() {
            r#"
            SELECT id, tenant_id, action_id, status, approver_id, approver_note, created_at, decided_at
            FROM approvals WHERE tenant_id = $1 AND status = $2
            ORDER BY created_at DESC LIMIT $3 OFFSET $4
            "#
        } else {
            r#"
            SELECT id, tenant_id, action_id, status, approver_id, approver_note, created_at, decided_at
            FROM approvals WHERE tenant_id = $1
            ORDER BY created_at DESC LIMIT $2 OFFSET $3
            "#
        };

        let rows = if let Some(s) = status {
            sqlx::query(query)
                .bind(&tenant_id.0)
                .bind(approval_status_to_str(s))
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await
        } else {
            sqlx::query(query)
                .bind(&tenant_id.0)
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await
        }
        .map_err(NcheError::Database)?;

        Ok(rows.into_iter().map(row_to_approval).collect())
    }

    /// Update approval decision (approve or deny)
    pub async fn update_approval(
        &self,
        tenant_id: &TenantId,
        approval_id: &ApprovalId,
        approved: bool,
        approver_id: &str,
        approver_note: Option<&str>,
    ) -> Result<Option<Approval>> {
        let status = if approved {
            ApprovalStatus::Approved
        } else {
            ApprovalStatus::Denied
        };

        let row = sqlx::query(
            r#"
            UPDATE approvals
            SET status = $3, approver_id = $4, approver_note = $5, decided_at = now()
            WHERE tenant_id = $1 AND id = $2 AND status = 'pending'
            RETURNING id, tenant_id, action_id, status, approver_id, approver_note, created_at, decided_at
            "#,
        )
        .bind(&tenant_id.0)
        .bind(&approval_id.0)
        .bind(approval_status_to_str(status))
        .bind(approver_id)
        .bind(approver_note)
        .fetch_optional(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(row.map(row_to_approval))
    }

    /// List approvals across all tenants (admin CLI)
    pub async fn list_all_approvals(
        &self,
        tenant_id: Option<&TenantId>,
        status: Option<ApprovalStatus>,
        limit: i64,
    ) -> Result<Vec<Approval>> {
        let (query, has_tenant, has_status) = match (tenant_id.is_some(), status.is_some()) {
            (true, true) => (
                r#"
                SELECT id, tenant_id, action_id, status, approver_id, approver_note, created_at, decided_at
                FROM approvals WHERE tenant_id = $1 AND status = $2
                ORDER BY created_at DESC LIMIT $3
                "#,
                true,
                true,
            ),
            (true, false) => (
                r#"
                SELECT id, tenant_id, action_id, status, approver_id, approver_note, created_at, decided_at
                FROM approvals WHERE tenant_id = $1
                ORDER BY created_at DESC LIMIT $2
                "#,
                true,
                false,
            ),
            (false, true) => (
                r#"
                SELECT id, tenant_id, action_id, status, approver_id, approver_note, created_at, decided_at
                FROM approvals WHERE status = $1
                ORDER BY created_at DESC LIMIT $2
                "#,
                false,
                true,
            ),
            (false, false) => (
                r#"
                SELECT id, tenant_id, action_id, status, approver_id, approver_note, created_at, decided_at
                FROM approvals
                ORDER BY created_at DESC LIMIT $1
                "#,
                false,
                false,
            ),
        };

        let rows = match (has_tenant, has_status) {
            (true, true) => {
                sqlx::query(query)
                    .bind(&tenant_id.unwrap().0)
                    .bind(approval_status_to_str(status.unwrap()))
                    .bind(limit)
                    .fetch_all(&self.pool)
                    .await
            }
            (true, false) => {
                sqlx::query(query)
                    .bind(&tenant_id.unwrap().0)
                    .bind(limit)
                    .fetch_all(&self.pool)
                    .await
            }
            (false, true) => {
                sqlx::query(query)
                    .bind(approval_status_to_str(status.unwrap()))
                    .bind(limit)
                    .fetch_all(&self.pool)
                    .await
            }
            (false, false) => {
                sqlx::query(query)
                    .bind(limit)
                    .fetch_all(&self.pool)
                    .await
            }
        }
        .map_err(NcheError::Database)?;

        Ok(rows.into_iter().map(row_to_approval).collect())
    }

    /// Get approval by ID without tenant scope (admin CLI)
    pub async fn get_approval_by_id(&self, approval_id: &ApprovalId) -> Result<Option<Approval>> {
        let row = sqlx::query(
            r#"
            SELECT id, tenant_id, action_id, status, approver_id, approver_note, created_at, decided_at
            FROM approvals WHERE id = $1
            "#,
        )
        .bind(&approval_id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(row.map(row_to_approval))
    }

    /// Count pending approvals for a tenant
    pub async fn count_pending_approvals(&self, tenant_id: &TenantId) -> Result<i64> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count FROM approvals
            WHERE tenant_id = $1 AND status = 'pending'
            "#,
        )
        .bind(&tenant_id.0)
        .fetch_one(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(row.get::<i64, _>("count"))
    }

    /// Atomically update approval and action state in a single transaction
    /// Returns (Approval, new ActionState) or None if approval was already decided
    pub async fn decide_approval(
        &self,
        tenant_id: &TenantId,
        approval_id: &ApprovalId,
        approved: bool,
        approver_id: &str,
        approver_note: Option<&str>,
    ) -> Result<Option<(Approval, crate::domain::ActionState)>> {
        use crate::domain::ActionState;

        let approval_status = if approved {
            ApprovalStatus::Approved
        } else {
            ApprovalStatus::Denied
        };

        let new_action_state = if approved {
            ActionState::ReadyToExecute
        } else {
            ActionState::Denied
        };

        let new_action_state_str = if approved {
            "ready_to_execute"
        } else {
            "denied"
        };

        // Use a transaction to update both approval and action atomically
        let mut tx = self.pool.begin().await.map_err(NcheError::Database)?;

        // Update approval - only if still pending
        let approval_row = sqlx::query(
            r#"
            UPDATE approvals
            SET status = $3, approver_id = $4, approver_note = $5, decided_at = now()
            WHERE tenant_id = $1 AND id = $2 AND status = 'pending'
            RETURNING id, tenant_id, action_id, status, approver_id, approver_note, created_at, decided_at
            "#,
        )
        .bind(&tenant_id.0)
        .bind(&approval_id.0)
        .bind(approval_status_to_str(approval_status))
        .bind(approver_id)
        .bind(approver_note)
        .fetch_optional(&mut *tx)
        .await
        .map_err(NcheError::Database)?;

        let Some(approval_row) = approval_row else {
            // Approval was already decided or doesn't exist
            tx.rollback().await.map_err(NcheError::Database)?;
            return Ok(None);
        };

        let approval = row_to_approval(approval_row);

        // Update action state - only if in paused_for_approval state
        let result = sqlx::query(
            r#"
            UPDATE actions
            SET state = $3, updated_at = now()
            WHERE tenant_id = $1 AND id = $2 AND state = 'paused_for_approval'
            "#,
        )
        .bind(&tenant_id.0)
        .bind(&approval.action_id.0)
        .bind(new_action_state_str)
        .execute(&mut *tx)
        .await
        .map_err(NcheError::Database)?;

        if result.rows_affected() == 0 {
            // Action was not in the expected state - rollback
            tx.rollback().await.map_err(NcheError::Database)?;
            return Err(NcheError::InvalidStateTransition {
                from: ActionState::PausedForApproval,
                action: "decide_approval".into(),
            });
        }

        tx.commit().await.map_err(NcheError::Database)?;

        Ok(Some((approval, new_action_state)))
    }
}

fn row_to_approval(r: sqlx::postgres::PgRow) -> Approval {
    Approval {
        id: ApprovalId::from_string(r.get("id")),
        tenant_id: TenantId::from_string(r.get("tenant_id")),
        action_id: ActionId::from_string(r.get("action_id")),
        status: str_to_approval_status(r.get("status")),
        approver_id: r.get("approver_id"),
        approver_note: r.get("approver_note"),
        created_at: r.get("created_at"),
        decided_at: r.get("decided_at"),
    }
}

fn approval_status_to_str(s: ApprovalStatus) -> &'static str {
    match s {
        ApprovalStatus::Pending => "pending",
        ApprovalStatus::Approved => "approved",
        ApprovalStatus::Denied => "denied",
    }
}

fn str_to_approval_status(s: &str) -> ApprovalStatus {
    match s {
        "pending" => ApprovalStatus::Pending,
        "approved" => ApprovalStatus::Approved,
        "denied" => ApprovalStatus::Denied,
        _ => ApprovalStatus::Pending,
    }
}
