# NCHE v0.1 Implementation TODO

## Legend
- [ ] Not started
- [x] Complete
- [~] Partial

---

## 1. Project Setup
- [x] Cargo.toml with dependencies
- [x] Module structure (domain, db, api, policy, tools, webhooks, executor)
- [x] Error types with axum IntoResponse
- [x] Domain types (IDs, enums, entity structs)
- [x] State machine transitions
- [x] Initial SQL migration
- [x] .env configuration
- [x] nche.yaml configuration file loading

---

## 2. Database Layer (`src/db/`)

### Tenant Operations
- [x] `create_tenant()`
- [x] `get_tenant()`
- [x] `list_tenants()`
- [x] `update_tenant()`

### Agent Operations
- [x] `create_agent_with_key()` - generate API key, hash secret
- [x] `get_agent_by_api_key()` - lookup by prefix, verify hash
- [x] `get_agent()`
- [x] `list_agents()`

### Session Operations
- [x] `create_session()`
- [x] `get_session()`
- [x] `end_session()`
- [x] `list_sessions()`

### Action Operations
- [x] `create_action()`
- [x] `get_action()` - tenant-scoped
- [x] `list_actions()` - with state filter, pagination
- [x] `update_action_state()`
- [x] `update_action_policy()`
- [x] `lock_ready_actions()` - atomic SELECT FOR UPDATE SKIP LOCKED
- [x] `complete_action_execution()`

### Approval Operations
- [x] `create_approval()`
- [x] `get_approval()`
- [x] `get_approval_by_action()`
- [x] `list_approvals()` - by status, tenant-scoped
- [x] `update_approval()` - approve/deny with note
- [x] `decide_approval()` - atomic transaction (approval + action state)
- [x] `count_pending_approvals()`

### Event Logging
- [x] `create_event()`
- [x] `list_events()` - by action_id, session_id, tenant
- [x] `get_action_events()` - timeline view

### Webhook Delivery
- [x] `create_webhook_delivery()`
- [x] `get_webhook_delivery()`
- [x] `get_ready_webhook_deliveries()` - for retry processing
- [x] `update_webhook_delivery_status()`
- [x] `add_webhook_attempt()`
- [x] `update_webhook_next_attempt()`
- [x] `mark_webhook_failed()`

### Dashboard Auth
- [x] `create_dashboard_user()`
- [x] `get_dashboard_user_by_email()`
- [x] `verify_dashboard_user()` - password verification
- [x] `create_dashboard_session()`
- [x] `get_dashboard_session()`
- [x] `delete_dashboard_session()`
- [x] `extend_dashboard_session()`
- [x] `cleanup_expired_sessions()`

### NCHE-Native Records
- [x] Tasks CRUD + archive/unarchive
- [x] Cases CRUD + archive/unarchive
- [x] Documents CRUD + archive/unarchive
- [x] Links CRUD + archive/unarchive

---

## 3. API Layer (`src/api/`)

### Authentication
- [x] `agent_auth_middleware` - extract API key from header, verify, inject AuthContext
- [x] `dashboard_auth_middleware` - verify session cookie, inject DashboardContext
- [x] `AgentAuthContext` extractor
- [x] `DashboardAuthContext` extractor
- [x] `csrf_protected_middleware` - require X-Session-Id header for mutations

### Agent API Routes (`/v1/`)
- [x] `POST /v1/sessions` - create session
- [x] `GET /v1/sessions/:id` - get session
- [x] `DELETE /v1/sessions/:id` - end session
- [x] `POST /v1/actions` - propose action → policy eval → state transition
- [x] `GET /v1/actions/:id` - get action with status metadata
- [x] `GET /v1/actions` - list actions with filters
- [x] `GET /v1/approvals` - list approvals
- [x] `PATCH /v1/approvals/:id` - approve/deny (atomic transaction)

### NCHE-Native Record Routes
- [x] `POST /v1/records/tasks` - create task
- [x] `GET /v1/records/tasks` - list tasks
- [x] `GET /v1/records/tasks/archived` - list archived tasks
- [x] `GET /v1/records/tasks/:id` - get task
- [x] `POST /v1/records/tasks/:id/archive` - archive task
- [x] `POST /v1/records/tasks/:id/unarchive` - unarchive task
- [x] `POST /v1/records/cases` - create case
- [x] `GET /v1/records/cases` - list cases
- [x] `GET /v1/records/cases/archived` - list archived cases
- [x] `GET /v1/records/cases/:id` - get case
- [x] `POST /v1/records/cases/:id/archive` - archive case
- [x] `POST /v1/records/cases/:id/unarchive` - unarchive case
- [x] `POST /v1/records/documents` - create document
- [x] `GET /v1/records/documents` - list documents
- [x] `GET /v1/records/documents/archived` - list archived documents
- [x] `GET /v1/records/documents/:id` - get document
- [x] `POST /v1/records/documents/:id/archive` - archive document
- [x] `POST /v1/records/documents/:id/unarchive` - unarchive document
- [x] `POST /v1/records/links` - create link
- [x] `GET /v1/records/links` - list links
- [x] `GET /v1/records/links/archived` - list archived links
- [x] `GET /v1/records/links/:id` - get link
- [x] `POST /v1/records/links/:id/archive` - archive link
- [x] `POST /v1/records/links/:id/unarchive` - unarchive link

