# Reading the NCHE Codebase

This guide helps you understand the NCHE backend architecture and provides a recommended order for reading the code.

## Architecture Overview

NCHE is an **Agent Control Plane** - infrastructure for AI agent oversight. The core flow is:

```
Agent → Creates Session → Proposes Action → Policy Evaluation → Approval/Execute → Webhook
```

The backend is a Rust application using:
- **Axum** - Web framework
- **SQLx** - Async PostgreSQL
- **Tokio** - Async runtime

## Directory Structure

```
backend/nche/
├── src/
│   ├── main.rs          # CLI entrypoint
│   ├── lib.rs           # Library exports
│   ├── config.rs        # YAML configuration loading
│   ├── domain/          # Core types and state machine
│   ├── db/              # Database operations
│   │   ├── mod.rs       # Database connection
│   │   ├── tenants.rs   # Tenant CRUD
│   │   ├── agents.rs    # Agent + API key management
│   │   ├── sessions.rs  # Session CRUD
│   │   ├── actions.rs   # Action CRUD + locking
│   │   ├── approvals.rs # Approval workflow
│   │   ├── events.rs    # Audit logging
│   │   ├── webhooks.rs  # Webhook delivery tracking
│   │   ├── dashboard.rs # Dashboard auth
│   │   ├── tasks.rs     # NCHE-Native: Tasks
│   │   ├── cases.rs     # NCHE-Native: Cases
│   │   ├── documents.rs # NCHE-Native: Documents
│   │   └── links.rs     # NCHE-Native: Links
│   ├── api/             # HTTP handlers and routing
│   │   ├── mod.rs       # Routes, handlers, PaginatedResponse
│   │   ├── auth.rs      # Auth + CSRF middleware
│   │   └── records.rs   # NCHE-Native Records handlers
│   ├── policy/          # Policy evaluation engine
│   ├── tools/           # Tool execution (send_email, http_request)
│   ├── executor/        # Background action executor
│   ├── webhooks/        # Webhook delivery system
│   ├── dashboard/       # Embedded UI serving
│   └── error.rs         # Error types
└── tests/               # Integration tests
    ├── common/
    │   └── mod.rs       # Test helpers: TestContext, fixtures
    ├── db_test.rs       # Database integration tests
    ├── api_test.rs      # API endpoint tests
    └── webhook_test.rs  # Webhook delivery tests
```

---

## Recommended Reading Order

### Phase 1: Core Domain (Start Here)

#### 1. `src/domain/mod.rs`
**Purpose:** Defines all core types - IDs, enums, and entity structs.

Key concepts:
- **ID newtypes** (`TenantId`, `AgentId`, `TaskId`, `CaseId`, etc.) - Type-safe identifiers with nanoid generation
- **ActionState enum** - The heart of the system: `Proposed → PausedForApproval → ReadyToExecute → Executed`
- **Core entity structs** - `Tenant`, `Agent`, `Session`, `Action`, `Approval`, `Event`
- **Native record structs** - `Task`, `Case`, `Document`, `Link`
- **Status enums** - `TaskStatus`, `CaseStatus`, `Severity`, `RecordType`

```rust
// Example: Understanding the ID macro
define_id!(ActionId, "act");  // Creates act_xxxxxxxxxxxx IDs
define_id!(TaskId, "task");   // Creates task_xxxxxxxxxxxx IDs
```

#### 2. `src/domain/state_machine.rs`
**Purpose:** Defines valid state transitions for actions.

This is critical - it enforces what transitions are allowed:
- `Proposed` can go to `ReadyToExecute`, `PausedForApproval`, or `Denied`
- `PausedForApproval` can go to `ReadyToExecute` (approved) or `Denied`
- `ReadyToExecute` can go to `Executing`
- etc.

#### 3. `src/error.rs`
**Purpose:** Custom error types with HTTP response mapping.

Note how `NcheError` implements `IntoResponse` to return proper JSON errors with status codes.

---

### Phase 2: Database Layer

#### 4. `src/db/mod.rs`
**Purpose:** Database connection and migration setup.

The `Database` struct wraps a `PgPool`. All database operations are methods on this struct.

