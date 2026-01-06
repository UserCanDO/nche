# NCHE v0.1 Build Specification (Rust)

**Target:** Launch in 2 weeks
**Goal:** Production-ready infrastructure — multi-tenant, webhook-enabled, with approval dashboard

---

## Scope Cutline

### In v0.1 (Launch)

- **Multi-tenancy** from day one
- **Postgres** support
- **Webhooks** for approval notifications
- **Web dashboard** for approvals and audit log
- Two tools: `send_email`, `http_request`
- API key auth (tenant-scoped)
- Hardcoded policy rules (code, not config)
- Single binary with subcommands (`nche serve`, `nche approvals`)
- Docker Compose with Postgres

### Explicitly NOT in v0.1

- Policy DSL or external policy engine (OPA, Cedar)
- Role-based access control (beyond tenant isolation)
- Additional tools (Slack, calendar, etc.)
- Real-time WebSocket updates
- MCP integration
- OpenTelemetry

---

## Architecture Overview

```text
┌─────────────────────────────────────────────────────────────────┐
│                          Tenants                                │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │  Tenant A   │  │  Tenant B   │  │  Tenant C   │             │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘             │
└─────────┼────────────────┼────────────────┼─────────────────────┘
          │                │                │
          ▼                ▼                ▼
┌─────────────────────────────────────────────────────────────────┐
│                        AI Agents                                │
│         (each agent belongs to exactly one tenant)              │
└─────────────────────────┬───────────────────────────────────────┘
                          │ HTTP + API Key
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Agent Control Plane                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐        │
│  │  Policy  │  │ Approval │  │   Tool   │  │  Audit   │        │
│  │  Engine  │  │  Queue   │  │ Executor │  │   Log    │        │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘        │
└─────────────────────────┬───────────────────────────────────────┘
                          │
          ┌───────────────┼───────────────┐
          ▼               ▼               ▼
    ┌──────────┐   ┌──────────┐   ┌──────────┐
    │ Postgres │   │ Webhooks │   │  Tools   │
    │    DB    │   │ (notify) │   │ (execute)│
    └──────────┘   └──────────┘   └──────────┘
```

---

## Data Model

### Core Tables (Multi-tenant)

```sql
-- migrations/001_initial.sql

-- Tenants are the top-level isolation boundary
CREATE TABLE tenants (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    webhook_url TEXT,
    webhook_secret TEXT,
    webhook_events TEXT,  -- JSON array of event types
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Agents belong to a tenant
CREATE TABLE agents (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    name TEXT NOT NULL,
    api_key_hash TEXT NOT NULL,
    api_key_prefix TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(tenant_id, name)
);

-- Sessions scope agent runs
CREATE TABLE sessions (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    agent_id TEXT NOT NULL REFERENCES agents(id),
    actor_id TEXT NOT NULL,
    actor_type TEXT NOT NULL CHECK (actor_type IN ('user', 'org', 'system')),
    autonomy_level TEXT NOT NULL CHECK (autonomy_level IN ('full', 'supervised', 'restricted')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ended_at TIMESTAMPTZ
);

-- Actions are the core unit
CREATE TABLE actions (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    session_id TEXT NOT NULL REFERENCES sessions(id),
    tool TEXT NOT NULL,
    params JSONB NOT NULL,
    state TEXT NOT NULL CHECK (state IN (
        'proposed', 'paused_for_approval', 'ready_to_execute', 
        'executing', 'executed', 'denied', 'failed'
    )),
    policy_result TEXT CHECK (policy_result IN ('allow', 'deny', 'require_approval')),
    policy_reason TEXT,
    result JSONB,
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Approvals link to actions (one per action in v0.1)
CREATE TABLE approvals (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    action_id TEXT NOT NULL REFERENCES actions(id),
    status TEXT NOT NULL CHECK (status IN ('pending', 'approved', 'denied')),
    approver_id TEXT,
    approver_note TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    decided_at TIMESTAMPTZ,
    UNIQUE(action_id)  -- One approval per action in v0.1
);

-- Immutable audit log
CREATE TABLE events (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    session_id TEXT REFERENCES sessions(id),  -- NULL for session-level events
    action_id TEXT REFERENCES actions(id),    -- NULL for session-level events
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Performance indexes for audit queries
CREATE INDEX idx_events_tenant_created ON events(tenant_id, created_at DESC);
CREATE INDEX idx_events_action_timeline ON events(action_id, created_at ASC);

-- Webhook delivery log (for retries and debugging)
CREATE TABLE webhook_deliveries (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'delivered', 'failed')),
    attempts INTEGER NOT NULL DEFAULT 0,
    last_attempt_at TIMESTAMPTZ,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_error TEXT,
    attempt_metadata JSONB NOT NULL DEFAULT '[]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- NCHE-native records: Tasks
CREATE TABLE tasks (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    session_id TEXT REFERENCES sessions(id),
    title TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'in_progress', 'completed')),
    notes JSONB DEFAULT '[]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- NCHE-native records: Cases
CREATE TABLE cases (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    session_id TEXT REFERENCES sessions(id),
    title TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'escalated', 'resolved')),
    severity TEXT NOT NULL DEFAULT 'medium' CHECK (severity IN ('low', 'medium', 'high', 'critical')),
    evidence JSONB DEFAULT '[]',
    external_ref TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- NCHE-native records: Documents (metadata only)
CREATE TABLE documents (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    session_id TEXT REFERENCES sessions(id),
    doc_type TEXT NOT NULL,
    filename TEXT,
    checksum TEXT,
    storage_uri TEXT,
    tags JSONB DEFAULT '[]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- NCHE-native records: Links
CREATE TABLE links (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    source_type TEXT NOT NULL CHECK (source_type IN ('action', 'task', 'case', 'document')),
    source_id TEXT NOT NULL,
    target_type TEXT NOT NULL CHECK (target_type IN ('action', 'task', 'case', 'document', 'approval')),
    target_id TEXT NOT NULL,
    relation TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Dashboard users (for web UI authentication)
CREATE TABLE dashboard_users (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    email TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    name TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(tenant_id, email)
);

-- Dashboard sessions
CREATE TABLE dashboard_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES dashboard_users(id),
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Performance indexes
CREATE INDEX idx_agents_tenant ON agents(tenant_id);
CREATE INDEX idx_sessions_tenant ON sessions(tenant_id);
CREATE INDEX idx_sessions_agent ON sessions(agent_id);
CREATE INDEX idx_actions_tenant ON actions(tenant_id);
CREATE INDEX idx_actions_session ON actions(session_id);
CREATE INDEX idx_actions_state_created ON actions(state, created_at);
CREATE INDEX idx_approvals_tenant ON approvals(tenant_id);
CREATE INDEX idx_approvals_action ON approvals(action_id);
CREATE INDEX idx_webhook_deliveries_tenant ON webhook_deliveries(tenant_id);
CREATE INDEX idx_webhook_deliveries_next_attempt ON webhook_deliveries(next_attempt_at) WHERE status = 'pending';
CREATE INDEX idx_events_tenant_created ON events(tenant_id, created_at DESC);
CREATE INDEX idx_events_action_timeline ON events(action_id, created_at ASC);
CREATE INDEX idx_tasks_tenant ON tasks(tenant_id);
CREATE INDEX idx_cases_tenant ON cases(tenant_id);
CREATE INDEX idx_documents_tenant ON documents(tenant_id);
CREATE INDEX idx_links_tenant ON links(tenant_id);
CREATE INDEX idx_dashboard_users_tenant ON dashboard_users(tenant_id);

-- Updated_at triggers (optional - can also be handled in application code)
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_tenants_updated_at BEFORE UPDATE ON tenants
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_actions_updated_at BEFORE UPDATE ON actions
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_tasks_updated_at BEFORE UPDATE ON tasks
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_cases_updated_at BEFORE UPDATE ON cases
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_dashboard_sessions_updated_at BEFORE UPDATE ON dashboard_sessions
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
```


## Domain Types

