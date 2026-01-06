# NCHE CLI Reference

Complete reference for the `nche` command-line interface.

## Installation

```bash
cd backend/nche
cargo build --release
# Binary at: target/release/nche
```

Or run directly with Cargo:
```bash
cargo run -- <command>
```

## Global Options

```
nche --help     Show help
nche --version  Show version
```

---

## Commands

### `nche serve`

Start the API server with embedded dashboard.

```bash
nche serve [OPTIONS]
```

#### Options

| Option | Environment Variable | Default | Description |
|--------|---------------------|---------|-------------|
| `--host <HOST>` | `SERVER_HOST` | `127.0.0.1` | Host address to bind |
| `--port <PORT>` | `SERVER_PORT` | `3000` | Port to bind |
| `--executor-disabled` | `EXECUTOR_DISABLED` | `false` | Disable background action executor |
| `--executor-poll-interval-ms <MS>` | `EXECUTOR_POLL_INTERVAL_MS` | `1000` | Executor polling interval |
| `--executor-batch-size <N>` | `EXECUTOR_BATCH_SIZE` | `10` | Max actions to process per poll |
| `--webhook-disabled` | `WEBHOOK_DISPATCHER_DISABLED` | `false` | Disable webhook dispatcher |
| `--webhook-poll-interval-ms <MS>` | `WEBHOOK_POLL_INTERVAL_MS` | `5000` | Webhook polling interval |
| `--webhook-batch-size <N>` | `WEBHOOK_BATCH_SIZE` | `20` | Max webhooks to send per poll |
| `--webhook-max-retries <N>` | `WEBHOOK_MAX_RETRIES` | `5` | Max webhook retry attempts |

#### Examples

```bash
# Start with defaults
nche serve

# Custom port
nche serve --port 8080

# Bind to all interfaces (for Docker/production)
nche serve --host 0.0.0.0 --port 8080

# Disable background services (debugging)
nche serve --executor-disabled --webhook-disabled

# Faster polling for development
nche serve --executor-poll-interval-ms 500 --webhook-poll-interval-ms 1000
```

#### What It Does

1. Runs database migrations
2. Starts the HTTP server
3. Spawns background action executor (unless disabled)
4. Spawns background webhook dispatcher (unless disabled)
5. Serves the embedded dashboard UI at `/`
6. Serves API endpoints at `/v1/*` and `/dashboard/*`

---

### `nche migrate`

Run database migrations.

```bash
nche migrate
```

Migrations are embedded in the binary and run automatically on `nche serve`, but this command lets you run them separately.

#### Example

```bash
nche migrate
# Output:
# ✓ Migrations complete
```

---

### `nche init`

Initialize NCHE with a tenant, agent, and dashboard user.

```bash
nche init [OPTIONS]
```

#### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--tenant-name <NAME>` | `Default Tenant` | Name for the bootstrap tenant |
| `--agent-name <NAME>` | `Default Agent` | Name for the bootstrap agent |
| `--user-email <EMAIL>` | `admin@localhost` | Email for dashboard user |
| `--user-password <PASS>` | `admin` | Password for dashboard user |
| `--webhook-url <URL>` | (none) | Optional webhook URL for tenant |

#### Examples

```bash
# Quick start with defaults
nche init

# Custom configuration
nche init \
  --tenant-name "Acme Corp" \
  --agent-name "Production Agent" \
  --user-email "admin@acme.com" \
  --user-password "secure123" \
  --webhook-url "https://acme.com/webhooks/nche"
```

#### Output

```
Initializing NCHE...

Running migrations...
✓ Migrations complete

Creating tenant: Acme Corp
✓ Tenant created: ten_xxxxxxxxxxxx

Creating agent: Production Agent
✓ Agent created: agt_xxxxxxxxxxxx
  API Key: nche_agt_xxxxxxxxxxxx_xxxxxxxxxxxxxxxxxxxxxxxx

  ⚠️  Save this API key - it cannot be retrieved later!

Creating dashboard user: admin@acme.com
✓ Dashboard user created: user_xxxxxxxxxxxx

═══════════════════════════════════════════════════════════
NCHE initialized successfully!
═══════════════════════════════════════════════════════════

Tenant ID:    ten_xxxxxxxxxxxx
Agent ID:     agt_xxxxxxxxxxxx
API Key:      nche_agt_xxxxxxxxxxxx_xxxxxxxxxxxxxxxxxxxxxxxx

Dashboard:    http://localhost:3000/
Login:        admin@acme.com / secure123

Start the server with: nche serve
═══════════════════════════════════════════════════════════
```

**Important:** Save the API key! It cannot be retrieved later.

---

### `nche tenants`

Tenant management commands.

#### `nche tenants list`

List all tenants.

```bash
nche tenants list [OPTIONS]
```

| Option | Default | Description |
|--------|---------|-------------|
| `--limit <N>` | `100` | Maximum tenants to show |

##### Example

```bash
nche tenants list

# Output:
# ID                   NAME                           WEBHOOK URL                              CREATED
# --------------------------------------------------------------------------------------------------------------
# ten_xxxxxxxxxxxx     Acme Corp                      https://acme.com/webhook                 2026-01-06T00:00:00Z
# ten_yyyyyyyyyyyy     Test Tenant                    -                                        2026-01-05T00:00:00Z
```

