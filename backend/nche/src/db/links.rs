use sqlx::Row;
use time::OffsetDateTime;

use crate::domain::{Link, LinkId, RecordType, TenantId};
use crate::error::{NcheError, Result};

use super::Database;

impl Database {
    pub async fn create_link(
        &self,
        tenant_id: &TenantId,
        source_type: RecordType,
        source_id: &str,
        target_type: RecordType,
        target_id: &str,
        relation: &str,
    ) -> Result<Link> {
        let id = LinkId::new();
        let now = OffsetDateTime::now_utc();

        sqlx::query(
            r#"
            INSERT INTO links (id, tenant_id, source_type, source_id, target_type, target_id, relation, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(&id.0)
        .bind(&tenant_id.0)
        .bind(record_type_to_str(source_type))
        .bind(source_id)
        .bind(record_type_to_str(target_type))
        .bind(target_id)
        .bind(relation)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(Link {
            id,
            tenant_id: tenant_id.clone(),
            source_type,
            source_id: source_id.to_string(),
            target_type,
            target_id: target_id.to_string(),
            relation: relation.to_string(),
            created_at: now,
            archived_at: None,
        })
    }

    pub async fn get_link(
        &self,
        tenant_id: &TenantId,
        link_id: &LinkId,
    ) -> Result<Option<Link>> {
        let row = sqlx::query(
            r#"
            SELECT id, tenant_id, source_type, source_id, target_type, target_id, relation, created_at, archived_at
            FROM links WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(&tenant_id.0)
        .bind(&link_id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(row.map(row_to_link))
    }

    pub async fn list_links(
        &self,
        tenant_id: &TenantId,
        source_type: Option<RecordType>,
        source_id: Option<&str>,
        target_type: Option<RecordType>,
        target_id: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Link>> {
        let mut query = String::from(
            "SELECT id, tenant_id, source_type, source_id, target_type, target_id, relation, created_at, archived_at
             FROM links WHERE tenant_id = $1 AND archived_at IS NULL",
        );

        let mut param_idx = 2;

        if source_type.is_some() {
            query.push_str(&format!(" AND source_type = ${}", param_idx));
            param_idx += 1;
        }

        if source_id.is_some() {
            query.push_str(&format!(" AND source_id = ${}", param_idx));
            param_idx += 1;
        }

        if target_type.is_some() {
            query.push_str(&format!(" AND target_type = ${}", param_idx));
            param_idx += 1;
        }

        if target_id.is_some() {
            query.push_str(&format!(" AND target_id = ${}", param_idx));
            param_idx += 1;
        }

        query.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut q = sqlx::query(&query).bind(&tenant_id.0);

        if let Some(st) = source_type {
            q = q.bind(record_type_to_str(st));
        }

        if let Some(si) = source_id {
            q = q.bind(si);
        }

        if let Some(tt) = target_type {
            q = q.bind(record_type_to_str(tt));
        }

        if let Some(ti) = target_id {
            q = q.bind(ti);
        }

        q = q.bind(limit).bind(offset);

        let rows = q.fetch_all(&self.pool).await.map_err(NcheError::Database)?;

        Ok(rows.into_iter().map(row_to_link).collect())
    }

    pub async fn list_archived_links(
        &self,
        tenant_id: &TenantId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Link>> {
        let rows = sqlx::query(
            r#"
            SELECT id, tenant_id, source_type, source_id, target_type, target_id, relation, created_at, archived_at
            FROM links WHERE tenant_id = $1 AND archived_at IS NOT NULL
            ORDER BY archived_at DESC LIMIT $2 OFFSET $3
            "#,
        )
        .bind(&tenant_id.0)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(rows.into_iter().map(row_to_link).collect())
    }

    pub async fn archive_link(
        &self,
        tenant_id: &TenantId,
        link_id: &LinkId,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE links SET archived_at = now()
            WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL
            "#,
        )
        .bind(&tenant_id.0)
        .bind(&link_id.0)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn unarchive_link(
        &self,
        tenant_id: &TenantId,
        link_id: &LinkId,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE links SET archived_at = NULL
            WHERE tenant_id = $1 AND id = $2 AND archived_at IS NOT NULL
            "#,
        )
        .bind(&tenant_id.0)
        .bind(&link_id.0)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(result.rows_affected() > 0)
    }
}

fn row_to_link(r: sqlx::postgres::PgRow) -> Link {
    Link {
        id: LinkId::from_string(r.get("id")),
        tenant_id: TenantId::from_string(r.get("tenant_id")),
        source_type: str_to_record_type(r.get("source_type")),
        source_id: r.get("source_id"),
        target_type: str_to_record_type(r.get("target_type")),
        target_id: r.get("target_id"),
        relation: r.get("relation"),
        created_at: r.get("created_at"),
        archived_at: r.get("archived_at"),
    }
}

fn record_type_to_str(t: RecordType) -> &'static str {
    match t {
        RecordType::Action => "action",
        RecordType::Task => "task",
        RecordType::Case => "case",
        RecordType::Document => "document",
        RecordType::Approval => "approval",
    }
}

fn str_to_record_type(s: &str) -> RecordType {
    match s {
        "action" => RecordType::Action,
        "task" => RecordType::Task,
        "case" => RecordType::Case,
        "document" => RecordType::Document,
        "approval" => RecordType::Approval,
        _ => RecordType::Action,
    }
}