```rust
// src/domain/mod.rs

use serde::{Deserialize, Serialize};

// === Identifiers (newtypes for type safety) ===

macro_rules! define_id {
    ($name:ident, $prefix:expr) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
        #[sqlx(transparent)]
        pub struct $name(pub String);

        impl $name {
            pub fn new() -> Self {
                Self(format!("{}_{}", $prefix, nanoid::nanoid!(12)))
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

define_id!(TenantId, "ten");
define_id!(AgentId, "agent");
define_id!(SessionId, "sess");
define_id!(ActionId, "act");
define_id!(ApprovalId, "appr");
define_id!(EventId, "evt");
define_id!(TaskId, "task");
define_id!(CaseId, "case");
define_id!(DocumentId, "doc");
define_id!(LinkId, "link");
define_id!(WebhookDeliveryId, "whd");
define_id!(DashboardUserId, "user");
define_id!(DashboardSessionId, "dsess");

// === Enums ===

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ActorType {
    User,
    Org,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum AutonomyLevel {
    Full,
    Supervised,
    Restricted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ActionState {
    Proposed,
    PausedForApproval,
    ReadyToExecute,
    Executing,
    Executed,
    Denied,
    Failed,
}

impl ActionState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Executed | Self::Denied | Self::Failed)
    }
    
    pub fn is_pending_approval(&self) -> bool {
        matches!(self, Self::PausedForApproval)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum PolicyResult {
    Allow,
    Deny,
    RequireApproval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Denied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Open,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum CaseStatus {
    Open,
    Escalated,
    Resolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum RecordType {
    Action,
    Task,
    Case,
    Document,
    Approval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum WebhookDeliveryStatus {
    Pending,
    Delivered,
    Failed,
}

// === Tool Names ===

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolName {
    SendEmail,
    HttpRequest,
}

impl std::fmt::Display for ToolName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SendEmail => write!(f, "send_email"),
            Self::HttpRequest => write!(f, "http_request"),
        }
    }
}

impl std::str::FromStr for ToolName {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "send_email" => Ok(Self::SendEmail),
            "http_request" => Ok(Self::HttpRequest),
            _ => Err(format!("Unknown tool: {}", s)),
        }
    }
}

// === Webhook Events ===

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookEventType {
    ApprovalRequired,
    ActionApproved,
    ActionDenied,
    ActionExecuted,
    ActionFailed,
}

impl std::fmt::Display for WebhookEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ApprovalRequired => write!(f, "approval_required"),
            Self::ActionApproved => write!(f, "action_approved"),
            Self::ActionDenied => write!(f, "action_denied"),
            Self::ActionExecuted => write!(f, "action_executed"),
            Self::ActionFailed => write!(f, "action_failed"),
        }
    }
}
```

---

## State Machine

```text
                    ┌──────────────┐
                    │   Proposed   │
                    └──────┬───────┘
                           │
                    ┌──────▼───────┐
                    │    Policy    │
                    │  Evaluation  │
                    └──────┬───────┘
                           │
          ┌────────────────┼────────────────┐
          │                │                │
          ▼                ▼                ▼
    ┌─────────┐    ┌──────────────┐   ┌───────────┐
    │ Denied  │    │  PausedFor   │   │  ReadyTo  │
    │         │    │  Approval    │   │  Execute  │
    └─────────┘    └──────┬───────┘   └─────┬─────┘
                          │                 │
               ┌──────────┴──────────┐      │
               │                     │      │
               ▼                     ▼      │
         ┌─────────┐           ┌───────────┐│
         │ Denied  │           │  ReadyTo  │◄┘
         │         │           │  Execute  │
         └─────────┘           └─────┬─────┘
                                     │
                                     ▼
                               ┌───────────┐
                               │ Executing │
                               └─────┬─────┘
                                     │
                          ┌──────────┴──────────┐
                          │                     │
                          ▼                     ▼
                    ┌──────────┐          ┌─────────┐
                    │ Executed │          │ Failed  │
                    └──────────┘          └─────────┘
```

### State Transitions (enforced in code)

```rust
// src/domain/state_machine.rs

use crate::domain::{ActionState, PolicyResult};
use crate::error::{AcpError, Result};

impl ActionState {
    pub fn apply_policy(self, result: PolicyResult) -> Result<Self> {
        match (self, result) {
            (Self::Proposed, PolicyResult::Allow) => Ok(Self::ReadyToExecute),
            (Self::Proposed, PolicyResult::Deny) => Ok(Self::Denied),
            (Self::Proposed, PolicyResult::RequireApproval) => Ok(Self::PausedForApproval),
            _ => Err(AcpError::InvalidStateTransition {
                from: self,
                action: "apply_policy".into(),
            }),
        }
    }

    pub fn apply_approval(self, approved: bool) -> Result<Self> {
        match (self, approved) {
            (Self::PausedForApproval, true) => Ok(Self::ReadyToExecute),
            (Self::PausedForApproval, false) => Ok(Self::Denied),
            _ => Err(AcpError::InvalidStateTransition {
                from: self,
                action: "apply_approval".into(),
            }),
        }
    }

    pub fn begin_execution(self) -> Result<Self> {
        match self {
            Self::ReadyToExecute => Ok(Self::Executing),
            _ => Err(AcpError::InvalidStateTransition {
                from: self,
                action: "begin_execution".into(),
            }),
        }
    }

    pub fn complete_execution(self, success: bool) -> Result<Self> {
        match (self, success) {
            (Self::Executing, true) => Ok(Self::Executed),
            (Self::Executing, false) => Ok(Self::Failed),
            _ => Err(AcpError::InvalidStateTransition {
                from: self,
                action: "complete_execution".into(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions() {
        // Proposed -> Allow -> ReadyToExecute
        let state = ActionState::Proposed;
        let state = state.apply_policy(PolicyResult::Allow).unwrap();
        assert_eq!(state, ActionState::ReadyToExecute);

        // ReadyToExecute -> Executing
        let state = state.begin_execution().unwrap();
        assert_eq!(state, ActionState::Executing);

        // Executing -> Executed
        let state = state.complete_execution(true).unwrap();
        assert_eq!(state, ActionState::Executed);
        assert!(state.is_terminal());
    }

    #[test]
    fn test_approval_flow() {
        let state = ActionState::Proposed;
        let state = state.apply_policy(PolicyResult::RequireApproval).unwrap();
        assert_eq!(state, ActionState::PausedForApproval);

        // Approve
        let state = state.apply_approval(true).unwrap();
        assert_eq!(state, ActionState::ReadyToExecute);
    }

    #[test]
    fn test_invalid_transition() {
        let state = ActionState::Executed;
        let result = state.begin_execution();
        assert!(result.is_err());
    }
}
```

---

## Database Layer

### Tenant-Scoped Access Pattern

**CRITICAL: All DB methods must be tenant-scoped in v0.1**

```rust
// src/db/postgres.rs

impl Database {
    // ✅ CORRECT: Always require tenant_id
    pub async fn get_action(&self, tenant_id: &TenantId, action_id: &ActionId) -> Result<Option<Action>> {
        sqlx::query_as!(
            Action,
            "SELECT * FROM actions WHERE tenant_id = $1 AND id = $2",
            tenant_id.0,
            action_id.0
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AcpError::Database)
    }
    
    pub async fn list_actions(
        &self, 
        tenant_id: &TenantId, 
        state: Option<ActionState>,
        limit: i64,
        cursor: Option<&str>
    ) -> Result<Vec<Action>> {
        let mut query = "SELECT * FROM actions WHERE tenant_id = $1".to_string();
        let mut binds = vec![tenant_id.0.clone()];
        
        if let Some(state) = state {
            query.push_str(" AND state = $2");
            binds.push(state.to_string());
        }
        
        query.push_str(" ORDER BY created_at DESC LIMIT $");
        query.push_str(&(binds.len() + 1).to_string());
        
        // Execute with proper parameter binding...
    }
    
    // ❌ NEVER expose unscoped methods like this:
    // pub async fn get_action_unscoped(&self, action_id: &ActionId) -> Result<Option<Action>>
    
    // ✅ Dashboard methods also tenant-scoped via user's tenant_id
    pub async fn list_approvals(
        &self,
        tenant_id: &TenantId,
        status: ApprovalStatus,
        limit: i64
    ) -> Result<Vec<Approval>> {
        sqlx::query_as!(
            Approval,
            "SELECT a.* FROM approvals a 
             JOIN actions act ON a.action_id = act.id 
             WHERE a.tenant_id = $1 AND a.status = $2 
             ORDER BY a.created_at DESC LIMIT $3",
            tenant_id.0,
            status.to_string(),
            limit
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AcpError::Database)
    }
}

// Handler pattern - always extract tenant_id from auth context
pub async fn get_action(
    State(state): State<AppState>,
    Path(action_id): Path<ActionId>,
) -> Result<Json<Action>> {
    let auth_ctx = extract_auth_context(&request)?;
    
    let action = state.db
        .get_action(&auth_ctx.tenant_id, &action_id)
        .await?
        .ok_or_else(|| AcpError::NotFound {
            entity: "action",
            id: action_id.to_string(),
        })?;
    
    Ok(Json(action))
}
```

### Database Connection Setup

