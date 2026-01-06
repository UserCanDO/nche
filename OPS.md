# NCHE Operations Guide

This document covers how to build, run, and operate the NCHE system.

## Prerequisites

- **Rust** (edition 2024) - Install via [rustup](https://rustup.rs/)
- **Node.js 18+** and **Bun** - For frontend development
- **PostgreSQL 14+** - Database

## Environment Setup

Create a `.env` file in `backend/nche/`:

```bash
DATABASE_URL=postgres://user:password@localhost:5432/nche
```

## Database Setup

```bash
# Create the database
createdb nche

# Run migrations (automatically run on server start, or manually)
cd backend/nche
cargo run -- migrate
```

## Quick Start

The fastest way to get started:

```bash
cd backend/nche

# Initialize: runs migrations + creates tenant, agent, and dashboard user
cargo run -- init

# Start the server
cargo run -- serve
```

This outputs:
- **Tenant ID** - Your tenant identifier
- **Agent ID** - Your agent identifier
- **API Key** - Use this for agent API calls (save it!)
- **Dashboard credentials** - Email/password for the web UI

Access the dashboard at http://localhost:3000/

---

## Backend Operations

### Running the Server

```bash
cd backend/nche
cargo run -- serve
```

#### Server Options

| Flag | Env Var | Default | Description |
|------|---------|---------|-------------|
| `--host` | `SERVER_HOST` | `127.0.0.1` | Bind address |
| `--port` | `SERVER_PORT` | `3000` | Bind port |
| `--executor-disabled` | `EXECUTOR_DISABLED` | `false` | Disable action executor |
| `--executor-poll-interval-ms` | `EXECUTOR_POLL_INTERVAL_MS` | `1000` | Executor poll interval |
| `--executor-batch-size` | `EXECUTOR_BATCH_SIZE` | `10` | Max actions per poll |
| `--webhook-disabled` | `WEBHOOK_DISPATCHER_DISABLED` | `false` | Disable webhook dispatcher |
| `--webhook-poll-interval-ms` | `WEBHOOK_POLL_INTERVAL_MS` | `5000` | Webhook poll interval |
| `--webhook-batch-size` | `WEBHOOK_BATCH_SIZE` | `20` | Max webhooks per poll |
| `--webhook-max-retries` | `WEBHOOK_MAX_RETRIES` | `5` | Max webhook retry attempts |

#### Examples

```bash
# Production-like settings
cargo run -- serve --host 0.0.0.0 --port 8080

# Disable background services (for debugging)
cargo run -- serve --executor-disabled --webhook-disabled

# Faster executor polling
cargo run -- serve --executor-poll-interval-ms 500
```

### Building for Production

```bash
cd backend/nche
cargo build --release

# Binary is at target/release/nche
./target/release/nche serve
```

The release binary embeds:
- Dashboard UI (static files via rust-embed)
- Database migrations (via sqlx)

### Cross-Compilation (Static Linux Binaries)

NCHE can be cross-compiled to produce fully static Linux binaries using musl. These binaries have zero external dependencies and run on any Linux distribution.

#### Prerequisites

```bash
# Install cross (handles toolchains via Docker)
cargo install cross

# Docker is required for cross-compilation
docker --version
```

#### Using the Build Script

```bash
cd backend/nche

# Build for current platform
./scripts/build-release.sh

# Cross-compile for Linux x86_64 (most servers)
./scripts/build-release.sh linux-x64

# Cross-compile for Linux ARM64 (AWS Graviton, Apple Silicon VMs)
./scripts/build-release.sh linux-arm64

# Build all platforms
./scripts/build-release.sh all
```

#### Manual Cross-Compilation

```bash
cd backend/nche

# Add target
rustup target add x86_64-unknown-linux-musl

# Build with cross
cross build --release --target x86_64-unknown-linux-musl

# Binary location
ls -la target/x86_64-unknown-linux-musl/release/nche
```

#### SQLx Offline Mode

For cross-compilation without a live database, prepare SQLx offline data:

```bash
# Install sqlx-cli
cargo install sqlx-cli

# Generate offline data (requires DATABASE_URL)
cargo sqlx prepare

# This creates .sqlx/ directory with query metadata
# Commit this to version control for CI/CD
```

#### Output Binaries

| Target | Binary Location | Notes |
|--------|-----------------|-------|
| Native | `target/release/nche` | Current platform |
| Linux x64 | `target/x86_64-unknown-linux-musl/release/nche` | Static, ~15-20MB |
| Linux ARM64 | `target/aarch64-unknown-linux-musl/release/nche` | Static, ~15-20MB |

### Health Check

```bash
curl http://localhost:3000/health
# Returns: ok
```

---

## Frontend Operations

The frontend is a Next.js application that gets statically exported and embedded in the Rust binary. For development, you can run it separately.

### Development Mode

```bash
cd frontend

# Install dependencies
bun install

# Run dev server (hot reload)
bun run dev
```

Access at http://localhost:3000/

**Note:** In dev mode, set `NEXT_PUBLIC_API_URL` to point to your backend:

```bash
NEXT_PUBLIC_API_URL=http://localhost:3001 bun run dev
```

### Building for Production

```bash
cd frontend
bun run build
```

This creates a static export in `frontend/out/` which gets embedded in the Rust binary at compile time.

### Rebuilding the Embedded Dashboard

After making frontend changes:

```bash
# 1. Build the frontend
cd frontend
bun run build

# 2. Rebuild the backend (picks up new static files)
cd ../backend/nche
cargo build
```

---

## API Endpoints

### Agent API (`/v1/`)

Requires `Authorization: Bearer <api_key>` header.

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/v1/sessions` | Create a session |
| GET | `/v1/sessions/:id` | Get session details |
| DELETE | `/v1/sessions/:id` | End a session |
| POST | `/v1/actions` | Propose an action |
| GET | `/v1/actions/:id` | Get action status |
| GET | `/v1/actions` | List actions |

### Dashboard API (`/dashboard/`)

Requires session cookie (set via login).

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/dashboard/login` | Login |
| POST | `/dashboard/api/logout` | Logout |
| GET | `/dashboard/api/me` | Current user info |
| GET | `/dashboard/api/stats` | Dashboard statistics |
| GET | `/dashboard/api/approvals` | List pending approvals |
| GET | `/dashboard/api/approvals/:id` | Approval details |
| PATCH | `/dashboard/api/approvals/:id` | Approve/deny |
| GET | `/dashboard/api/actions` | List actions |
| GET | `/dashboard/api/actions/:id` | Action details |
| GET | `/dashboard/api/events` | Audit log |

---

## Monitoring

### Logs

The server uses `tracing` for structured logging. Control log level via `RUST_LOG`:

```bash
# Debug logging
RUST_LOG=nche=debug cargo run -- serve

# Trace logging (very verbose)
RUST_LOG=nche=trace cargo run -- serve

# Production (info only)
RUST_LOG=nche=info cargo run -- serve
```

### Key Log Messages

- `Starting server on <addr>` - Server started
- `Starting action executor` - Executor background task running
- `Starting webhook dispatcher` - Webhook dispatcher running
- `Executing action <id>` - Action being executed
- `Webhook delivered to <url>` - Successful webhook delivery

---

## Troubleshooting

### "Invalid API key" error

The API key format is `nche_agt_<id>_<secret>`. Ensure you're using the full key returned by `nche init` or when creating an agent.

### Dashboard shows "Failed to load"

1. Ensure you're logged in (visit `/login` first)
2. Check that the backend is running
3. Verify the frontend was built with correct `baseUrl` (should be empty for embedded mode)

### Actions stuck in "paused_for_approval"

This is expected behavior. Actions requiring approval wait until approved via:
- Dashboard UI
- `nche approvals approve <id>` CLI
- PATCH `/dashboard/api/approvals/:id` API

### Webhooks not being sent

1. Check tenant has `webhook_url` configured
2. Verify webhook dispatcher is enabled (`--webhook-disabled` not set)
3. Check logs for webhook errors
4. Failed webhooks retry with exponential backoff (up to 5 times)

---

## Database Management

### Backup

```bash
pg_dump nche > nche_backup.sql
```

### Restore

```bash
psql nche < nche_backup.sql
```

### Reset (Development)

```bash
dropdb nche
createdb nche
cargo run -- migrate
cargo run -- init
```

---

## Testing

NCHE includes comprehensive test coverage with 125 unit tests and 55+ integration tests.

### Running Unit Tests

```bash
cd backend/nche

# Run all unit tests
cargo test

# Run specific test module
cargo test state_machine       # State machine transitions
cargo test policy_engine       # Policy evaluation
cargo test api_key             # API key generation/parsing
cargo test webhook             # Webhook signatures/filtering
cargo test tool                # Tool registry and validation
cargo test email_policy        # Email policy (blocked domains, internal)

# Run with output
cargo test -- --nocapture
```

### Running Integration Tests

Integration tests require a PostgreSQL database and test the full stack.

```bash
# 1. Create test database
createdb nche_test

# 2. Set environment variable
export TEST_DATABASE_URL="postgres://postgres:postgres@localhost:5432/nche_test"

# 3. Run all integration tests
cargo test --test db_test --test api_test --test webhook_test

# 4. Run specific test file
cargo test --test db_test          # Database CRUD operations
cargo test --test api_test         # API endpoints (auth, sessions, actions)
cargo test --test webhook_test     # Webhook delivery and retry logic

# 5. Run specific test by name
cargo test --test api_test test_create_session
cargo test --test db_test test_action_state_transitions
```

### Test Categories

| Test File | Tests | Coverage |
|-----------|-------|----------|
| `tests/db_test.rs` | 20+ | Tenant, agent, session, action, approval, event, task, case, document, link CRUD |
| `tests/api_test.rs` | 20+ | Authentication, session API, action API, approval API, records API |
| `tests/webhook_test.rs` | 15+ | Webhook delivery, status updates, retry logic, exponential backoff |

### Test Database Isolation

Each test creates its own tenant and cleans up after itself:

```rust
#[tokio::test]
async fn test_example() {
    let ctx = TestContext::new().await;  // Creates isolated tenant, agent, API key

    // Run test using ctx.db, ctx.tenant, ctx.agent, ctx.api_key, ctx.router

    ctx.cleanup().await;  // Removes all test data
}
```

### Continuous Integration

For CI environments:

```bash
# Run all tests with single command
export TEST_DATABASE_URL="postgres://postgres:postgres@localhost:5432/nche_test"
cargo test --all-targets

# Just unit tests (no database needed)
cargo test --lib
```

---

## Email Policy Configuration

NCHE provides fine-grained control over email policy through blocked domains and internal domain detection.

### Blocked Email Domains

Configure globally blocked email domains in `nche.yaml`. Emails to these domains are **always denied**, regardless of autonomy level:

```yaml
policy:
  blocked_email_domains:
    - competitor.com
    - banned-domain.org
    - "*.gov"           # Wildcard: blocks all .gov domains
```

When an agent tries to send an email to a blocked domain, the action is denied with a clear message:
```
Email to blocked domain 'competitor.com' is not permitted
```

### Internal Domain Detection

Configure internal domains per-tenant to allow emails to internal addresses to be auto-approved in supervised mode.

**Set via API or database:**
```sql
UPDATE tenants
SET internal_domains = '["acme.com", "acme.io", "*.acme-internal.net"]'
WHERE id = 'ten_xxx';
```

**Behavior by autonomy level:**

| Autonomy Level | Internal Email | External Email |
|----------------|----------------|----------------|
| Full           | Allowed        | Allowed        |
| Supervised     | Auto-approved  | Requires approval |
| Restricted     | Requires approval | Requires approval |

**Wildcards:** Use `*.domain.com` to match all subdomains (e.g., `*.acme.com` matches `hr.acme.com`).

**Priority:** Blocked domains take precedence over internal domains. If a domain is both internal and blocked, emails to it will be denied.

---

## Extending NCHE

### Registering Custom Tools

NCHE uses a trait-based tool system. To add a custom tool:

#### 1. Implement the `Tool` trait

```rust
use async_trait::async_trait;
use nche::tools::{Tool, ToolResult};
use nche::error::Result;

pub struct MyCustomTool {
    // Add any dependencies your tool needs
}

#[async_trait]
impl Tool for MyCustomTool {
    fn name(&self) -> &'static str {
        "my_custom_tool"
    }

    fn validate(&self, params: &serde_json::Value) -> Result<()> {
        // Validate parameters before execution
        // Return Err(NcheError::BadRequest { message }) for invalid params
        Ok(())
    }

    async fn execute(&self, params: serde_json::Value) -> Result<ToolResult> {
        // Execute the tool and return result
        Ok(ToolResult {
            success: true,
            data: Some(serde_json::json!({"result": "done"})),
            error: None,
        })
    }
}
```

#### 2. Register with the ToolRegistry

```rust
use std::sync::Arc;
use nche::tools::{ToolRegistry, ToolExecutor};

// Create registry with defaults and add your tool
let mut registry = ToolRegistry::default();
registry.register(Arc::new(MyCustomTool::new()));

// Create executor with custom registry
let executor = ToolExecutor::with_registry(registry);

// Validate params without executing
executor.validate("my_custom_tool", &params)?;

// Execute (validates automatically)
let result = executor.execute("my_custom_tool", params).await?;
```

#### 3. Add policy rules (optional)

Create a policy evaluator in `src/policy/` for your tool:

```rust
// src/policy/my_custom.rs
use crate::domain::AutonomyLevel;
use crate::policy::PolicyDecision;

pub fn evaluate(params: &serde_json::Value, autonomy: AutonomyLevel) -> PolicyDecision {
    match autonomy {
        AutonomyLevel::Full => PolicyDecision::allow("Full autonomy"),
        AutonomyLevel::Supervised => PolicyDecision::require_approval("Requires review"),
        AutonomyLevel::Restricted => PolicyDecision::require_approval("Restricted mode"),
    }
}
```

Then register it in `src/policy/mod.rs`:

```rust
pub fn evaluate(session: &Session, action: &Action) -> PolicyDecision {
    match action.tool.as_str() {
        "send_email" => email::evaluate(&action.params, session.autonomy_level),
        "http_request" => http::evaluate(&action.params, session.autonomy_level),
        "my_custom_tool" => my_custom::evaluate(&action.params, session.autonomy_level),
        _ => PolicyDecision::require_approval("Unknown tool"),
    }
}
```

### Built-in Tools

| Tool | Description | Validation |
|------|-------------|------------|
| `send_email` | Send email via configurable provider | Email format, non-empty subject |
| `http_request` | HTTP requests with configurable timeout | Valid URL, valid HTTP method |

### Configuring Email Providers

The `send_email` tool supports multiple email providers:

#### Console Provider (Default - Development)

```rust
use nche::tools::SendEmailTool;

// Default: logs emails to console (no actual sending)
let tool = SendEmailTool::default();
```

#### SMTP Provider

```rust
use nche::tools::{SendEmailTool, SmtpConfig, SmtpEmailProvider};
use std::sync::Arc;

let config = SmtpConfig::new("smtp.example.com", 587, "noreply@example.com")
    .with_credentials("username", "password")
    .with_from_name("NCHE")
    .with_tls(true);

let provider = SmtpEmailProvider::new(config)?;
let tool = SendEmailTool::new(Arc::new(provider));
```

#### SendGrid Provider

```rust
use nche::tools::{SendEmailTool, SendGridConfig, SendGridEmailProvider};
use std::sync::Arc;

let config = SendGridConfig::new("your-sendgrid-api-key", "noreply@example.com")
    .with_from_name("NCHE");

let provider = SendGridEmailProvider::new(config);
let tool = SendEmailTool::new(Arc::new(provider));
```

#### Mailgun Provider

```rust
use nche::tools::{SendEmailTool, MailgunConfig, MailgunEmailProvider, MailgunRegion};
use std::sync::Arc;

let config = MailgunConfig::new("your-mailgun-api-key", "mg.example.com", "noreply@example.com")
    .with_from_name("NCHE")
    .with_region(MailgunRegion::EU); // or MailgunRegion::US (default)

let provider = MailgunEmailProvider::new(config);
let tool = SendEmailTool::new(Arc::new(provider));
```

#### Using Configuration Files

Email providers can also be configured via JSON/YAML:

```json
{
  "type": "smtp",
  "host": "smtp.example.com",
  "port": 587,
  "username": "user",
  "password": "pass",
  "from_address": "noreply@example.com",
  "from_name": "NCHE",
  "use_tls": true
}
```

```rust
use nche::tools::EmailProviderConfig;

let config: EmailProviderConfig = serde_json::from_str(json)?;
let provider = config.into_provider()?;
```

### Configuring HTTP Request Timeout

The `http_request` tool has a configurable timeout (default: 30 seconds):

```rust
use nche::tools::HttpRequestTool;
use std::time::Duration;

// Default timeout (30 seconds)
let tool = HttpRequestTool::default();

// Custom timeout
let tool = HttpRequestTool::default()
    .with_timeout_secs(60);  // 60 seconds

// Or with Duration
let tool = HttpRequestTool::default()
    .with_timeout(Duration::from_secs(120));
```

### Creating a Registry with Custom Configuration

```rust
use nche::tools::{ToolRegistry, EmailProviderConfig, ConsoleEmailProvider, EmailProvider};
use std::sync::Arc;
use std::time::Duration;

// With specific email provider and HTTP timeout
let email_provider: Arc<dyn EmailProvider> = Arc::new(ConsoleEmailProvider::new());
let registry = ToolRegistry::with_config(
    email_provider,
    reqwest::Client::new(),
    Some(Duration::from_secs(60)),  // Custom HTTP timeout
);
```

### Tool Result Format

All tools return a `ToolResult`:

```rust
pub struct ToolResult {
    pub success: bool,                    // Whether execution succeeded
    pub data: Option<serde_json::Value>,  // Result data (if any)
    pub error: Option<String>,            // Error message (if failed)
}
```
