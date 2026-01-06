use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use serde::Deserialize;

use super::handlers::PaginatedResponse;
use super::{AgentAuthContext, AppState};
use crate::domain::*;
use crate::error::{NcheError, Result};

// === Task Handlers ===

#[derive(Deserialize)]
pub struct CreateTaskRequest {
    pub session_id: Option<String>,
    pub title: String,
    pub status: Option<TaskStatus>,
    pub notes: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct ListTasksQuery {
    pub session_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn create_task(
    State(state): State<AppState>,
    Extension(auth): Extension<AgentAuthContext>,
    Json(req): Json<CreateTaskRequest>,
) -> Result<impl IntoResponse> {
    let session_id = req.session_id.map(SessionId::from_string);

    let task = state
        .db
        .create_task(
            &auth.tenant_id,
            session_id.as_ref(),
            &req.title,
            req.status,
            req.notes,
        )
        .await?;

    Ok((StatusCode::CREATED, Json(task)))
}

pub async fn get_task(
    State(state): State<AppState>,
    Extension(auth): Extension<AgentAuthContext>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let task_id = TaskId::from_string(id);
    let task = state
        .db
        .get_task(&auth.tenant_id, &task_id)
        .await?
        .ok_or_else(|| NcheError::NotFound {
            entity: "task",
            id: task_id.to_string(),
        })?;

    Ok(Json(task))
}

pub async fn list_tasks(
    State(state): State<AppState>,
    Extension(auth): Extension<AgentAuthContext>,
    Query(query): Query<ListTasksQuery>,
) -> Result<impl IntoResponse> {
    let session_id = query.session_id.map(SessionId::from_string);
    let status = query.status.and_then(|s| match s.as_str() {
        "open" => Some(TaskStatus::Open),
        "in_progress" => Some(TaskStatus::InProgress),
        "completed" => Some(TaskStatus::Completed),
        _ => None,
    });

    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);

    let tasks = state
        .db
        .list_tasks(
            &auth.tenant_id,
            session_id.as_ref(),
            status,
            limit + 1,
            offset,
        )
        .await?;

    Ok(Json(PaginatedResponse::from_items(tasks, limit, offset)))
}

pub async fn list_archived_tasks(
    State(state): State<AppState>,
    Extension(auth): Extension<AgentAuthContext>,
    Query(query): Query<PaginationQuery>,
) -> Result<impl IntoResponse> {
    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);

    let tasks = state
        .db
        .list_archived_tasks(&auth.tenant_id, limit + 1, offset)
        .await?;

    Ok(Json(PaginatedResponse::from_items(tasks, limit, offset)))
}

pub async fn archive_task(
    State(state): State<AppState>,
    Extension(auth): Extension<AgentAuthContext>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let task_id = TaskId::from_string(id);
    let success = state.db.archive_task(&auth.tenant_id, &task_id).await?;

    if !success {
        return Err(NcheError::NotFound {
            entity: "task",
            id: task_id.to_string(),
        });
    }

    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn unarchive_task(
    State(state): State<AppState>,
    Extension(auth): Extension<AgentAuthContext>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let task_id = TaskId::from_string(id);
    let success = state.db.unarchive_task(&auth.tenant_id, &task_id).await?;

    if !success {
        return Err(NcheError::NotFound {
            entity: "task",
            id: task_id.to_string(),
        });
    }

    Ok(Json(serde_json::json!({ "success": true })))
}

// === Case Handlers ===

#[derive(Deserialize)]
pub struct CreateCaseRequest {
    pub session_id: Option<String>,
    pub title: String,
    pub status: Option<CaseStatus>,
    pub severity: Option<Severity>,
    pub evidence: Option<serde_json::Value>,
    pub external_ref: Option<String>,
}

#[derive(Deserialize)]
pub struct ListCasesQuery {
    pub session_id: Option<String>,
    pub status: Option<String>,
    pub severity: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn create_case(
    State(state): State<AppState>,
    Extension(auth): Extension<AgentAuthContext>,
    Json(req): Json<CreateCaseRequest>,
) -> Result<impl IntoResponse> {
    let session_id = req.session_id.map(SessionId::from_string);

    let case = state
        .db
        .create_case(
            &auth.tenant_id,
            session_id.as_ref(),
            &req.title,
            req.status,
            req.severity,
            req.evidence,
            req.external_ref.as_deref(),
        )
        .await?;

    Ok((StatusCode::CREATED, Json(case)))
}

pub async fn get_case(
    State(state): State<AppState>,
    Extension(auth): Extension<AgentAuthContext>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let case_id = CaseId::from_string(id);
    let case = state
        .db
        .get_case(&auth.tenant_id, &case_id)
        .await?
        .ok_or_else(|| NcheError::NotFound {
            entity: "case",
            id: case_id.to_string(),
        })?;

    Ok(Json(case))
}

pub async fn list_cases(
    State(state): State<AppState>,
    Extension(auth): Extension<AgentAuthContext>,
    Query(query): Query<ListCasesQuery>,
) -> Result<impl IntoResponse> {
    let session_id = query.session_id.map(SessionId::from_string);
    let status = query.status.and_then(|s| match s.as_str() {
        "open" => Some(CaseStatus::Open),
        "escalated" => Some(CaseStatus::Escalated),
        "resolved" => Some(CaseStatus::Resolved),
        _ => None,
    });
    let severity = query.severity.and_then(|s| match s.as_str() {
        "low" => Some(Severity::Low),
        "medium" => Some(Severity::Medium),
        "high" => Some(Severity::High),
        "critical" => Some(Severity::Critical),
        _ => None,
    });

    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);

    let cases = state
        .db
        .list_cases(
            &auth.tenant_id,
            session_id.as_ref(),
            status,
            severity,
            limit + 1,
            offset,
        )
        .await?;

    Ok(Json(PaginatedResponse::from_items(cases, limit, offset)))
}

pub async fn list_archived_cases(
    State(state): State<AppState>,
    Extension(auth): Extension<AgentAuthContext>,
    Query(query): Query<PaginationQuery>,
) -> Result<impl IntoResponse> {
    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);

    let cases = state
        .db
        .list_archived_cases(&auth.tenant_id, limit + 1, offset)
        .await?;

    Ok(Json(PaginatedResponse::from_items(cases, limit, offset)))
}