### Dashboard API Routes (`/dashboard/`)

#### Core (Complete)
- [x] `POST /dashboard/login`
- [x] `POST /dashboard/api/logout`
- [x] `GET /dashboard/api/approvals` - list pending
- [x] `GET /dashboard/api/approvals/:id` - detail view with action & events
- [x] `PATCH /dashboard/api/approvals/:id` - approve/deny (atomic transaction)
- [x] `GET /dashboard/api/events` - event log with filters
- [x] `GET /dashboard/api/me` - current user info

#### Management (Complete)
- [x] `GET /dashboard/api/agents` - list agents
- [x] `POST /dashboard/api/agents` - create agent (returns API key)
- [x] `GET /dashboard/api/sessions` - list sessions
- [x] `GET /dashboard/api/actions` - list actions (broader than approvals)
- [x] `GET /dashboard/api/actions/:id` - action detail with approval and events
- [x] `GET /dashboard/api/stats` - dashboard overview stats

### Request/Response Types
- [x] CreateSessionRequest / SessionResponse
- [x] CreateActionRequest / ActionResponse
- [x] ActionStatusResponse (with polling metadata)
- [x] ApprovalUpdateRequest
- [x] ApprovalDetail (with action and events)
- [x] LoginRequest / LoginResponse
- [x] PaginatedResponse<T>

---

## 4. Core Services

### Action Executor (`src/executor/mod.rs`)
- [x] ExecutorConfig with configurable parameters
- [x] Poll loop for ready_to_execute actions
- [x] Lock actions atomically (SKIP LOCKED)
- [x] Execute tool (dispatch to ToolExecutor)
- [x] Update action state (executed/failed)
- [x] Record result in database
- [x] Queue webhook on execution complete
- [x] Concurrent execution of locked actions
- [x] Configurable via CLI (poll interval, batch size, enable/disable)

### Tool Registry (`src/tools/mod.rs`)
- [x] ToolExecutor with execute() dispatch
- [x] ToolResult struct (success, data, error)
- [x] send_email implementation (mock/dry-run)
- [x] http_request implementation (full HTTP client)
- [x] Tool trait with validate() method
- [x] ToolRegistry with register/get
- [x] Configurable email provider (console/SMTP/SendGrid/Mailgun)
- [x] HTTP request timeout configuration

### Webhook Dispatcher (`src/webhooks/mod.rs`)
- [x] WebhookSender with HMAC-SHA256 signing
- [x] WebhookDispatcher background service
- [x] WebhookDispatcherConfig with configurable parameters
- [x] Poll pending deliveries loop
- [x] Exponential backoff (60s, 120s, 240s, 480s, 960s)
- [x] Max 5 retries (configurable)
- [x] `queue_webhook()` helper function
- [x] Tenant webhook_events filtering
- [x] Configurable via CLI (poll interval, batch size, max retries)

### Webhook Events
- [x] `approval_required` - queued on action requiring approval
- [x] `action_approved` - queued on approval
- [x] `action_denied` - queued on denial
- [x] `action_executed` - queued on successful execution
- [x] `action_failed` - queued on failed execution

---

## 5. Policy Engine (`src/policy/`)

- [x] PolicyEngine dispatcher
- [x] PolicyDecision with allow/deny/require_approval helpers
- [x] Tool-specific evaluators (modular)
- [x] Unknown tool → require_approval (safe default)
- [x] email.rs evaluator
- [x] http.rs evaluator (GET allowed in supervised, others need approval)
- [x] Blocked email domains list (configurable via nche.yaml)
- [x] Internal domain detection (per-tenant config via internal_domains)

---

## 6. Dashboard (`src/dashboard/` + `dashboard/`)

### Rust Serving
- [x] Add rust-embed dependency
- [x] `serve_dashboard()` handler
- [x] SPA fallback to index.html
- [x] MIME type detection

### React SPA (`frontend/`)
- [x] Next.js 16 project setup with TypeScript + Tailwind
- [x] RTK Query API client (`lib/api.ts`)
- [x] Redux store with StoreProvider
- [x] Pages:
  - [x] Login page (`/login`)
  - [x] Approvals list (`/approvals`) with filters and approve/deny dialogs
  - [x] Approval detail (`/approvals/[id]`) with params, timeline, decision box
  - [x] Actions list (`/actions`) with filters and pagination
  - [x] Action detail (`/actions/[id]`) with params, result, timeline
  - [x] Audit log (`/audit`) with filters and CSV export
- [x] Components:
  - [x] Shell layout with sidebar navigation
  - [x] ApprovalCard with safety preview
  - [x] ApproveDialog for confirm dialogs
  - [x] JsonViewer (collapsible JSON display)
  - [x] EventTimeline
  - [x] ToolBadge, ActionStatusBadge
