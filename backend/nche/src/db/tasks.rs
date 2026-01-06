use sqlx::Row;
use time::OffsetDateTime;

use crate::domain::{SessionId, Task, TaskId, TaskStatus, TenantId};
use crate::error::{NcheError, Result};

use super::Database;

impl Database {
    pub async fn create_task(
        &self,
        tenant_id: &TenantId,
        session_id: Option<&SessionId>,
        title: &str,
        status: Option<TaskStatus>,
        notes: Option<serde_json::Value>,
    ) -> Result<Task> {
        let id = TaskId::new();
        let now = OffsetDateTime::now_utc();
        let status = status.unwrap_or(TaskStatus::Open);
        let notes = notes.unwrap_or_else(|| serde_json::json!([]));

        sqlx::query(
            r#"
            INSERT INTO tasks (id, tenant_id, session_id, title, status, notes, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $7)
            "#,
        )
        .bind(&id.0)
        .bind(&tenant_id.0)
        .bind(session_id.map(|s| &s.0))
        .bind(title)
        .bind(task_status_to_str(status))
        .bind(&notes)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(Task {
            id,
            tenant_id: tenant_id.clone(),
            session_id: session_id.cloned(),
            title: title.to_string(),
            status,
            notes,
            created_at: now,
            updated_at: now,
            archived_at: None,
        })
    }

    pub async fn get_task(
        &self,
        tenant_id: &TenantId,
        task_id: &TaskId,
    ) -> Result<Option<Task>> {
        let row = sqlx::query(
            r#"
            SELECT id, tenant_id, session_id, title, status, notes, created_at, updated_at, archived_at
            FROM tasks WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(&tenant_id.0)
        .bind(&task_id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(row.map(row_to_task))
    }

    pub async fn list_tasks(
        &self,
        tenant_id: &TenantId,
        session_id: Option<&SessionId>,
        status: Option<TaskStatus>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Task>> {
        let mut query = String::from(
            "SELECT id, tenant_id, session_id, title, status, notes, created_at, updated_at, archived_at
             FROM tasks WHERE tenant_id = $1 AND archived_at IS NULL",
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
            q = q.bind(task_status_to_str(st));
        }

        q = q.bind(limit).bind(offset);

        let rows = q.fetch_all(&self.pool).await.map_err(NcheError::Database)?;

        Ok(rows.into_iter().map(row_to_task).collect())
    }

    pub async fn list_archived_tasks(
        &self,
        tenant_id: &TenantId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Task>> {
        let rows = sqlx::query(
            r#"
            SELECT id, tenant_id, session_id, title, status, notes, created_at, updated_at, archived_at
            FROM tasks WHERE tenant_id = $1 AND archived_at IS NOT NULL
            ORDER BY archived_at DESC LIMIT $2 OFFSET $3
            "#,
        )
        .bind(&tenant_id.0)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(rows.into_iter().map(row_to_task).collect())
    }

    pub async fn update_task(
        &self,
        tenant_id: &TenantId,
        task_id: &TaskId,
        title: Option<&str>,
        status: Option<TaskStatus>,
        notes: Option<serde_json::Value>,
    ) -> Result<Option<Task>> {
        // Build dynamic update query
        let mut updates = Vec::new();
        let mut param_idx = 3; // $1 = tenant_id, $2 = task_id

        if title.is_some() {
            updates.push(format!("title = ${}", param_idx));
            param_idx += 1;
        }
        if status.is_some() {
            updates.push(format!("status = ${}", param_idx));
            param_idx += 1;
        }
        if notes.is_some() {
            updates.push(format!("notes = ${}", param_idx));
            param_idx += 1;
        }

        if updates.is_empty() {
            return self.get_task(tenant_id, task_id).await;
        }

        let query = format!(
            "UPDATE tasks SET {}, updated_at = now() WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL
             RETURNING id, tenant_id, session_id, title, status, notes, created_at, updated_at, archived_at",
            updates.join(", ")
        );

        let mut q = sqlx::query(&query)
            .bind(&tenant_id.0)
            .bind(&task_id.0);

        if let Some(t) = title {
            q = q.bind(t);
        }
        if let Some(s) = status {
            q = q.bind(task_status_to_str(s));
        }
        if let Some(n) = &notes {
            q = q.bind(n);
        }

        let row = q
            .fetch_optional(&self.pool)
            .await
            .map_err(NcheError::Database)?;

        Ok(row.map(row_to_task))
    }

    pub async fn archive_task(
        &self,
        tenant_id: &TenantId,
        task_id: &TaskId,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE tasks SET archived_at = now(), updated_at = now()
            WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL
            "#,
        )
        .bind(&tenant_id.0)
        .bind(&task_id.0)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn unarchive_task(
        &self,
        tenant_id: &TenantId,
        task_id: &TaskId,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE tasks SET archived_at = NULL, updated_at = now()
            WHERE tenant_id = $1 AND id = $2 AND archived_at IS NOT NULL
            "#,
        )
        .bind(&tenant_id.0)
        .bind(&task_id.0)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(result.rows_affected() > 0)
    }
}

fn row_to_task(r: sqlx::postgres::PgRow) -> Task {
    Task {
        id: TaskId::from_string(r.get("id")),
        tenant_id: TenantId::from_string(r.get("tenant_id")),
        session_id: r.get::<Option<String>, _>("session_id").map(SessionId::from_string),
        title: r.get("title"),
        status: str_to_task_status(r.get("status")),
        notes: r.get("notes"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
        archived_at: r.get("archived_at"),
    }
}

fn task_status_to_str(s: TaskStatus) -> &'static str {
    match s {
        TaskStatus::Open => "open",
        TaskStatus::InProgress => "in_progress",
        TaskStatus::Completed => "completed",
    }
}

fn str_to_task_status(s: &str) -> TaskStatus {
    match s {
        "open" => TaskStatus::Open,
        "in_progress" => TaskStatus::InProgress,
        "completed" => TaskStatus::Completed,
        _ => TaskStatus::Open,
    }
}