```rust
// src/db/postgres.rs

use sqlx::postgres::PgPoolOptions;
use crate::domain::*;
use crate::error::{AcpError, Result};

pub struct Database {
    pool: sqlx::PgPool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            .map_err(AcpError::Database)?;
            
        Ok(Self { pool })
    }
    
    // Run migrations
    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("./migrations/postgres")
            .run(&self.pool)
            .await
            .map_err(AcpError::Database)
    }
}
```

```rust
// src/error.rs

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use crate::domain::ActionState;

#[derive(Debug, thiserror::Error)]
pub enum AcpError {
    #[error("Not found: {entity} with id {id}")]
    NotFound { entity: &'static str, id: String },

    #[error("Invalid state transition from {from:?}: {action}")]
    InvalidStateTransition { from: ActionState, action: String },

    #[error("Unauthorized: {message}")]
    Unauthorized { message: String },

    #[error("Forbidden: {message}")]
    Forbidden { message: String },

    #[error("Bad request: {message}")]
    BadRequest { message: String },

    #[error("Conflict: {message}")]
    Conflict { message: String },

    #[error("Tool execution failed: {message}")]
    ToolExecution { message: String },

    #[error("Webhook delivery failed: {message}")]
    WebhookDelivery { message: String },

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, AcpError>;

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    code: &'static str,
}

impl IntoResponse for AcpError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            Self::NotFound { .. } => (StatusCode::NOT_FOUND, "not_found"),
            Self::InvalidStateTransition { .. } => (StatusCode::CONFLICT, "invalid_state"),
            Self::Unauthorized { .. } => (StatusCode::UNAUTHORIZED, "unauthorized"),
            Self::Forbidden { .. } => (StatusCode::FORBIDDEN, "forbidden"),
            Self::BadRequest { .. } => (StatusCode::BAD_REQUEST, "bad_request"),
            Self::Conflict { .. } => (StatusCode::CONFLICT, "conflict"),
            Self::ToolExecution { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "tool_error"),
            Self::WebhookDelivery { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "webhook_error"),
            Self::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "database_error"),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
        };

        let body = Json(ErrorResponse {
            error: self.to_string(),
            code,
        });

        (status, body).into_response()
    }
}
```

---

## Multi-tenant Auth

```rust
// src/api/auth.rs

use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::Response,
};
use crate::{
    domain::{AgentId, TenantId},
    error::{AcpError, Result},
    AppState,
};

// API key format: nche_live_<agent_id>_<random>
// Example: nche_live_agent_abc123_7fK9xM2pQ4rS

#[derive(Debug, Clone)]
pub struct ApiKey {
    pub prefix: String,      // "nche_live"
    pub agent_id: AgentId,   // "agent_abc123" 
    pub secret: String,      // "7fK9xM2pQ4rS"
}

impl ApiKey {
    pub fn new(agent_id: AgentId) -> Self {
        Self {
            prefix: "nche_live".to_string(),
            agent_id: agent_id.clone(),
            secret: nanoid::nanoid!(16),
        }
    }
    
    pub fn to_string(&self) -> String {
        format!("nche_live_{}_{}", self.agent_id.0, self.secret)
    }
    
    pub fn parse(key: &str) -> Result<Self> {
        let parts: Vec<&str> = key.split('_').collect();
        if parts.len() != 4 || parts[0] != "nche" || parts[1] != "live" {
            return Err(AcpError::Unauthorized {
                message: "Invalid API key format".into(),
            });
        }
        
        Ok(Self {
            prefix: format!("{}_{}", parts[0], parts[1]),
            agent_id: AgentId(parts[2].to_string()),
            secret: parts[3].to_string(),
        })
    }
    
    pub fn prefix(&self) -> String {
        format!("nche_live_{}", self.agent_id.0)
    }
}

// Updated Database methods
impl Database {
    // Fast lookup by prefix first, then verify hash
    pub async fn get_agent_by_api_key(&self, api_key: &str) -> Result<Option<(Agent, TenantId)>> {
        let key = ApiKey::parse(api_key)?;
        
        // Direct lookup by agent_id (primary key)
        let agent = sqlx::query_as!(
            Agent,
            "SELECT id, tenant_id, name, api_key_hash, api_key_prefix, created_at 
             FROM agents WHERE id = $1",
            key.agent_id.0
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AcpError::Database)?;
        
        if let Some(agent) = agent {
            // Verify prefix matches (quick check)
            if agent.api_key_prefix != key.prefix() {
                return Ok(None);
            }
            
            // Verify secret hash
            let stored_hash = agent.api_key_hash;
            let computed_hash = argon2::hash_encoded(
                key.secret.as_bytes(),
                &self.server_salt,
                &argon2::Config::default(),
            ).map_err(|e| AcpError::Internal(format!("Hash error: {}", e)))?;
            
            if argon2::verify_encoded(&stored_hash, key.secret.as_bytes())
                .map_err(|e| AcpError::Internal(format!("Verify error: {}", e)))? 
            {
                return Ok(Some((agent, agent.tenant_id)));
            }
        }
        
        Ok(None)
    }
    
    // Generate API key for new agent
    pub async fn create_agent_with_key(&self, tenant_id: &TenantId, name: &str) -> Result<(Agent, String)> {
        let agent_id = AgentId::new();
        let api_key = ApiKey::new(agent_id.clone());
        
        // Hash the secret part only
        let api_key_hash = argon2::hash_encoded(
            api_key.secret.as_bytes(),
            &self.server_salt,
            &argon2::Config::default(),
        ).map_err(|e| AcpError::Internal(format!("Hash error: {}", e)))?;
        
        let agent = Agent {
            id: agent_id,
            tenant_id: tenant_id.clone(),
            name: name.to_string(),
            api_key_hash,
            api_key_prefix: api_key.prefix(),
            created_at: time::OffsetDateTime::now_utc(),
        };
        
        sqlx::query!(
            "INSERT INTO agents (id, tenant_id, name, api_key_hash, api_key_prefix, created_at) 
             VALUES ($1, $2, $3, $4, $5, $6)",
            agent.id.0,
            agent.tenant_id.0,
            agent.name,
            agent.api_key_hash,
            agent.api_key_prefix,
            agent.created_at
        )
        .execute(&self.pool)
        .await
        .map_err(AcpError::Database)?;
        
        Ok((agent, api_key.to_string()))
    }
}

/// Dashboard authentication with secure cookies
pub async fn dashboard_login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse> {
    // Verify credentials...
    let (user, tenant_id) = verify_credentials(&state.db, &payload.email, &payload.password).await?;
    
    // Create secure session
    let session_id = DashboardSessionId::new();
    let expires_at = time::OffsetDateTime::now_utc() + time::Duration::hours(8);
    
    state.db.create_dashboard_session(
        &session_id,
        &user.id,
        &tenant_id,
        expires_at,
    ).await?;
    
    // Set secure cookie with proper flags
    let cookie = format!(
        "nche_session={}; HttpOnly; Secure; SameSite=Lax; Path=/; Expires={}",
        session_id.0,
        expires_at.format(&time::format_description::well_known::Rfc3339)
    );
    
    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        cookie.parse().unwrap(),
    );
    
    Ok((headers, Json(LoginResponse { success: true })))
}

/// CSRF protection for mutations
pub async fn csrf_protected_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response> {
    // For mutations (POST, PATCH, DELETE), require X-Session-Id header
    if matches!(request.method(), Method::POST | Method::PATCH | Method::DELETE) {
        let session_header = request
            .headers()
            .get("X-Session-Id")
            .and_then(|h| h.to_str().ok());
            
        let cookie_session = request
            .headers()
            .get(header::COOKIE)
            .and_then(|h| h.to_str().ok())
            .and_then(|cookies| {
                cookies.split(';')
                    .find_map(|c| c.trim().strip_prefix("nche_session="))
            });
            
        // Both must match for mutations
        match (session_header, cookie_session) {
            (Some(header), Some(cookie)) if header == cookie => {
                // Valid CSRF protection
            }
            _ => {
                return Err(AcpError::Unauthorized {
                    message: "CSRF protection failed".into(),
                });
            }
        }
    }
    
    Ok(next.run(request).await)
}
```

/// Action executor - polls for ReadyToExecute actions and runs tools
pub struct ActionExecutor {
    db: Arc<Database>,
    tool_registry: Arc<ToolRegistry>,
    webhook_tx: mpsc::Sender<WebhookJob>,
}

impl ActionExecutor {
    pub fn new(
        db: Arc<Database>,
        tool_registry: Arc<ToolRegistry>,
        webhook_tx: mpsc::Sender<WebhookJob>,
    ) -> Self {
        Self { db, tool_registry, webhook_tx }
    }
    
