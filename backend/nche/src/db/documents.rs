use sqlx::Row;
use time::OffsetDateTime;

use crate::domain::{Document, DocumentId, SessionId, TenantId};
use crate::error::{NcheError, Result};

use super::Database;

impl Database {
    pub async fn create_document(
        &self,
        tenant_id: &TenantId,
        session_id: Option<&SessionId>,
        doc_type: &str,
        filename: Option<&str>,
        checksum: Option<&str>,
        storage_uri: Option<&str>,
        tags: Option<serde_json::Value>,
    ) -> Result<Document> {
        let id = DocumentId::new();
        let now = OffsetDateTime::now_utc();
        let tags = tags.unwrap_or_else(|| serde_json::json!([]));

        sqlx::query(
            r#"
            INSERT INTO documents (id, tenant_id, session_id, doc_type, filename, checksum, storage_uri, tags, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(&id.0)
        .bind(&tenant_id.0)
        .bind(session_id.map(|s| &s.0))
        .bind(doc_type)
        .bind(filename)
        .bind(checksum)
        .bind(storage_uri)
        .bind(&tags)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(Document {
            id,
            tenant_id: tenant_id.clone(),
            session_id: session_id.cloned(),
            doc_type: doc_type.to_string(),
            filename: filename.map(String::from),
            checksum: checksum.map(String::from),
            storage_uri: storage_uri.map(String::from),
            tags,
            created_at: now,
            archived_at: None,
        })
    }

    pub async fn get_document(
        &self,
        tenant_id: &TenantId,
        doc_id: &DocumentId,
    ) -> Result<Option<Document>> {
        let row = sqlx::query(
            r#"
            SELECT id, tenant_id, session_id, doc_type, filename, checksum, storage_uri, tags, created_at, archived_at
            FROM documents WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(&tenant_id.0)
        .bind(&doc_id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(row.map(row_to_document))
    }

    pub async fn list_documents(
        &self,
        tenant_id: &TenantId,
        session_id: Option<&SessionId>,
        doc_type: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Document>> {
        let mut query = String::from(
            "SELECT id, tenant_id, session_id, doc_type, filename, checksum, storage_uri, tags, created_at, archived_at
             FROM documents WHERE tenant_id = $1 AND archived_at IS NULL",
        );

        let mut param_idx = 2;

        if session_id.is_some() {
            query.push_str(&format!(" AND session_id = ${}", param_idx));
            param_idx += 1;
        }

        if doc_type.is_some() {
            query.push_str(&format!(" AND doc_type = ${}", param_idx));
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

        if let Some(dt) = doc_type {
            q = q.bind(dt);
        }

        q = q.bind(limit).bind(offset);

        let rows = q.fetch_all(&self.pool).await.map_err(NcheError::Database)?;

        Ok(rows.into_iter().map(row_to_document).collect())
    }

    pub async fn list_archived_documents(
        &self,
        tenant_id: &TenantId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Document>> {
        let rows = sqlx::query(
            r#"
            SELECT id, tenant_id, session_id, doc_type, filename, checksum, storage_uri, tags, created_at, archived_at
            FROM documents WHERE tenant_id = $1 AND archived_at IS NOT NULL
            ORDER BY archived_at DESC LIMIT $2 OFFSET $3
            "#,
        )
        .bind(&tenant_id.0)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(rows.into_iter().map(row_to_document).collect())
    }

    pub async fn archive_document(
        &self,
        tenant_id: &TenantId,
        doc_id: &DocumentId,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE documents SET archived_at = now()
            WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL
            "#,
        )
        .bind(&tenant_id.0)
        .bind(&doc_id.0)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn unarchive_document(
        &self,
        tenant_id: &TenantId,
        doc_id: &DocumentId,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE documents SET archived_at = NULL
            WHERE tenant_id = $1 AND id = $2 AND archived_at IS NOT NULL
            "#,
        )
        .bind(&tenant_id.0)
        .bind(&doc_id.0)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(result.rows_affected() > 0)
    }
}

fn row_to_document(r: sqlx::postgres::PgRow) -> Document {
    Document {
        id: DocumentId::from_string(r.get("id")),
        tenant_id: TenantId::from_string(r.get("tenant_id")),
        session_id: r.get::<Option<String>, _>("session_id").map(SessionId::from_string),
        doc_type: r.get("doc_type"),
        filename: r.get("filename"),
        checksum: r.get("checksum"),
        storage_uri: r.get("storage_uri"),
        tags: r.get("tags"),
        created_at: r.get("created_at"),
        archived_at: r.get("archived_at"),
    }
}
