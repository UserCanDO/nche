# Nche

**Nche** (Igbo: "watchman") is an open-source control plane for AI agents. It sits between your agent and the real world — enforcing policies, pausing for human approval, and maintaining an immutable audit trail.

```text
┌─────────────┐      ┌─────────────┐      ┌─────────────┐
│   AI Agent  │ ───▶ │    Nche     │ ───▶ │   Tools     │
│             │      │ (approve,   │      │ (email,     │
│             │ ◀─── │  audit,     │ ◀─── │  http, ...) │
│             │      │  execute)   │      │             │
└─────────────┘      └─────────────┘      └─────────────┘
```

## Why Nche?

AI agents are moving from "suggesting" to "doing" — sending emails, calling APIs, submitting forms. Without proper controls, that's terrifying.

Nche provides:

- **Policy enforcement** — Allow, deny, or require approval per tool/action
- **Human-in-the-loop** — Pause execution until a human approves
- **Webhooks** — Get notified when actions need attention
- **Audit trail** — Immutable log of every action, decision, and outcome
- **Multi-tenancy** — Isolate agents and approvers by organization
- **Web dashboard** — Approve actions with one click

Agents never touch tools or credentials directly. Only Nche does.

## Quick Start

### Option 1: From Source

```bash
# Clone the repo
git clone https://github.com/usercando/nche.git
cd nche/backend/nche

# Copy and configure environment
cp .env.example .env
# Edit .env with your PostgreSQL credentials

# Build release binary
cargo build --release

# Initialize database and create bootstrap tenant/agent/user
./target/release/nche init

# Start the server
./target/release/nche serve

# Open the dashboard
open http://localhost:3000
```

### Option 2: Docker Compose

```bash
# Clone the repo
git clone https://github.com/usercando/nche.git
cd nche

# Start with Docker
docker compose up

# Open the dashboard
open http://localhost:3000
```

## Configuration

NCHE can be configured via environment variables. See `.env.example` for all options.

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | (required) | PostgreSQL connection string |
| `SERVER_HOST` | `127.0.0.1` | Server bind address |
| `SERVER_PORT` | `3000` | Server port |
| `EXECUTOR_DISABLED` | `false` | Disable background action executor |
| `EXECUTOR_POLL_INTERVAL_MS` | `1000` | How often to poll for ready actions |
| `WEBHOOK_DISPATCHER_DISABLED` | `false` | Disable webhook delivery |
| `WEBHOOK_MAX_RETRIES` | `5` | Max webhook retry attempts |
| `RUST_LOG` | `info` | Log level (trace, debug, info, warn, error) |

## CLI Commands

```bash
# Initialize database and create bootstrap data
nche init

# Start the server
nche serve [--host 0.0.0.0] [--port 3000]
nche serve --executor-disabled    # Run without action executor
nche serve --webhook-disabled     # Run without webhook dispatcher

# Tenant management
nche tenants list
nche tenants create --name "My Company" --webhook-url "https://..."

# Approval management
nche approvals list [--tenant <ID>] [--status pending]
nche approvals approve <ID> --approver "admin" [--note "Approved"]
nche approvals deny <ID> --approver "admin" --reason "Not allowed"
```

## Testing

```bash
cd backend/nche

# Run all unit tests (125 tests)
cargo test

# Run specific unit test module
cargo test state_machine
cargo test policy_engine
cargo test api_key

# Run integration tests (requires PostgreSQL)
export TEST_DATABASE_URL="postgres://postgres:postgres@localhost:5432/nche_test"
createdb nche_test  # Create test database first

# Run all integration tests
cargo test --test db_test --test api_test --test webhook_test

# Run specific integration test file
cargo test --test db_test
cargo test --test api_test
cargo test --test webhook_test

# Run a specific test by name
cargo test --test api_test test_create_session
```

## Project Structure

```text
nche/
├── backend/nche/       # Rust API server (Axum, SQLx, Postgres)
├── frontend/           # Next.js dashboard (React, Redux, Tailwind, shadcn)
├── docker-compose.yml  # Full stack: backend + frontend + postgres
├── SPEC.md             # Technical specification
└── README.md           # You are here
```

## Architecture

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
│                         Nche                                    │
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

## How It Works

1. **Agent proposes an action** — "Send email to client@example.com"
2. **Nche evaluates policy** — Check autonomy level, tool rules, recipient
3. **Decision is made:**
   - `allow` → Execute immediately
   - `deny` → Reject with reason
   - `require_approval` → Pause and notify
4. **Human approves** (if needed) — Via dashboard, CLI, or Slack
5. **Nche executes** — Tool runs with stored credentials
6. **Result recorded** — Full audit trail preserved

## Documentation

- [Backend README](./backend/nche/README.md) — API server setup and development
- [Frontend README](./frontend/README.md) — Dashboard setup and development
- [SPEC.md](./SPEC.md) — Full technical specification

## API Overview

```bash
# Create a session
curl -X POST http://localhost:8080/v1/sessions \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"actor_id": "user_123", "actor_type": "user", "autonomy_level": "supervised"}'

# Propose an action
curl -X POST http://localhost:8080/v1/actions:propose \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"session_id": "sess_xxx", "tool": "send_email", "params": {"to": "client@example.com", "subject": "Hello", "body": "..."}}'

# Check action status
curl http://localhost:8080/v1/actions/act_xxx \
  -H "Authorization: Bearer $API_KEY"
```

## Tools (v0.1)

| Tool | Description |
|------|-------------|
| `send_email` | Send email (console mode in v0.1) |
| `http_request` | Make HTTP requests |

## Roadmap

- [ ] Policy DSL (configurable rules)
- [ ] MCP integration (Model Context Protocol)
- [ ] OpenTelemetry support
- [ ] Slack approvals
- [ ] Additional tools (calendar, file upload, Jira, etc.)

## Contributing

Contributions welcome. Please read the [SPEC.md](./SPEC.md) first to understand the architecture.

## License

MIT

---

*Nche: Your AI agents' watchman.*