    pub async fn run(self) {
        info!("Action executor started");
        
        let mut interval = tokio::time::interval(Duration::from_secs(2));
        
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = self.process_ready_actions().await {
                        error!("Executor error: {}", e);
                    }
                }
            }
        }
    }
    
    async fn process_ready_actions(&self) -> Result<()> {
        // Atomic: lock ready actions and set to executing
        let actions = self.db.lock_ready_actions(10).await?;
        
        for action in actions {
            if let Err(e) = self.execute_action(action).await {
                error!("Action execution failed: {}", e);
            }
        }
        
        Ok(())
    }
    
    async fn execute_action(&self, action: Action) -> Result<()> {
        info!("Executing action: {}", action.id);
        
        // Get tool
        let tool = self.tool_registry.get(&action.tool)
            .ok_or_else(|| AcpError::ToolExecution {
                message: format!("Unknown tool: {}", action.tool),
            })?;
        
        // Execute tool
        let result = tool.execute(action.params.clone()).await;
        
        // Update action state and record result
        let final_state = if result.success {
            ActionState::Executed
        } else {
            ActionState::Failed
        };
        
        // Update in database
        self.db.complete_action_execution(
            &action.id,
            final_state,
            Some(serde_json::to_value(result)?),
        ).await?;
        
        // Send webhook if needed
        if final_state == ActionState::Executed {
            if let Err(e) = queue_webhook(
                &self.db,
                &self.webhook_tx,
                &action.tenant_id,
                WebhookEventType::ActionExecuted,
                serde_json::json!({
                    "action_id": action.id,
                    "tool": action.tool,
                    "result": result
                })
            ).await {
                error!("Failed to queue webhook: {}", e);
            }
        }
        
        Ok(())
    }
}

// Database methods for executor
impl Database {
    // Lock ready actions atomically
    pub async fn lock_ready_actions(&self, limit: i64) -> Result<Vec<Action>> {
        sqlx::query_as!(
            Action,
            "UPDATE actions 
             SET state = 'executing', updated_at = now()
             WHERE ctid IN (
                 SELECT ctid FROM actions 
                 WHERE state = 'ready_to_execute' 
                 ORDER BY created_at ASC 
                 LIMIT $1
                 FOR UPDATE SKIP LOCKED
             )
             RETURNING *",
            limit
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AcpError::Database)
    }
    
    // Complete action execution
    pub async fn complete_action_execution(
        &self,
        action_id: &ActionId,
        state: ActionState,
        result: Option<Value>,
    ) -> Result<()> {
        sqlx::query!(
            "UPDATE actions 
             SET state = $1, result = $2, updated_at = now()
             WHERE id = $3",
            state.to_string(),
            result,
            action_id.0
        )
        .execute(&self.pool)
        .await
        .map_err(AcpError::Database)?;
        
        // Record execution event
        self.create_event(
            &EventId::new(),
            &action_id.tenant_id, // Need tenant_id from action
            action_id,
            &format!("action.{}", match state {
                ActionState::Executed => "executed",
                ActionState::Failed => "failed",
                _ => "unknown"
            }),
            serde_json::json!({
                "state": state.to_string(),
                "result": result
            })
        ).await
    }
}

```rust
// src/webhooks/mod.rs

use crate::domain::{TenantId, WebhookDeliveryId, WebhookDeliveryStatus, WebhookEventType};
use crate::db::Database;
use crate::error::Result;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Serialize)]
pub struct WebhookPayload {
    pub event: WebhookEventType,
    pub timestamp: String,
    pub data: Value,
}

#[derive(Debug, Clone)]
pub struct WebhookConfig {
    pub url: String,
    pub secret: String,
    pub events: Vec<WebhookEventType>,
}

/// Webhook dispatcher - runs in background, processes delivery queue
pub struct WebhookDispatcher {
    db: Arc<Database>,
    client: reqwest::Client,
    rx: mpsc::Receiver<WebhookJob>,
}

#[derive(Debug)]
pub struct WebhookJob {
    pub tenant_id: TenantId,
    pub delivery_id: WebhookDeliveryId,
}

impl WebhookDispatcher {
    pub fn new(db: Arc<Database>) -> (Self, mpsc::Sender<WebhookJob>) {
        let (tx, rx) = mpsc::channel(1000);
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to build HTTP client");

        (Self { db, client, rx }, tx)
    }
    
    pub async fn run(mut self) {
        info!("Webhook dispatcher started");
        
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        
        loop {
            tokio::select! {
                Some(job) = self.rx.recv() => {
                    if let Err(e) = self.process_job(job).await {
                        error!("Webhook delivery failed: {}", e);
                    }
                }
                _ = interval.tick() => {
                    // Process retries
                    if let Err(e) = self.process_retry_jobs().await {
                        error!("Webhook retry processing failed: {}", e);
                    }
                }
            }
        }
    }
    
    async fn process_retry_jobs(&self) -> Result<()> {
        // Get webhook deliveries ready for retry
        let retry_jobs = self.db.get_ready_webhook_deliveries().await?;
        
        for job in retry_jobs {
            if let Err(e) = self.process_job(job).await {
                error!("Webhook retry failed: {}", e);
            }
        }
        
        Ok(())
    }
    
    async fn process_job(&self, job: WebhookJob) -> Result<()> {
        let delivery = self.db.get_webhook_delivery(&job.delivery_id).await?
            .ok_or_else(|| crate::error::AcpError::NotFound {
                entity: "webhook_delivery",
                id: job.delivery_id.to_string(),
            })?;

        let tenant = self.db.get_tenant(&job.tenant_id).await?
            .ok_or_else(|| crate::error::AcpError::NotFound {
                entity: "tenant",
                id: job.tenant_id.to_string(),
            })?;

        let Some(webhook_url) = &tenant.webhook_url else {
            // No webhook configured, mark as delivered
            self.db.update_webhook_delivery_status(
                &job.delivery_id,
                WebhookDeliveryStatus::Delivered,
                None,
            ).await?;
            return Ok(());
        };

        let Some(webhook_secret) = &tenant.webhook_secret else {
            warn!("Webhook URL configured but no secret for tenant {}", job.tenant_id);
            return Ok(());
        };

        // Record attempt start
        let attempt_start = time::OffsetDateTime::now_utc();
        
        // Sign the payload - use consistent JSON serialization
        let payload_json = serde_json::to_string(&delivery.payload)?;
        let signature = sign_payload(&payload_json, webhook_secret);

        // Attempt delivery with consistent event type header
        let result = self.client
            .post(webhook_url)
            .header("Content-Type", "application/json")
            .header("X-NCHE-Signature", format!("sha256={}", signature))
            .header("X-NCHE-Event", delivery.event_type.clone()) // Uses snake_case from enum
            .body(payload_json)
            .send()
            .await;

        let attempt_end = time::OffsetDateTime::now_utc();
        let duration_ms = (attempt_end - attempt_start).whole_milliseconds() as u64;

        match result {
            Ok(response) if response.status().is_success() => {
                // Success - mark as delivered
                self.db.update_webhook_delivery_status(
                    &job.delivery_id,
                    WebhookDeliveryStatus::Delivered,
                    None,
                ).await?;
                
                // Record successful attempt
                self.db.add_webhook_attempt(
                    &job.delivery_id,
                    response.status().as_u16(),
                    duration_ms,
                    None,
                ).await?;
                
                info!("Webhook delivered: {} -> {}", job.delivery_id, webhook_url);
            }
            Ok(response) => {
                let error = format!("HTTP {}", response.status());
                let should_retry = delivery.attempts < 10 && !response.status().is_client_error();
                
                self.update_delivery_with_retry(
                    &job.delivery_id,
                    WebhookDeliveryStatus::Failed,
                    &error,
                    response.status().as_u16(),
                    duration_ms,
                    should_retry,
                ).await?;
                
                warn!("Webhook failed: {} -> {}: {}", job.delivery_id, webhook_url, error);
            }
            Err(e) => {
                let error = e.to_string();
                let should_retry = delivery.attempts < 10;
                
                self.update_delivery_with_retry(
                    &job.delivery_id,
                    WebhookDeliveryStatus::Failed,
                    &error,
                    0, // No HTTP status
                    duration_ms,
                    should_retry,
                ).await?;
                
                warn!("Webhook failed: {} -> {}: {}", job.delivery_id, webhook_url, error);
            }
        }

        Ok(())
    }
    
    async fn update_delivery_with_retry(
        &self,
        delivery_id: &WebhookDeliveryId,
        status: WebhookDeliveryStatus,
        error: &str,
        http_status: u16,
        duration_ms: u64,
        should_retry: bool,
    ) -> Result<()> {
        // Record attempt
        self.db.add_webhook_attempt(delivery_id, http_status, duration_ms, Some(error)).await?;
        
        if should_retry {
            // Calculate next attempt time with exponential backoff
            let next_attempt = self.calculate_next_attempt(self.db.get_webhook_delivery(delivery_id).await?.unwrap().attempts);
            
            self.db.update_webhook_delivery_status(
                delivery_id,
                status,
                Some(error),
            ).await?;
            
            self.db.update_webhook_next_attempt(delivery_id, next_attempt).await?;
        } else {
            // Final failure
            self.db.update_webhook_delivery_status(
                delivery_id,
                WebhookDeliveryStatus::Failed,
                Some(error),
            ).await?;
        }
        
        Ok(())
    }
    
    fn calculate_next_attempt(&self, attempts: i32) -> time::OffsetDateTime {
        let base_delay = std::time::Duration::from_secs(10);
        let multiplier = 2u64.pow(attempts.min(6) as u32); // Cap at 2^6 = 64x
        let delay = base_delay * multiplier;
        let max_delay = std::time::Duration::from_secs(1800); // 30 minutes max
        
        let final_delay = std::cmp::min(delay, max_delay);
        time::OffsetDateTime::now_utc() + time::Duration::from(final_delay)
    }
}

// Database methods for webhook retries
impl Database {
    pub async fn get_ready_webhook_deliveries(&self) -> Result<Vec<WebhookJob>> {
        sqlx::query_as!(
            WebhookJob,
            "SELECT id as delivery_id, tenant_id 
             FROM webhook_deliveries 
             WHERE status = 'pending' 
               AND next_attempt_at <= now()
             ORDER BY next_attempt_at ASC
             LIMIT 100"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AcpError::Database)
    }
    
    pub async fn add_webhook_attempt(
        &self,
        delivery_id: &WebhookDeliveryId,
        http_status: u16,
        duration_ms: u64,
        error: Option<&str>,
    ) -> Result<()> {
        let attempt_metadata = serde_json::json!({
            "attempt": sqlx::query!("SELECT attempts + 1 as new_attempts FROM webhook_deliveries WHERE id = $1", delivery_id.0)
                .fetch_one(&self.pool)
                .await
                .map_err(AcpError::Database)?
                .new_attempts,
            "timestamp": time::OffsetDateTime::now_utc().to_string(),
            "http_status": http_status,
            "duration_ms": duration_ms,
            "error": error
        });
        
        sqlx::query!(
            "UPDATE webhook_deliveries 
             SET attempts = attempts + 1,
                 last_attempt_at = now(),
                 attempt_metadata = attempt_metadata || $1
             WHERE id = $2",
            attempt_metadata,
            delivery_id.0
        )
        .execute(&self.pool)
        .await
        .map_err(AcpError::Database)
    }
    
    pub async fn update_webhook_next_attempt(
        &self,
        delivery_id: &WebhookDeliveryId,
        next_attempt: time::OffsetDateTime,
    ) -> Result<()> {
        sqlx::query!(
            "UPDATE webhook_deliveries 
             SET next_attempt_at = $1, status = 'pending'
             WHERE id = $2",
            next_attempt,
            delivery_id.0
        )
        .execute(&self.pool)
        .await
        .map_err(AcpError::Database)
    }
}

fn sign_payload(payload: &str, secret: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(payload.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Queue a webhook for delivery
pub async fn queue_webhook(
    db: &Database,
    webhook_tx: &mpsc::Sender<WebhookJob>,
    tenant_id: &TenantId,
    event_type: WebhookEventType,
    data: Value,
) -> Result<()> {
    // Check if tenant has this event enabled
    let tenant = db.get_tenant(tenant_id).await?
        .ok_or_else(|| crate::error::AcpError::NotFound {
            entity: "tenant",
            id: tenant_id.to_string(),
        })?;

    let enabled_events: Vec<WebhookEventType> = tenant.webhook_events
        .map(|e| serde_json::from_value(e).unwrap_or_default())
        .unwrap_or_default();

    if !enabled_events.contains(&event_type) {
        return Ok(()); // Event not enabled for this tenant
    }

    let payload = WebhookPayload {
        event: event_type,
        timestamp: time::OffsetDateTime::now_utc().to_string(),
        data,
    };

    let delivery_id = WebhookDeliveryId::new();
    db.create_webhook_delivery(
        &delivery_id,
        tenant_id,
        &event_type.to_string(),
        &serde_json::to_value(&payload)?,
    ).await?;

    // Queue for async delivery
    let _ = webhook_tx.send(WebhookJob {
        tenant_id: tenant_id.clone(),
        delivery_id,
    }).await;

    Ok(())
}
```