#### 5. `src/db/tenants.rs`
**Purpose:** Tenant CRUD operations.

Simple starting point - shows the pattern used throughout:
- Build SQL query
- Bind parameters
- Execute and map results

#### 6. `src/db/agents.rs`
**Purpose:** Agent management and API key handling.

**Important:** Study the `ApiKey` struct:
- `generate()` - Creates `nche_<agent_id>_<secret>` format
- `parse()` - Extracts prefix and secret from API key
- Password hashing with Argon2

#### 7. `src/db/sessions.rs`, `src/db/actions.rs`
**Purpose:** Session and action CRUD.

Note `lock_ready_actions()` - uses `SELECT FOR UPDATE SKIP LOCKED` for safe concurrent execution.

#### 8. `src/db/approvals.rs`
**Purpose:** Approval management.

Study `decide_approval()` - it's a transaction that atomically:
1. Updates approval status
2. Updates action state
3. Rolls back if action isn't in expected state

#### 9. `src/db/events.rs`, `src/db/webhooks.rs`, `src/db/dashboard.rs`
**Purpose:** Event logging, webhook delivery tracking, dashboard auth.

#### 10. `src/db/tasks.rs`, `src/db/cases.rs`, `src/db/documents.rs`, `src/db/links.rs`
**Purpose:** NCHE-Native Records - structured data types for agent oversight.

These implement CRUD + archive/unarchive for:
- **Tasks** - Simple work items with status (open/in_progress/completed)
- **Cases** - Investigation records with severity (low/medium/high/critical)
- **Documents** - Metadata-only file records with tags
- **Links** - Graph relationships between any record types

All support soft-delete via `archived_at` timestamp.

---

### Phase 3: Policy Engine

#### 11. `src/policy/mod.rs`
**Purpose:** Policy evaluation dispatcher.

The `PolicyEngine` evaluates actions based on tool type and autonomy level:
```rust
pub fn evaluate(&self, action: &CreateActionRequest, autonomy: AutonomyLevel) -> PolicyDecision
```

#### 12. `src/policy/email.rs`, `src/policy/http.rs`
**Purpose:** Tool-specific policy rules.

Example rules:
- Internal emails → Allow in supervised mode
- External emails → Require approval
- HTTP GET → Allow in supervised mode
- HTTP POST/PUT/DELETE → Require approval

---

### Phase 4: API Layer

#### 13. `src/api/auth.rs`
**Purpose:** Authentication and CSRF middleware.

Three middleware functions:
- `agent_auth_middleware` - Extracts API key from `Authorization: Bearer` header, verifies, injects `AgentAuthContext`
- `dashboard_auth_middleware` - Extracts session from `nche_session` cookie, verifies, injects `DashboardAuthContext`
- `csrf_protected_middleware` - For POST/PUT/PATCH/DELETE, requires `X-Session-Id` header matching session cookie

#### 14. `src/api/mod.rs`
**Purpose:** HTTP routing and handlers.

This is the largest file. Read in sections:

1. **Router setup** (`create_router`)
   - Routes are organized: `/health`, `/v1/*`, `/dashboard/*`
   - Middleware layers: CORS, tracing, auth, CSRF

2. **Agent API handlers**
   - `create_session`, `create_action`, `get_action`, `list_actions`, etc.

3. **Dashboard API handlers**
   - `dashboard_login`, `get_approvals`, `decide_approval`, `dashboard_stats`, etc.

4. **Request/Response types**
   - Located at bottom of file
   - `CreateSessionRequest`, `ActionResponse`, `PaginatedResponse<T>`, etc.

5. **PaginatedResponse<T>**
   - Generic wrapper for list endpoints with `data`, `limit`, `offset`, `has_more`
   - Uses "fetch N+1" pattern to determine `has_more` without COUNT queries

#### 15. `src/api/records.rs`
**Purpose:** NCHE-Native Records API handlers.

Handlers for `/v1/records/*` endpoints:
- Tasks, Cases, Documents, Links - each with:
  - `create_*`, `get_*`, `list_*`
  - `list_archived_*`, `archive_*`, `unarchive_*`
- All list endpoints return `PaginatedResponse<T>`

---

### Phase 5: Background Services

