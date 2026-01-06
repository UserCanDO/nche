use sqlx::Row;
use time::OffsetDateTime;

use crate::domain::{Case, CaseId, CaseStatus, SessionId, Severity, TenantId};
use crate::error::{NcheError, Result};

use super::Database;

impl Database {
    pub async fn create_case(
        &self,
        tenant_id: &TenantId,
        session_id: Option<&SessionId>,
        title: &str,
        status: Option<CaseStatus>,
        severity: Option<Severity>,
        evidence: Option<serde_json::Value>,
        external_ref: Option<&str>,
    ) -> Result<Case> {
        let id = CaseId::new();
        let now = OffsetDateTime::now_utc();
        let status = status.unwrap_or(CaseStatus::Open);
        let severity = severity.unwrap_or(Severity::Medium);
        let evidence = evidence.unwrap_or_else(|| serde_json::json!([]));

        sqlx::query(
            r#"
            INSERT INTO cases (id, tenant_id, session_id, title, status, severity, evidence, external_ref, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
            "#,
        )
        .bind(&id.0)
        .bind(&tenant_id.0)
        .bind(session_id.map(|s| &s.0))
        .bind(title)
        .bind(case_status_to_str(status))
        .bind(severity_to_str(severity))
        .bind(&evidence)
        .bind(external_ref)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(Case {
            id,
            tenant_id: tenant_id.clone(),
            session_id: session_id.cloned(),
            title: title.to_string(),
            status,
            severity,
            evidence,
            external_ref: external_ref.map(String::from),
            created_at: now,
            updated_at: now,
            archived_at: None,
        })
    }

    pub async fn get_case(
        &self,
        tenant_id: &TenantId,
        case_id: &CaseId,
    ) -> Result<Option<Case>> {
        let row = sqlx::query(
            r#"
            SELECT id, tenant_id, session_id, title, status, severity, evidence, external_ref, created_at, updated_at, archived_at
            FROM cases WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(&tenant_id.0)
        .bind(&case_id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(row.map(row_to_case))
    }

    pub async fn list_cases(
        &self,
        tenant_id: &TenantId,
        session_id: Option<&SessionId>,
        status: Option<CaseStatus>,
        severity: Option<Severity>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Case>> {
        let mut query = String::from(
            "SELECT id, tenant_id, session_id, title, status, severity, evidence, external_ref, created_at, updated_at, archived_at
             FROM cases WHERE tenant_id = $1 AND archived_at IS NULL",
        );

        let mut param_idx = 2;

        if session_id.is_some() {
            query.push_str(&format!(" AND session_id = ${}", param_idx));
            param_idx += 1;
        }

        if status.is_some() {
            query.push_str(&format!(" AND status = ${}", param_idx));
            param_idx += 1;
        }

        if severity.is_some() {
            query.push_str(&format!(" AND severity = ${}", param_idx));
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

        if let Some(st) = status {
            q = q.bind(case_status_to_str(st));
        }

        if let Some(sev) = severity {
            q = q.bind(severity_to_str(sev));
        }

        q = q.bind(limit).bind(offset);

        let rows = q.fetch_all(&self.pool).await.map_err(NcheError::Database)?;

        Ok(rows.into_iter().map(row_to_case).collect())
    }

    pub async fn list_archived_cases(
        &self,
        tenant_id: &TenantId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Case>> {
        let rows = sqlx::query(
            r#"
            SELECT id, tenant_id, session_id, title, status, severity, evidence, external_ref, created_at, updated_at, archived_at
            FROM cases WHERE tenant_id = $1 AND archived_at IS NOT NULL
            ORDER BY archived_at DESC LIMIT $2 OFFSET $3
            "#,
        )
        .bind(&tenant_id.0)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(rows.into_iter().map(row_to_case).collect())
    }

    pub async fn update_case(
        &self,
        tenant_id: &TenantId,
        case_id: &CaseId,
        title: Option<&str>,
        status: Option<CaseStatus>,
        severity: Option<Severity>,
        evidence: Option<serde_json::Value>,
        external_ref: Option<&str>,
    ) -> Result<Option<Case>> {
        let mut updates = Vec::new();
        let mut param_idx = 3;

        if title.is_some() {
            updates.push(format!("title = ${}", param_idx));
            param_idx += 1;
        }
        if status.is_some() {
            updates.push(format!("status = ${}", param_idx));
            param_idx += 1;
        }
        if severity.is_some() {
            updates.push(format!("severity = ${}", param_idx));
            param_idx += 1;
        }
        if evidence.is_some() {
            updates.push(format!("evidence = ${}", param_idx));
            param_idx += 1;
        }
        if external_ref.is_some() {
            updates.push(format!("external_ref = ${}", param_idx));
            param_idx += 1;
        }

        if updates.is_empty() {
            return self.get_case(tenant_id, case_id).await;
        }

        let query = format!(
            "UPDATE cases SET {}, updated_at = now() WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL
             RETURNING id, tenant_id, session_id, title, status, severity, evidence, external_ref, created_at, updated_at, archived_at",
            updates.join(", ")
        );

        let mut q = sqlx::query(&query)
            .bind(&tenant_id.0)
            .bind(&case_id.0);

        if let Some(t) = title {
            q = q.bind(t);
        }
        if let Some(s) = status {
            q = q.bind(case_status_to_str(s));
        }
        if let Some(sev) = severity {
            q = q.bind(severity_to_str(sev));
        }
        if let Some(e) = &evidence {
            q = q.bind(e);
        }
        if let Some(er) = external_ref {
            q = q.bind(er);
        }

        let row = q
            .fetch_optional(&self.pool)
            .await
            .map_err(NcheError::Database)?;

        Ok(row.map(row_to_case))
    }

    pub async fn archive_case(
        &self,
        tenant_id: &TenantId,
        case_id: &CaseId,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE cases SET archived_at = now(), updated_at = now()
            WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL
            "#,
        )
        .bind(&tenant_id.0)
        .bind(&case_id.0)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn unarchive_case(
        &self,
        tenant_id: &TenantId,
        case_id: &CaseId,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE cases SET archived_at = NULL, updated_at = now()
            WHERE tenant_id = $1 AND id = $2 AND archived_at IS NOT NULL
            "#,
        )
        .bind(&tenant_id.0)
        .bind(&case_id.0)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(result.rows_affected() > 0)
    }
}

fn row_to_case(r: sqlx::postgres::PgRow) -> Case {
    Case {
        id: CaseId::from_string(r.get("id")),
        tenant_id: TenantId::from_string(r.get("tenant_id")),
        session_id: r.get::<Option<String>, _>("session_id").map(SessionId::from_string),
        title: r.get("title"),
        status: str_to_case_status(r.get("status")),
        severity: str_to_severity(r.get("severity")),
        evidence: r.get("evidence"),
        external_ref: r.get("external_ref"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
        archived_at: r.get("archived_at"),
    }
}

fn case_status_to_str(s: CaseStatus) -> &'static str {
    match s {
        CaseStatus::Open => "open",
        CaseStatus::Escalated => "escalated",
        CaseStatus::Resolved => "resolved",
    }
}

fn str_to_case_status(s: &str) -> CaseStatus {
    match s {
        "open" => CaseStatus::Open,
        "escalated" => CaseStatus::Escalated,
        "resolved" => CaseStatus::Resolved,
        _ => CaseStatus::Open,
    }
}

fn severity_to_str(s: Severity) -> &'static str {
    match s {
        Severity::Low => "low",
        Severity::Medium => "medium",
        Severity::High => "high",
        Severity::Critical => "critical",
    }
}

fn str_to_severity(s: &str) -> Severity {
    match s {
        "low" => Severity::Low,
        "medium" => Severity::Medium,
        "high" => Severity::High,
        "critical" => Severity::Critical,
        _ => Severity::Medium,
    }
}