---

## Tool Interface

```rust
// src/tools/mod.rs

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::error::Result;

pub mod email;
pub mod http;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ToolResult {
    pub fn success(result: impl Serialize) -> Self {
        Self {
            success: true,
            result: Some(serde_json::to_value(result).unwrap_or(Value::Null)),
            error: None,
        }
    }

    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            success: false,
            result: None,
            error: Some(error.into()),
        }
    }
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn validate(&self, params: &Value) -> Result<()>;
    async fn execute(&self, params: Value) -> ToolResult;
}

pub struct ToolRegistry {
    tools: std::collections::HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: std::collections::HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: impl Tool + 'static) {
        self.tools.insert(tool.name().to_string(), Box::new(tool));
    }

    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        let mut registry = Self::new();
        registry.register(email::SendEmailTool::new());
        registry.register(http::HttpRequestTool::new());
        registry
    }
}
```

### Email Tool

```rust
// src/tools/email.rs

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::info;
use crate::error::{AcpError, Result};
use super::{Tool, ToolResult};

#[derive(Debug, Deserialize)]
pub struct SendEmailParams {
    pub to: String,
    pub subject: String,
    pub body: String,
    #[serde(default)]
    pub cc: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SendEmailResult {
    pub message_id: String,
    pub sent_at: String,
}

pub struct SendEmailTool {
    dry_run: bool,
}

impl SendEmailTool {
    pub fn new() -> Self {
        Self { dry_run: true }
    }
}

#[async_trait]
impl Tool for SendEmailTool {
    fn name(&self) -> &'static str {
        "send_email"
    }

    fn validate(&self, params: &Value) -> Result<()> {
        let _: SendEmailParams = serde_json::from_value(params.clone())
            .map_err(|e| AcpError::BadRequest {
                message: format!("Invalid send_email params: {}", e),
            })?;
        Ok(())
    }

    async fn execute(&self, params: Value) -> ToolResult {
        let params: SendEmailParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolResult::failure(format!("Invalid params: {}", e)),
        };

        if self.dry_run {
            info!(
                to = %params.to,
                subject = %params.subject,
                "DRY RUN: Would send email"
            );
        }

        let message_id = format!("msg_{}", nanoid::nanoid!(12));
        ToolResult::success(SendEmailResult {
            message_id,
            sent_at: time::OffsetDateTime::now_utc().to_string(),
        })
    }
}
```

### HTTP Request Tool