pub async fn archive_case(
    State(state): State<AppState>,
    Extension(auth): Extension<AgentAuthContext>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let case_id = CaseId::from_string(id);
    let success = state.db.archive_case(&auth.tenant_id, &case_id).await?;

    if !success {
        return Err(NcheError::NotFound {
            entity: "case",
            id: case_id.to_string(),
        });
    }

    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn unarchive_case(
    State(state): State<AppState>,
    Extension(auth): Extension<AgentAuthContext>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let case_id = CaseId::from_string(id);
    let success = state.db.unarchive_case(&auth.tenant_id, &case_id).await?;

    if !success {
        return Err(NcheError::NotFound {
            entity: "case",
            id: case_id.to_string(),
        });
    }

    Ok(Json(serde_json::json!({ "success": true })))
}

// === Document Handlers ===

#[derive(Deserialize)]
pub struct CreateDocumentRequest {
    pub session_id: Option<String>,
    pub doc_type: String,
    pub filename: Option<String>,
    pub checksum: Option<String>,
    pub storage_uri: Option<String>,
    pub tags: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct ListDocumentsQuery {
    pub session_id: Option<String>,
    pub doc_type: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn create_document(
    State(state): State<AppState>,
    Extension(auth): Extension<AgentAuthContext>,
    Json(req): Json<CreateDocumentRequest>,
) -> Result<impl IntoResponse> {
    let session_id = req.session_id.map(SessionId::from_string);

    let document = state
        .db
        .create_document(
            &auth.tenant_id,
            session_id.as_ref(),
            &req.doc_type,
            req.filename.as_deref(),
            req.checksum.as_deref(),
            req.storage_uri.as_deref(),
            req.tags,
        )
        .await?;

    Ok((StatusCode::CREATED, Json(document)))
}

pub async fn get_document(
    State(state): State<AppState>,
    Extension(auth): Extension<AgentAuthContext>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let doc_id = DocumentId::from_string(id);
    let document = state
        .db
        .get_document(&auth.tenant_id, &doc_id)
        .await?
        .ok_or_else(|| NcheError::NotFound {
            entity: "document",
            id: doc_id.to_string(),
        })?;

    Ok(Json(document))
}

pub async fn list_documents(
    State(state): State<AppState>,
    Extension(auth): Extension<AgentAuthContext>,
    Query(query): Query<ListDocumentsQuery>,
) -> Result<impl IntoResponse> {
    let session_id = query.session_id.map(SessionId::from_string);

    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);

    let documents = state
        .db
        .list_documents(
            &auth.tenant_id,
            session_id.as_ref(),
            query.doc_type.as_deref(),
            limit + 1,
            offset,
        )
        .await?;

    Ok(Json(PaginatedResponse::from_items(documents, limit, offset)))
}

pub async fn list_archived_documents(
    State(state): State<AppState>,
    Extension(auth): Extension<AgentAuthContext>,
    Query(query): Query<PaginationQuery>,
) -> Result<impl IntoResponse> {
    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);

    let documents = state
        .db
        .list_archived_documents(&auth.tenant_id, limit + 1, offset)
        .await?;

    Ok(Json(PaginatedResponse::from_items(documents, limit, offset)))
}

pub async fn archive_document(
    State(state): State<AppState>,
    Extension(auth): Extension<AgentAuthContext>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let doc_id = DocumentId::from_string(id);
    let success = state.db.archive_document(&auth.tenant_id, &doc_id).await?;

    if !success {
        return Err(NcheError::NotFound {
            entity: "document",
            id: doc_id.to_string(),
        });
    }

    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn unarchive_document(
    State(state): State<AppState>,
    Extension(auth): Extension<AgentAuthContext>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let doc_id = DocumentId::from_string(id);
    let success = state.db.unarchive_document(&auth.tenant_id, &doc_id).await?;

    if !success {
        return Err(NcheError::NotFound {
            entity: "document",
            id: doc_id.to_string(),
        });
    }

    Ok(Json(serde_json::json!({ "success": true })))
}

// === Link Handlers ===

#[derive(Deserialize)]
pub struct CreateLinkRequest {
    pub source_type: RecordType,
    pub source_id: String,
    pub target_type: RecordType,
    pub target_id: String,
    pub relation: String,
}

#[derive(Deserialize)]
pub struct ListLinksQuery {
    pub source_type: Option<String>,
    pub source_id: Option<String>,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn create_link(
    State(state): State<AppState>,
    Extension(auth): Extension<AgentAuthContext>,
    Json(req): Json<CreateLinkRequest>,
) -> Result<impl IntoResponse> {
    let link = state
        .db
        .create_link(
            &auth.tenant_id,
            req.source_type,
            &req.source_id,
            req.target_type,
            &req.target_id,
            &req.relation,
        )
        .await?;

    Ok((StatusCode::CREATED, Json(link)))
}

pub async fn get_link(
    State(state): State<AppState>,
    Extension(auth): Extension<AgentAuthContext>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let link_id = LinkId::from_string(id);
    let link = state
        .db
        .get_link(&auth.tenant_id, &link_id)
        .await?
        .ok_or_else(|| NcheError::NotFound {
            entity: "link",
            id: link_id.to_string(),
        })?;

    Ok(Json(link))
}

fn parse_record_type(s: &str) -> Option<RecordType> {
    match s {
        "action" => Some(RecordType::Action),
        "task" => Some(RecordType::Task),
        "case" => Some(RecordType::Case),
        "document" => Some(RecordType::Document),
        "approval" => Some(RecordType::Approval),
        _ => None,
    }
}

pub async fn list_links(
    State(state): State<AppState>,
    Extension(auth): Extension<AgentAuthContext>,
    Query(query): Query<ListLinksQuery>,
) -> Result<impl IntoResponse> {
    let source_type = query.source_type.as_deref().and_then(parse_record_type);
    let target_type = query.target_type.as_deref().and_then(parse_record_type);

    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);

    let links = state
        .db
        .list_links(
            &auth.tenant_id,
            source_type,
            query.source_id.as_deref(),
            target_type,
            query.target_id.as_deref(),
            limit + 1,
            offset,
        )
        .await?;

    Ok(Json(PaginatedResponse::from_items(links, limit, offset)))
}

pub async fn list_archived_links(
    State(state): State<AppState>,
    Extension(auth): Extension<AgentAuthContext>,
    Query(query): Query<PaginationQuery>,
) -> Result<impl IntoResponse> {
    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);

    let links = state
        .db
        .list_archived_links(&auth.tenant_id, limit + 1, offset)
        .await?;

    Ok(Json(PaginatedResponse::from_items(links, limit, offset)))
}

pub async fn archive_link(
    State(state): State<AppState>,
    Extension(auth): Extension<AgentAuthContext>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let link_id = LinkId::from_string(id);
    let success = state.db.archive_link(&auth.tenant_id, &link_id).await?;

    if !success {
        return Err(NcheError::NotFound {
            entity: "link",
            id: link_id.to_string(),
        });
    }

    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn unarchive_link(
    State(state): State<AppState>,
    Extension(auth): Extension<AgentAuthContext>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let link_id = LinkId::from_string(id);
    let success = state.db.unarchive_link(&auth.tenant_id, &link_id).await?;

    if !success {
        return Err(NcheError::NotFound {
            entity: "link",
            id: link_id.to_string(),
        });
    }

    Ok(Json(serde_json::json!({ "success": true })))
}