- [x] shadcn/ui components (button, card, table, dialog, etc.)

---

## 7. CLI (`src/cli/`)

- [x] Basic serve command with configurable host/port
- [x] Executor configuration flags (--executor-disabled, --executor-poll-interval-ms, --executor-batch-size)
- [x] Webhook configuration flags (--webhook-disabled, --webhook-poll-interval-ms, etc.)
- [x] Basic migrate command
- [x] `nche init` - run migrations, create bootstrap tenant/agent/user
- [x] `nche tenants list`
- [x] `nche tenants create --name "..." --webhook-url "..."`
- [x] `nche approvals list --tenant --status` (full implementation)
- [x] `nche approvals approve <id> --approver --note`
- [x] `nche approvals deny <id> --approver --reason`
- [x] Configuration file loading (nche.yaml)

---

## 8. Testing

### Unit Tests (125 total)
- [x] State machine tests (4 tests)
- [x] Domain tests - ID types, ActionState, ToolName, WebhookEventType, enum serialization (19 tests)
- [x] API key tests - generation, parsing, edge cases, uniqueness (12 tests)
- [x] Policy engine tests - all autonomy levels, email/HTTP policies, unknown tools (14 tests)
- [x] Webhook tests - signature generation, event filtering, config (17 tests)
- [x] Error tests - message formatting, HTTP status code mapping (19 tests)
- [x] Executor config tests (1 test)
- [x] Config tests - YAML parsing, env overrides, defaults (4 tests)
- [x] Tool tests - registry, validation, SendEmailTool, HttpRequestTool, timeout config (17 tests)
- [x] Email provider tests - console, SMTP, SendGrid, Mailgun config builders (6 tests)
- [x] Email policy tests - blocked domains, internal domains, wildcard matching (8 tests)

### Integration Tests
- [x] `tests/common/mod.rs` - test helpers, fixtures
- [x] `tests/api_test.rs` - API integration tests
- [x] `tests/webhook_test.rs` - delivery/retry tests
- [x] `tests/db_test.rs` - database integration tests

---

## 9. Deployment (Single Binary)

- [x] Release build profile optimization (Cargo.toml)
- [x] Embed dashboard assets with rust-embed (zero external files)
- [x] Embed migrations with sqlx (compile-time verified)
- [x] Cross-compilation targets (linux-musl for static binary)
- [x] nche.yaml.example
- [x] .env.example
- [x] README.md with quickstart
- [ ] Dockerfile
- [ ] Install script or Homebrew formula (optional)

---

## 10. Examples

### Python Examples (`examples/python/`)
- [x] `nche_client.py` - Python NCHE API client
- [x] `agent_anthropic.py` - Agent using Anthropic Claude
- [x] `agent_openai.py` - Agent using OpenAI GPT

### Rust Examples (`examples/rust/`)
- [x] `src/nche_client.rs` - Rust NCHE API client
- [x] `src/agent_anthropic.rs` - Agent using Anthropic Claude
- [x] `src/agent_openai.rs` - Agent using OpenAI GPT

### Scenario Scripts
- [x] `examples/scenarios/email_approved.sh`
- [x] `examples/scenarios/email_denied.sh`
- [x] `examples/scenarios/full_workflow.sh`

---

## Priority Order for MVP

1. ~~**Database layer** - CRUD for tenants, agents, sessions, actions, approvals~~ ✅
2. ~~**Agent auth middleware** - API key verification~~ ✅
3. ~~**Core action flow** - create action → policy → state transition~~ ✅
4. ~~**Action executor** - poll and execute ready actions~~ ✅
5. ~~**Webhook dispatcher** - send notifications~~ ✅
6. ~~**Dashboard API** - login, list/approve/deny~~ ✅
7. ~~**Dashboard UI** - React SPA (embedded with rust-embed)~~ ✅
8. ~~**CLI commands** - init, tenant management~~ ✅
9. **Single binary release** - optimized build, embedded assets
10. **Tests and examples**

---

## Progress Summary

| Component | Status |
|-----------|--------|
| Database Layer | ✅ Complete |
| Auth Middleware | ✅ Complete |
| Core Action Flow | ✅ Complete |
| Action Executor | ✅ Complete |
| Webhook Dispatcher | ✅ Complete |
| Policy Engine | ✅ Complete |
| NCHE-Native Records | ✅ Complete (Tasks, Cases, Documents, Links + archive) |
| Dashboard API (Core) | ✅ Complete (login, approvals, audit) |
| Dashboard API (Management) | ✅ Complete (agents, sessions, actions, stats) |
| Dashboard UI | ✅ Complete (Next.js + RTK Query + shadcn/ui) |
| Dashboard Embedding | ✅ Complete (rust-embed, serve_dashboard, SPA fallback) |
| CLI (full) | ✅ Complete (init, tenants, approvals) |
| Testing | ✅ Complete (125 unit tests, 55+ integration tests) |
| Deployment | ~95% (Dockerfile TODO) |
| Examples | ✅ Complete (Python + Rust agents, scenario scripts) |