```rust
// src/tools/http.rs

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::error::{AcpError, Result};
use super::{Tool, ToolResult};

#[derive(Debug, Deserialize)]
pub struct HttpRequestParams {
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub body: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct HttpRequestResult {
    pub status: u16,
    pub headers: std::collections::HashMap<String, String>,
    pub body: Option<Value>,
}

pub struct HttpRequestTool {
    client: reqwest::Client,
}

impl HttpRequestTool {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to build HTTP client"),
        }
    }
}

#[async_trait]
impl Tool for HttpRequestTool {
    fn name(&self) -> &'static str {
        "http_request"
    }

    fn validate(&self, params: &Value) -> Result<()> {
        let p: HttpRequestParams = serde_json::from_value(params.clone())
            .map_err(|e| AcpError::BadRequest {
                message: format!("Invalid http_request params: {}", e),
            })?;

        let valid_methods = ["GET", "POST", "PUT", "PATCH", "DELETE"];
        if !valid_methods.contains(&p.method.to_uppercase().as_str()) {
            return Err(AcpError::BadRequest {
                message: format!("Invalid HTTP method: {}", p.method),
            });
        }

        if !p.url.starts_with("http://") && !p.url.starts_with("https://") {
            return Err(AcpError::BadRequest {
                message: "URL must start with http:// or https://".into(),
            });
        }

        Ok(())
    }

    async fn execute(&self, params: Value) -> ToolResult {
        let params: HttpRequestParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolResult::failure(format!("Invalid params: {}", e)),
        };

        let method = match params.method.to_uppercase().as_str() {
            "GET" => reqwest::Method::GET,
            "POST" => reqwest::Method::POST,
            "PUT" => reqwest::Method::PUT,
            "PATCH" => reqwest::Method::PATCH,
            "DELETE" => reqwest::Method::DELETE,
            _ => return ToolResult::failure("Invalid HTTP method"),
        };

        let mut request = self.client.request(method, &params.url);

        for (key, value) in &params.headers {
            request = request.header(key, value);
        }

        if let Some(body) = params.body {
            request = request.json(&body);
        }

        match request.send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let headers: std::collections::HashMap<String, String> = response
                    .headers()
                    .iter()
                    .filter_map(|(k, v)| {
                        v.to_str().ok().map(|v| (k.to_string(), v.to_string()))
                    })
                    .collect();
                
                // Try JSON first, fallback to text (capped at 10KB)
                let body = match response.json().await {
                    Ok(json) => Some(json),
                    Err(_) => {
                        // Fallback to text for non-JSON responses
                        response.text().await
                            .ok()
                            .and_then(|text| {
                                if text.len() > 10240 {
                                    Some(Value::String(format!("{}... [truncated]", &text[..10240])))
                                } else {
                                    Some(Value::String(text))
                                }
                            })
                    }
                };

                ToolResult::success(HttpRequestResult { status, headers, body })
            }
            Err(e) => ToolResult::failure(format!("HTTP request failed: {}", e)),
        }
    }
}
```

---

## Policy Engine

```rust
// src/policy/mod.rs

use crate::domain::{AutonomyLevel, PolicyResult, ToolName};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct PolicyDecision {
    pub result: PolicyResult,
    pub reason: String,
}

impl PolicyDecision {
    pub fn allow(reason: impl Into<String>) -> Self {
        Self { result: PolicyResult::Allow, reason: reason.into() }
    }

    pub fn deny(reason: impl Into<String>) -> Self {
        Self { result: PolicyResult::Deny, reason: reason.into() }
    }

    pub fn require_approval(reason: impl Into<String>) -> Self {
        Self { result: PolicyResult::RequireApproval, reason: reason.into() }
    }
}

pub struct PolicyEngine {
    blocked_email_domains: Vec<String>,
    internal_domain: String,
}

impl PolicyEngine {
    pub fn new() -> Self {
        Self {
            blocked_email_domains: vec![
                "competitor.com".into(),
                "spam.example.com".into(),
            ],
            internal_domain: "mycompany.com".into(),
        }
    }

    pub fn evaluate(
        &self,
        tool: &ToolName,
        params: &Value,
        autonomy: AutonomyLevel,
    ) -> PolicyDecision {
        if autonomy == AutonomyLevel::Restricted {
            return PolicyDecision::require_approval(
                "Restricted session requires approval for all actions"
            );
        }

        match tool {
            ToolName::SendEmail => self.evaluate_email(params, autonomy),
            ToolName::HttpRequest => self.evaluate_http(params, autonomy),
        }
    }

    fn evaluate_email(&self, params: &Value, autonomy: AutonomyLevel) -> PolicyDecision {
        let to = params.get("to").and_then(|v| v.as_str()).unwrap_or("");
        let domain = to.split('@').last().unwrap_or("");

        if self.blocked_email_domains.iter().any(|d| d == domain) {
            return PolicyDecision::deny(format!("Blocked email domain: {}", domain));
        }

        if domain == self.internal_domain && autonomy == AutonomyLevel::Supervised {
            return PolicyDecision::allow("Internal email in supervised mode");
        }

        if autonomy == AutonomyLevel::Supervised {
            return PolicyDecision::require_approval("External email requires approval");
        }

        PolicyDecision::allow("Full autonomy mode")
    }

    fn evaluate_http(&self, params: &Value, autonomy: AutonomyLevel) -> PolicyDecision {
        let url = params.get("url").and_then(|v| v.as_str()).unwrap_or("");
        let method = params.get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("GET")
            .to_uppercase();

        if autonomy == AutonomyLevel::Supervised && method != "GET" {
            return PolicyDecision::require_approval(
                format!("{} request requires approval in supervised mode", method)
            );
        }

        if url.contains("localhost") || url.contains("127.0.0.1") {
            return PolicyDecision::deny("Cannot make requests to localhost");
        }

        PolicyDecision::allow("HTTP request permitted")
    }
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}
```

---

## API Routes

```rust
// src/api/routes.rs

use axum::{
    middleware,
    routing::{get, patch, post},
    Router,
};
use crate::AppState;
use super::auth::{agent_auth_middleware, dashboard_auth_middleware};

/// Agent API routes (authenticated via API key)
pub fn agent_api_router() -> Router<AppState> {
    Router::new()
        // Sessions
        .route("/v1/sessions", post(super::handlers::create_session))
        .route("/v1/sessions/:id", get(super::handlers::get_session))
        // Actions
        .route("/v1/actions", post(super::handlers::create_action))
        .route("/v1/actions/:id", get(super::handlers::get_action))
        .route("/v1/actions", get(super::handlers::list_actions))
        // Approvals
        .route("/v1/approvals", get(super::handlers::list_approvals))
        .route("/v1/approvals/:id", patch(super::handlers::update_approval))
        // NCHE-native records
        .route("/v1/records/tasks", post(super::handlers::create_task))
        .route("/v1/records/tasks", get(super::handlers::list_tasks))
        .route("/v1/records/tasks/:id", get(super::handlers::get_task))
        .route("/v1/records/cases", post(super::handlers::create_case))
        .route("/v1/records/cases", get(super::handlers::list_cases))
        .route("/v1/records/cases/:id", get(super::handlers::get_case))
        .route("/v1/records/documents", post(super::handlers::create_document))
        .route("/v1/records/documents/:id", get(super::handlers::get_document))
}

/// Dashboard API routes (authenticated via session)
pub fn dashboard_api_router() -> Router<AppState> {
    Router::new()
        // Approvals
        .route("/dashboard/approvals", get(super::handlers::list_pending_approvals))
        .route("/dashboard/approvals/:id", get(super::handlers::get_approval_detail))
        .route("/dashboard/approvals/:id", patch(super::handlers::update_approval))
        // Audit log
        .route("/dashboard/audit", get(super::handlers::list_audit_events))
}

/// Build complete router with proper state injection
pub fn router(state: AppState) -> Router {
    Router::new()
        // Agent API with auth middleware
        .merge(
            agent_api_router()
                .layer(middleware::from_fn_with_state(state.clone(), agent_auth_middleware))
        )
        // Dashboard API with auth middleware
        .merge(
            dashboard_api_router()
                .layer(middleware::from_fn_with_state(state.clone(), dashboard_auth_middleware))
        )
        // Dashboard auth routes (no auth required)
        .merge(dashboard_auth_router())
        // Health check (no auth required)
        .merge(health_router())
        // Serve dashboard SPA
        .route("/dashboard/*path", get(super::dashboard::serve_dashboard))
        .route("/", get(super::dashboard::serve_dashboard))
        .with_state(state)
}
```

---

## Dashboard Embedding

### Build Process

**Option A: Commit pre-built assets (recommended for v0.1)**

```bash
# One-time setup
cd dashboard
npm ci
npm run build
cd ..

# Commit built assets
git add dashboard/dist/
git commit -m "Add built dashboard assets"
```

**Option B: Build during Rust build (more complex)**

```rust
// build.rs

use std::process::Command;

fn main() {
    // Only build dashboard if not already present
    if !std::path::Path::new("dashboard/dist/index.html").exists() {
        println!("cargo:warning=Building dashboard assets...");
        
        let output = Command::new("npm")
            .args(&["ci"])
            .current_dir("dashboard")
            .output()
            .expect("Failed to run npm ci");
            
        if !output.status.success() {
            panic!("npm ci failed: {}", String::from_utf8_lossy(&output.stderr));
        }
        
        let output = Command::new("npm")
            .args(&["run", "build"])
            .current_dir("dashboard")
            .output()
            .expect("Failed to run npm build");
            
        if !output.status.success() {
            panic!("npm build failed: {}", String::from_utf8_lossy(&output.stderr));
        }
        
        println!("cargo:warning=Dashboard assets built successfully");
    }
    
    // Tell Cargo to rerun build.rs if dashboard files change
    println!("cargo:rerun-if-changed=dashboard/src");
    println!("cargo:rerun-if-changed=dashboard/package.json");
}
```

### Recommended Approach for v0.1

**Commit pre-built assets** because:
- Simpler build process for users
- No npm dependency during cargo build
- More reliable builds
- Easy to audit changes

