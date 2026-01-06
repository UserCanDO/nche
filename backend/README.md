# Nche Backend

Rust API server for Nche — the control plane for AI agents.

## Stack

- **Axum** — Web framework
- **SQLx** — Database (Postgres + SQLite)
- **Tokio** — Async runtime
- **Clap** — CLI
- **Argon2** — Password hashing
- **HMAC-SHA256** — Webhook signatures

## Prerequisites

- Rust 1.75+
- PostgreSQL 15+ (or SQLite for development)

## Quick Start

```bash
# Install dependencies
cargo build

# Set up environment
cp .env.example .env
# Edit .env with your database URL

# Run migrations
cargo run -- migrate

# Initialize with bootstrap data
cargo run -- init

# Start the server
cargo run -- serve
```

## Configuration

Create `nche.yaml` in the project root:

```yaml
server:
  host: 0.0.0.0
  port: 8080

database:
  url: postgres://nche:nche@localhost:5432/nche
  max_connections: 10

tenants:
  - id: ten_default
    name: Default Tenant
    webhook_url: ${WEBHOOK_URL}
    webhook_secret: ${WEBHOOK_SECRET}
    webhook_events:
      - approval_required
      - action_executed
      - action_failed

agents:
  - id: agent_demo
    tenant_id: ten_default
    name: Demo Agent
    api_key: ${AGENT_API_KEY}

dashboard_users:
  - email: admin@example.com
    password: ${DASHBOARD_PASSWORD}
    tenant_id: ten_default
    name: Admin User
```

Environment variables in `${VAR}` syntax are substituted at runtime.

## CLI Commands

```bash
# Start the API server
nche serve

# Start with dashboard disabled
nche serve --dashboard=false

# Initialize database schema
nche migrate

# Bootstrap tenants, agents, users from config
nche init

# List tenants
nche tenants list

# Create a tenant
nche tenants create --name "Acme Corp" --webhook-url "https://..."

# List pending approvals
nche approvals list --status pending

# Approve an action
nche approvals approve act_xxx --approver "admin@example.com" --note "Looks good"

# Deny an action
nche approvals deny act_xxx --approver "admin@example.com" --reason "Recipient not authorized"
```

## Project Structure

```text
src/
├── main.rs              # CLI entry point
├── lib.rs               # Library root
├── config.rs            # Configuration loading
├── error.rs             # Error types
├── db/
│   ├── mod.rs           # Database abstraction
│   ├── postgres.rs      # Postgres implementation
│   └── sqlite.rs        # SQLite implementation
├── domain/
│   ├── mod.rs           # Domain types (IDs, enums)
│   └── state_machine.rs # Action state transitions
├── api/
│   ├── mod.rs           # API module
│   ├── routes.rs        # Route definitions
│   ├── handlers.rs      # Request handlers
│   ├── types.rs         # Request/response types
│   └── auth.rs          # Authentication middleware
├── policy/
│   └── mod.rs           # Policy engine
├── tools/
│   ├── mod.rs           # Tool trait and registry
│   ├── email.rs         # send_email tool
│   └── http.rs          # http_request tool
├── webhooks/
│   └── mod.rs           # Webhook dispatcher
├── executor.rs          # Tool execution
├── events.rs            # Event logging
└── cli/
    ├── mod.rs           # CLI module
    ├── serve.rs         # serve command
    ├── init.rs          # init command
    ├── tenants.rs       # tenants commands
    └── approvals.rs     # approvals commands
```

## API Endpoints

### Agent API (API Key auth)

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/v1/sessions` | Create a session |
| GET | `/v1/sessions/:id` | Get session details |
| POST | `/v1/actions:propose` | Propose an action |
| GET | `/v1/actions/:id` | Get action status |
| GET | `/v1/actions/:id/events` | Get action audit trail |
| POST | `/v1/records/tasks` | Create a task |
| GET | `/v1/records/tasks/:id` | Get task |
| PATCH | `/v1/records/tasks/:id` | Update task |
| POST | `/v1/records/cases` | Create a case |
| GET | `/v1/records/cases/:id` | Get case |
| PATCH | `/v1/records/cases/:id` | Update case |

### Dashboard API (Session auth)

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/dashboard/login` | Login |
| POST | `/dashboard/logout` | Logout |
| GET | `/dashboard/approvals` | List approvals |
| GET | `/dashboard/approvals/:id` | Get approval |
| POST | `/dashboard/approvals/:id/approve` | Approve action |
| POST | `/dashboard/approvals/:id/deny` | Deny action |
| GET | `/dashboard/actions` | List actions |
| GET | `/dashboard/actions/:id` | Get action detail |
| GET | `/dashboard/audit` | Audit log |

### Health

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Health check |

## Development

```bash
# Run with auto-reload
cargo watch -x run -- serve

# Run tests
cargo test

# Run specific test
cargo test test_state_machine

# Check without building
cargo check

# Format code
cargo fmt

# Lint
cargo clippy
```

## Database

### Postgres (Production)

```bash
# Create database
createdb nche

# Connection string
DATABASE_URL=postgres://user:pass@localhost:5432/nche
```

### SQLite (Development)

```bash
# Connection string
DATABASE_URL=sqlite://./data/nche.db
```

### Feature Flags

```bash
# Build with Postgres (default)
cargo build

# Build with SQLite
cargo build --no-default-features --features sqlite

# Build with both
cargo build --features "postgres sqlite"
```

## Testing

```bash
# Unit tests
cargo test

# Integration tests (requires database)
DATABASE_URL=postgres://... cargo test --test api_test

# With coverage
cargo tarpaulin
```

## Docker

```bash
# Build
docker build -t nche-backend .

# Run
docker run -p 8080:8080 \
  -e DATABASE_URL=postgres://... \
  nche-backend
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_URL` | Database connection string | Required |
| `AGENT_API_KEY` | Bootstrap agent API key | Required for init |
| `DASHBOARD_PASSWORD` | Bootstrap admin password | Required for init |
| `WEBHOOK_URL` | Default webhook URL | Optional |
| `WEBHOOK_SECRET` | Webhook HMAC secret | Optional |
| `RUST_LOG` | Log level | `info` |

## License

MIT