#### 16. `src/executor/mod.rs`
**Purpose:** Background action execution.

The executor:
1. Polls for `ready_to_execute` actions
2. Locks them atomically (prevents double execution)
3. Dispatches to tool executor
4. Updates state to `executed` or `failed`
5. Queues webhooks

Key function: `execute_action()` - orchestrates the full execution flow.

#### 17. `src/tools/mod.rs`
**Purpose:** Tool trait, registry, and implementations.

Key components:
- **`Tool` trait** - Interface for tools with `name()`, `validate()`, and `execute()` methods
- **`ToolRegistry`** - Registry for managing tools by name with `register()` and `get()`
- **`ToolExecutor`** - Uses registry to validate and execute tools
- **`SendEmailTool`** - Email tool (mock implementation, validates email format)
- **`HttpRequestTool`** - HTTP client (validates URL and method)

#### 18. `src/webhooks/mod.rs`
**Purpose:** Webhook delivery with retries.

Study:
- `WebhookSender` - HMAC-SHA256 signing, HTTP delivery
- `WebhookDispatcher` - Background poller with exponential backoff
- `queue_webhook()` - Helper to create webhook deliveries

---

### Phase 6: Configuration & CLI

#### 19. `src/config.rs`
**Purpose:** YAML configuration file loading.

Loads `nche.yaml` or `nche.yml` and applies environment variable overrides. Supports database, server, executor, webhook, and logging configuration.

#### 20. `src/dashboard/mod.rs`
**Purpose:** Embedded static file serving.

Uses `rust-embed` to embed the Next.js build. Note the SPA fallback logic for client-side routing.

#### 21. `src/main.rs`
**Purpose:** CLI entrypoint.

Uses `clap` for argument parsing. Commands:
- `serve` - Start server with all options
- `migrate` - Run migrations
- `init` - Bootstrap tenant/agent/user
- `tenants` - Tenant management
- `approvals` - Approval management

---

## Key Patterns

### 1. Request Context Extraction

Auth context is injected via middleware and extracted in handlers:

```rust
async fn handler(
    State(state): State<AppState>,
    Extension(auth): Extension<AgentAuthContext>,
    Json(req): Json<CreateActionRequest>,
) -> Result<Json<ActionResponse>, NcheError> {
    // auth.tenant_id, auth.agent_id available
}
```

### 2. Transaction Patterns

Critical operations use transactions:

```rust
let mut tx = self.pool.begin().await?;
// ... multiple operations ...
tx.commit().await?;
```

### 3. Background Task Spawning

Background services are spawned with handles:

```rust
pub fn spawn_executor(db: Arc<Database>, config: ExecutorConfig) -> JoinHandle<()> {
    tokio::spawn(async move {
        // polling loop
    })
}
```

### 4. Error Handling

Errors flow through `NcheError` which maps to HTTP responses:

```rust
pub enum NcheError {
    NotFound(String),           // 404
    Unauthorized(String),       // 401
    InvalidStateTransition { }, // 400
    Database(sqlx::Error),      // 500
    // ...
}
```

### 5. CSRF Protection

Dashboard mutations require `X-Session-Id` header matching the session cookie:

```rust
// csrf_protected_middleware checks:
// 1. Is this a mutation? (POST/PUT/PATCH/DELETE)
// 2. Does X-Session-Id header match nche_session cookie?
// 3. If mismatch → 403 Forbidden
```

### 6. Paginated Responses

List endpoints use the "fetch N+1" pattern for efficient pagination:

```rust
// Handler fetches limit + 1 items
let items = db.list_items(limit + 1, offset).await?;

// PaginatedResponse determines has_more without COUNT query
Ok(Json(PaginatedResponse::from_items(items, limit, offset)))
// Response: { data: [...], limit: 50, offset: 0, has_more: true }
```

---

## Data Flow Examples

### Creating an Action (Full Flow)

1. `POST /v1/actions` hits `create_action` handler
2. Handler extracts `AgentAuthContext` from middleware
3. Validates session exists and belongs to tenant
4. Creates `Action` in `proposed` state
5. Calls `PolicyEngine::evaluate()`
6. Based on result:
   - `Allow` → State becomes `ready_to_execute`
   - `Deny` → State becomes `denied`
   - `RequireApproval` → State becomes `paused_for_approval`, creates `Approval`