Add to `.gitignore`:
```
# Don't ignore built assets for v0.1
# dashboard/dist/
```

### Dashboard Serving (unchanged)

```rust
// src/dashboard/mod.rs

use axum::{
    body::Body,
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "dashboard/dist"]
struct DashboardAssets;

pub async fn serve_dashboard(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() || !path.contains('.') {
        "index.html"
    } else {
        path
    };

    match DashboardAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path)
                .first_or_octet_stream()
                .to_string();
            
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                .body(Body::from(content.data.to_vec()))
                .unwrap()
        }
        None => {
            // SPA fallback
            match DashboardAssets::get("index.html") {
                Some(content) => Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "text/html")
                    .body(Body::from(content.data.to_vec()))
                    .unwrap(),
                None => Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::from("Not found"))
                    .unwrap(),
            }
        }
    }
}
```
```

### Dashboard Pages (React)

**1. Login Page**

- Email + password form
- Error handling
- Redirect to approvals on success

**2. Pending Approvals (Home)**

```text
┌─────────────────────────────────────────────────────────────────┐
│  🔔 Pending Approvals (3)                              [Logout] │
├─────────────────────────────────────────────────────────────────┤
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ send_email to client@example.com          5 min ago        │ │
│ │ "External email requires approval"                          │ │
│ │                                    [View] [Approve] [Deny]  │ │
│ └─────────────────────────────────────────────────────────────┘ │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ http_request POST to api.stripe.com       12 min ago       │ │
│ │ "POST request requires approval in supervised mode"         │ │
│ │                                    [View] [Approve] [Deny]  │ │
│ └─────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

**3. Action Detail**

```text
┌─────────────────────────────────────────────────────────────────┐
│  ← Back                                     Action act_abc123   │
├─────────────────────────────────────────────────────────────────┤
│  Tool: send_email                                               │
│  State: PAUSED_FOR_APPROVAL                                     │
│  Created: 2026-01-15 12:00:00                                   │
│                                                                 │
│  Parameters:                                                    │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ {                                                          │ │
│  │   "to": "client@example.com",                              │ │
│  │   "subject": "Your renewal is due",                        │ │
│  │   "body": "Please review..."                               │ │
│  │ }                                                          │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                 │
│  Policy Reason: External email requires approval                │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Note (optional): ____________________________            │   │
│  │                                                          │   │
│  │                    [Approve]  [Deny]                     │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                 │
│  Event Timeline:                                                │
│  • action.proposed - 12:00:00                                   │
│  • action.policy_evaluated - 12:00:01                           │
│  • action.approval_requested - 12:00:01                         │
└─────────────────────────────────────────────────────────────────┘
```

**4. Audit Log**

```text
┌─────────────────────────────────────────────────────────────────┐
│  📋 Audit Log                                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ Filter: [All States ▼] [All Tools ▼] [Last 7 days ▼]   │    │
│  └─────────────────────────────────────────────────────────┘    │
├─────────────────────────────────────────────────────────────────┤
│  ID           Tool          State      Created                  │
│  ──────────────────────────────────────────────────────────     │
│  act_abc123   send_email    EXECUTED   2026-01-15 12:05        │
│  act_def456   http_request  DENIED     2026-01-15 11:30        │
│  act_ghi789   send_email    FAILED     2026-01-15 10:15        │
│                                                                 │
│  [< Prev]  Page 1 of 10  [Next >]                              │
└─────────────────────────────────────────────────────────────────┘
```

---

## Configuration

```yaml
# nche.yaml

server:
  host: 0.0.0.0
  port: 8080

database:
  # Postgres (recommended for production)
  url: postgres://nche:nche@localhost:5432/nche
  max_connections: 10

tenants:
  # Bootstrap tenant (created on init)
  - id: ten_default
    name: Default Tenant
    webhook_url: ${WEBHOOK_URL}
    webhook_secret: ${WEBHOOK_SECRET}
    webhook_events:
      - approval_required
      - action_executed
      - action_failed

agents:
  # Bootstrap agent (created on init)
  - id: agent_demo
    tenant_id: ten_default
    name: Demo Agent
    api_key: ${AGENT_API_KEY}

dashboard_users:
  # Bootstrap dashboard user
  - email: admin@example.com
    password: ${DASHBOARD_PASSWORD}
    tenant_id: ten_default
    name: Admin User

tools:
  send_email:
    provider: console  # or 'smtp', 'sendgrid'
  http_request:
    timeout_seconds: 30
```

---

## CLI

```rust
// src/cli.rs

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "nche")]
#[command(about = "Agent Control Plane - Safe, auditable control for AI agents")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(short, long, default_value = "nche.yaml")]
    pub config: String,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the NCHE server
    Serve {
        /// Also serve the dashboard
        #[arg(long, default_value = "true")]
        dashboard: bool,
    },

    /// Initialize database and bootstrap data
    Init,

    /// Manage tenants
    Tenants {
        #[command(subcommand)]
        action: TenantCommands,
    },

    /// Manage approvals
    Approvals {
        #[command(subcommand)]
        action: ApprovalCommands,
    },

    /// Database migrations
    Migrate,
}

#[derive(Subcommand)]
pub enum TenantCommands {
    /// List all tenants
    List,
    /// Create a new tenant
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        webhook_url: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ApprovalCommands {
    /// List pending approvals
    List {
        #[arg(long)]
        tenant: Option<String>,
        #[arg(long, default_value = "pending")]
        status: String,
    },
    /// Approve an action
    Approve {
        id: String,
        #[arg(long)]
        approver: String,
        #[arg(long)]
        note: Option<String>,
    },
    /// Deny an action
    Deny {
        id: String,
        #[arg(long)]
        approver: String,
        #[arg(long)]
        reason: String,
    },
}
```

---

## Project Structure

```text
nche/
├── README.md
├── SPEC.md
├── LICENSE                         # MIT
├── Cargo.toml
├── Cargo.lock
├── docker-compose.yml
├── Dockerfile
├── nche.yaml.example
├── migrations/
│   ├── postgres/
│   │   └── 001_initial.sql
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── config.rs
│   ├── error.rs
│   ├── db/
│   │   ├── mod.rs
│   │   ├── postgres.rs
│   ├── domain/
│   │   ├── mod.rs
│   │   └── state_machine.rs
│   ├── api/
│   │   ├── mod.rs
│   │   ├── routes.rs
│   │   ├── handlers.rs
│   │   ├── types.rs
│   │   └── auth.rs
│   ├── policy/
│   │   └── mod.rs
│   ├── tools/
│   │   ├── mod.rs
│   │   ├── email.rs
│   │   └── http.rs
│   ├── webhooks/
│   │   └── mod.rs
│   ├── executor.rs
│   ├── events.rs
│   ├── dashboard/
│   │   └── mod.rs                  # rust-embed for SPA
│   └── cli/
│       ├── mod.rs
│       ├── serve.rs
│       ├── init.rs
│       ├── tenants.rs
│       └── approvals.rs
├── dashboard/                      # React SPA
│   ├── package.json
│   ├── vite.config.ts
│   ├── src/
│   │   ├── main.tsx
│   │   ├── App.tsx
│   │   ├── api.ts
│   │   ├── pages/
│   │   │   ├── Login.tsx
│   │   │   ├── Approvals.tsx
│   │   │   ├── ActionDetail.tsx
│   │   │   └── AuditLog.tsx
│   │   └── components/
│   │       ├── ApprovalCard.tsx
│   │       ├── ActionParams.tsx
│   │       └── EventTimeline.tsx
│   └── dist/                       # Built assets (embedded)
├── tests/
│   ├── common/mod.rs
│   ├── api_test.rs
│   ├── policy_test.rs
│   ├── webhook_test.rs
│   └── state_machine_test.rs
└── examples/
    ├── demo_agent.rs
    └── scenarios/
        ├── email_approved.sh
        ├── email_denied.sh
        └── full_workflow.sh
```

---

## Cargo.toml

```toml
[package]
name = "nche"
version = "0.1.0"
edition = "2021"
description = "Agent Control Plane - Safe, auditable control for AI agents"
license = "MIT"
repository = "https://github.com/usercando/nche"
keywords = ["ai", "agents", "control-plane", "governance", "audit"]

[features]
default = ["postgres"]
postgres = ["sqlx/postgres"]

[dependencies]
# Web framework
axum = { version = "0.7", features = ["macros"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "trace"] }

# Async
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"

# Database
sqlx = { version = "0.7", features = ["runtime-tokio", "macros", "postgres", "json", "time"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"

# HTTP client
reqwest = { version = "0.12", features = ["json"] }

# CLI
clap = { version = "4", features = ["derive"] }

# Utilities
nanoid = "0.4"
time = { version = "0.3", features = ["serde"] }
thiserror = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# Auth
argon2 = "0.5"
hmac = "0.12"
sha2 = "0.10"
hex = "0.4"

# Dashboard embedding
rust-embed = "8"
mime_guess = "2"

[dev-dependencies]
tokio-test = "0.4"
tempfile = "3"

[build-dependencies]
# For building dashboard
npm_rs = "1"
```