#### `nche tenants create`

Create a new tenant.

```bash
nche tenants create --name <NAME> [OPTIONS]
```

| Option | Required | Description |
|--------|----------|-------------|
| `--name <NAME>` | Yes | Tenant name |
| `--webhook-url <URL>` | No | Webhook URL for notifications |
| `--webhook-secret <SECRET>` | No | HMAC secret for signing webhooks |

##### Example

```bash
nche tenants create --name "New Customer" --webhook-url "https://example.com/hook"

# Output:
# ✓ Tenant created successfully!
#
#   ID:          ten_xxxxxxxxxxxx
#   Name:        New Customer
#   Webhook URL: https://example.com/hook
```

---

### `nche approvals`

Approval management commands.

#### `nche approvals list`

List approvals.

```bash
nche approvals list [OPTIONS]
```

| Option | Default | Description |
|--------|---------|-------------|
| `--tenant <ID>` | (all) | Filter by tenant ID |
| `--status <STATUS>` | `pending` | Filter by status: `pending`, `approved`, `denied`, `all` |
| `--limit <N>` | `50` | Maximum approvals to show |

##### Examples

```bash
# List pending approvals (default)
nche approvals list

# List all approvals
nche approvals list --status all

# List denied approvals for specific tenant
nche approvals list --tenant ten_xxxxxxxxxxxx --status denied

# Output:
# APPROVAL ID          ACTION ID            STATUS       TENANT               CREATED
# ----------------------------------------------------------------------------------------------------
# appr_xxxxxxxxxxxx    act_yyyyyyyyyyyy     pending      ten_zzzzzzzzzzzz     2026-01-06T00:00:00Z
```

#### `nche approvals approve`

Approve a pending action.

```bash
nche approvals approve <ID> --approver <NAME> [OPTIONS]
```

| Argument/Option | Required | Description |
|-----------------|----------|-------------|
| `<ID>` | Yes | Approval ID |
| `--approver <NAME>` | Yes | Approver identifier (email/username) |
| `--note <NOTE>` | No | Optional approval note |

##### Example

```bash
nche approvals approve appr_xxxxxxxxxxxx --approver "admin@example.com" --note "Approved per ticket #123"

# Output:
# ✓ Approval appr_xxxxxxxxxxxx approved by admin@example.com
```

#### `nche approvals deny`

Deny a pending action.

```bash
nche approvals deny <ID> --approver <NAME> --reason <REASON>
```

| Argument/Option | Required | Description |
|-----------------|----------|-------------|
| `<ID>` | Yes | Approval ID |
| `--approver <NAME>` | Yes | Approver identifier |
| `--reason <REASON>` | Yes | Reason for denial |

##### Example

```bash
nche approvals deny appr_xxxxxxxxxxxx --approver "admin@example.com" --reason "Recipient not authorized"

# Output:
# ✓ Approval appr_xxxxxxxxxxxx denied by admin@example.com: Recipient not authorized
```

---

## Environment Variables

The CLI respects these environment variables:

| Variable | Required | Description |
|----------|----------|-------------|
| `DATABASE_URL` | Yes | PostgreSQL connection string |
| `SERVER_HOST` | No | Default host for `serve` |
| `SERVER_PORT` | No | Default port for `serve` |
| `EXECUTOR_DISABLED` | No | Disable executor by default |
| `EXECUTOR_POLL_INTERVAL_MS` | No | Executor poll interval |
| `EXECUTOR_BATCH_SIZE` | No | Executor batch size |
| `WEBHOOK_DISPATCHER_DISABLED` | No | Disable webhooks by default |
| `WEBHOOK_POLL_INTERVAL_MS` | No | Webhook poll interval |
| `WEBHOOK_BATCH_SIZE` | No | Webhook batch size |
| `WEBHOOK_MAX_RETRIES` | No | Max webhook retries |
| `RUST_LOG` | No | Log level filter |

### Example `.env` File

```bash
DATABASE_URL=postgres://nche:password@localhost:5432/nche
SERVER_HOST=0.0.0.0
SERVER_PORT=8080
RUST_LOG=nche=info
```

---

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Error (invalid arguments, operation failed, etc.) |

---

## Common Workflows

### Fresh Setup

```bash
# 1. Create database
createdb nche

# 2. Initialize
nche init --tenant-name "My Company" --user-email "admin@mycompany.com"

# 3. Save the API key from output!

# 4. Start server
nche serve
```

### Adding a New Tenant

```bash
# Create tenant
nche tenants create --name "New Customer" --webhook-url "https://..."

# Create agent for tenant (via API or dashboard)
# Create dashboard user (via API or dashboard)
```

### Processing Approvals

```bash
# Check pending
nche approvals list

# Review and approve
nche approvals approve appr_xxx --approver "you@example.com"

# Or deny with reason
nche approvals deny appr_xxx --approver "you@example.com" --reason "Not authorized"
```

### Debugging

```bash
# Run with debug logging
RUST_LOG=nche=debug nche serve

# Disable background services to isolate issues
nche serve --executor-disabled --webhook-disabled

# Check database state
nche tenants list
nche approvals list --status all
```