7. Creates event log entry
8. If approval required, queues `approval_required` webhook
9. Returns `ActionResponse`

### Executor Processing

1. `spawn_executor` starts background loop
2. Every `poll_interval_ms`, calls `db.lock_ready_actions(batch_size)`
3. For each locked action:
   - Calls `ToolExecutor::execute(tool, params)`
   - On success: `db.complete_action_execution(id, result, None)`
   - On failure: `db.complete_action_execution(id, None, error)`
   - Queues appropriate webhook

---

## Testing the Code

NCHE has comprehensive test coverage with 125 unit tests and 55+ integration tests.

### Unit Tests

```bash
cd backend/nche

# Run all unit tests
cargo test

# Run specific test module
cargo test state_machine
cargo test policy_engine
cargo test api_key
cargo test webhook

# Check compilation
cargo check
```

### Integration Tests

Integration tests require a PostgreSQL database. They test the full stack including database operations, API endpoints, and webhook delivery.

```bash
# Set up test database
export TEST_DATABASE_URL="postgres://postgres:postgres@localhost:5432/nche_test"
createdb nche_test

# Run all integration tests
cargo test --test db_test --test api_test --test webhook_test

# Run specific integration test file
cargo test --test db_test          # Database operations
cargo test --test api_test         # API endpoints
cargo test --test webhook_test     # Webhook delivery

# Run a specific test by name
cargo test --test api_test test_create_session
```

### Test File Structure

```
backend/nche/tests/
├── common/
│   └── mod.rs          # Test helpers: TestContext, fixtures, request builders
├── db_test.rs          # Database integration tests (20+ tests)
├── api_test.rs         # API integration tests (20+ tests)
└── webhook_test.rs     # Webhook delivery tests (15+ tests)
```

### TestContext

The `TestContext` struct in `tests/common/mod.rs` provides:

```rust
pub struct TestContext {
    pub db: Arc<Database>,      // Database connection
    pub tenant: Tenant,         // Pre-created test tenant
    pub agent: Agent,           // Pre-created test agent
    pub api_key: String,        // Valid API key for agent
    pub router: Router,         // Configured Axum router
}

// Usage in tests:
#[tokio::test]
async fn test_something() {
    let ctx = TestContext::new().await;

    // Make authenticated API request
    let (status, json) = ctx.agent_request(
        Method::POST,
        "/v1/sessions",
        Some(json!({"actor_id": "test", "actor_type": "user", "autonomy_level": "full"}))
    ).await;

    // Clean up test data
    ctx.cleanup().await;
}
```

---

## Quick Reference

| File | Lines | Purpose |
|------|-------|---------|
| `domain/mod.rs` | ~650 | Core types + entity structs |
| `api/mod.rs` | ~1100 | HTTP layer + PaginatedResponse |
| `api/auth.rs` | ~230 | Auth + CSRF middleware |
| `api/records.rs` | ~580 | Native records handlers |
| `db/*.rs` | ~1400 total | Database ops |
| `db/tasks.rs` | ~250 | Tasks CRUD |
| `db/cases.rs` | ~280 | Cases CRUD |
| `db/documents.rs` | ~180 | Documents CRUD |
| `db/links.rs` | ~200 | Links CRUD |
| `executor/mod.rs` | ~200 | Background executor |
| `tools/mod.rs` | ~510 | Tool trait, registry, implementations |
| `webhooks/mod.rs` | ~300 | Webhook delivery |
| `policy/*.rs` | ~150 total | Policy rules |
| `config.rs` | ~150 | YAML config loading |
| `main.rs` | ~480 | CLI |

### Test Files

| File | Tests | Purpose |
|------|-------|---------|
| `tests/common/mod.rs` | - | TestContext, fixtures, request helpers |
| `tests/db_test.rs` | 20+ | Tenant, agent, session, action, approval, event CRUD |
| `tests/api_test.rs` | 20+ | Authentication, sessions, actions, approvals, records |
| `tests/webhook_test.rs` | 15+ | Webhook delivery, retry logic, exponential backoff |