---

## Docker Compose

```yaml
# docker-compose.yml

version: '3.8'

services:
  nche:
    build: .
    ports:
      - "8080:8080"
    environment:
      - DATABASE_URL=postgres://nche:nche@postgres:5432/nche
      - AGENT_API_KEY=demo-api-key-change-me
      - DASHBOARD_PASSWORD=admin123
      - WEBHOOK_URL=
      - WEBHOOK_SECRET=
    depends_on:
      postgres:
        condition: service_healthy
    command: ["nche", "serve"]

  postgres:
    image: postgres:16-alpine
    environment:
      - POSTGRES_USER=nche
      - POSTGRES_PASSWORD=nche
      - POSTGRES_DB=nche
    volumes:
      - postgres_data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U nche"]
      interval: 5s
      timeout: 5s
      retries: 5

volumes:
  postgres_data:
```

---

## 6-Week Build Plan

### Week 1: Foundation

| Day | Focus |
|-----|-------|
| 1 | Project setup, Cargo.toml, feature flags |
| 2 | Config loading, error types, tracing |
| 3 | Domain types, all IDs, enums |
| 4 | State machine with tests |
| 5 | Tenant model, multi-tenant schema |

### Week 2: Database

| Day | Focus |
|-----|-------|
| 6 | Postgres migrations, sqlx setup |
| 8 | CRUD: tenants, agents, sessions |
| 9 | CRUD: actions, approvals |
| 10 | CRUD: events, webhook_deliveries |

### Week 3: Core API

| Day | Focus |
|-----|-------|
| 11 | Axum setup, health endpoint |
| 12 | Agent auth middleware, tenant scoping |
| 13 | Sessions endpoints |
| 14 | Actions: propose, get, policy engine |
| 15 | Event logging, integration test |

### Week 4: Approvals + Webhooks

| Day | Focus |
|-----|-------|
| 16 | Approval endpoints (list, get) |
| 17 | Approve/deny + state transitions |
| 18 | Webhook dispatcher (background task) |
| 19 | Webhook delivery, retries, HMAC signing |
| 20 | Tools: send_email, http_request, executor |

### Week 5: Dashboard

| Day | Focus |
|-----|-------|
| 21 | React setup, Vite, Tailwind |
| 22 | Login page, session management |
| 23 | Approvals list page |
| 24 | Action detail + approve/deny |
| 25 | Audit log page, rust-embed integration |

### Week 6: Polish

| Day | Focus |
|-----|-------|
| 26 | CLI: serve, init, migrate |
| 27 | CLI: tenants, approvals |
| 28 | Docker, docker-compose, tests |
| 29 | README, architecture diagram |
| 30 | Demo recording, HN post draft |

---

## Success Criteria

- [ ] `docker compose up` → NCHE + Postgres running in <90 seconds
- [ ] Create tenant via CLI
- [ ] Agent can propose action via API
- [ ] Action requiring approval triggers webhook
- [ ] Dashboard shows pending approval
- [ ] Approve via dashboard → action executes
- [ ] Audit log shows full trail
- [ ] All tests pass
- [ ] Binary < 25MB (with embedded dashboard)
- [ ] Works on Mac + Linux

---

## HN Post Draft

**Title:** Show HN: NCHE – Open-source control plane for AI agents (Rust)

**Text:**

````text
Hi HN,

AI agents are moving from "suggesting" to "doing" – sending emails, 
calling APIs, submitting forms. That's powerful, and terrifying 
without proper controls.

NCHE (Agent Control Plane) is the missing layer between your AI agent 
and the real world:

- Enforces policies (allow / deny / require approval)
- Pauses for human approval when needed
- Delivers webhooks so you know something's waiting
- Keeps an immutable audit trail
- Includes a web dashboard for approvals

Agents never touch tools or credentials directly. Only NCHE does.

I built this after reading a WaPo piece [1] calling for exactly these
guardrails. NCHE implements all 5 recommendations from that article.

Rust. Multi-tenant. Postgres. Single binary with embedded dashboard.

  git clone https://github.com/usercando/nche
  docker compose up
  open http://localhost:8080

Repo: https://github.com/usercando/nche

What's NOT in v0.1: policy DSL, RBAC, MCP integration, OpenTelemetry.
Happy to discuss the roadmap.

[1] https://washingtonpost.com/opinions/2026/01/05/agentic-ai-guardrails/
````

---

## Post-Launch Roadmap

### Phase 1: Tools Expansion

| Tool | Description | Priority |
|------|-------------|----------|
| `slack_message` | Send to channel or DM | High |
| `slack_reaction` | Add emoji reaction | Medium |
| `calendar_create` | Create Google/Outlook event | High |
| `calendar_reschedule` | Move existing event | Medium |
| `file_upload` | Upload to S3/GCS/Azure | High |
| `file_download` | Retrieve from storage | Medium |
| `pdf_fill` | Fill PDF form fields | Medium |
| `docusign_send` | Send for signature | Medium |
| `jira_create` | Create ticket | Medium |
| `jira_update` | Update ticket status | Medium |
| `zendesk_ticket` | Create support ticket | Low |
| `sms_send` | Send SMS via Twilio | Medium |
| `database_query` | Run read-only SQL | Low |
| `anthropic_message` | Call Claude API | High |

**BeneCRM-specific (Phase 2):**

- `carrier_portal_submit` — Submit to insurance carrier portals
- `census_upload` — Upload employee census files
- `form_5500_file` — File Form 5500 with DOL

---

### Phase 2: Dashboard Features

| Feature | Description |
|---------|-------------|
| Real-time updates | WebSocket push for new approvals |
| Bulk actions | Approve/deny multiple at once |
| Filters & search | By date, tool, status, actor |
| Approval delegation | "Out of office" routing |
| Approval rules | UI for policy configuration |
| Session inspector | Tree view of session actions |
| Agent management | Create/revoke API keys |
| Tenant admin | User management, settings |
| Audit export | CSV/JSON download |
| Dashboards | Actions/day, approval latency |
| Mobile responsive | Approve from phone |
| Slack integration | Approve directly from Slack |
| Email digest | Daily summary |

---

### Phase 3: OpenTelemetry

**What:** Industry standard for observability (traces, metrics, logs).

**Why NCHE needs it:**

1. **Distributed tracing** — See time in policy, approval wait, execution
2. **Metrics** — Actions/min, approval latency p95, error rate by tenant
3. **Integration** — Exports to Datadog, New Relic, Jaeger, Grafana
4. **Debugging** — Trace ID follows request through entire flow

**Implementation:**

```toml
# Cargo.toml additions
opentelemetry = "0.21"
opentelemetry-otlp = "0.14"
tracing-opentelemetry = "0.22"
```

```yaml
# nche.yaml
telemetry:
  enabled: true
  otlp_endpoint: http://localhost:4317
  service_name: nche
```

---

### Phase 4: MCP Integration

**What:** Model Context Protocol — Anthropic's standard for agent-tool connectivity.

**Use cases:**

1. **NCHE as MCP server**
   - Any MCP-compatible agent discovers NCHE tools
   - Claude, ChatGPT, Cursor can use NCHE natively

2. **NCHE wrapping MCP servers**
   - Add governance to existing MCP tools
   - Agent → NCHE → [Policy] → MCP Server → Tool

3. **Multi-agent coordination**
   - NCHE controls agent-to-agent delegation
   - Approval required for cross-agent calls

4. **Tool composition**
   - One NCHE action = multiple MCP tool calls
   - Single approval for composed workflows

---

## Final Notes

This spec is aggressive but achievable in 6 weeks with focused execution.

**Critical path:**

1. Multi-tenant schema (Week 1-2) — Everything depends on this
2. Webhook delivery (Week 4) — Can't demo without notifications
3. Dashboard MVP (Week 5) — CLI-only won't land on HN

**Acceptable shortcuts:**

- Dashboard can be ugly
- Email tool can be dry-run only
- Policy engine is hardcoded
- No retry logic for webhooks (log failures, move on)

**Not acceptable:**

- Broken multi-tenancy
- Missing audit trail
- Webhook signature missing (security)
