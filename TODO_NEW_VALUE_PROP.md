# NCHE v1 Refactor: Governance Middleware

## Core Architectural Shift

**Current:** Nche executes tools directly (owns credentials, runs SendGrid/HTTP)
**Target:** Nche is pure middleware (tenant executes, Nche governs)

```
CURRENT FLOW:
Agent → Nche → Policy → Approval → Nche Executes Tool → Done

TARGET FLOW:
Agent → Nche → Policy → Approval → Webhook to Tenant → Tenant Executes → Result back to Nche → Audit
```

---

## Phase 1: Execution Webhook Model ✅ COMPLETE

### 1.1 Database Changes

- [x] Add `execution_webhook_url` to tenants table
- [x] Add `execution_webhook_secret` to tenants table (for signing)
- [x] Add `execution_webhook_timeout_ms` to tenants table (default 30000)
- [x] Add new ActionState: `pending_execution` (after approval, before tenant executes)
- [x] Add `execution_result` JSONB column to actions table
- [x] Add `executed_by` column to actions (tracks who/what executed: "tenant_webhook", "manual", etc.)

### 1.2 Execution Flow Changes

- [x] After approval, transition to `ready_to_execute` then `pending_execution`
- [x] POST to tenant's execution webhook:
  ```json
  {
    "event_type": "execute_action",
    "timestamp": 1234567890,
    "action": {
      "id": "act_xxx",
      "session_id": "sess_xxx",
      "tool": "payment_charge",
      "params": { "amount_cents": 5000, "customer_id": "cus_123" }
    }
  }
  ```
- [x] Sign webhook with HMAC-SHA256 (X-Nche-Signature header)
- [x] Handle async execution (tenant reports back via result endpoint)

### 1.3 Result Reporting Endpoint

- [x] `POST /v1/actions/:id/result` - Tenant reports execution result
  ```json
  {
    "success": true,
    "result": { "charge_id": "ch_xxx" },
    "error": null,
    "executed_by": "tenant_webhook"
  }
  ```
- [x] Validate action is in `pending_execution` state
- [x] Transition to `executed` or `failed`
- [x] Store result in `execution_result` column
- [x] Create audit event
- [x] Queue `action_executed` or `action_failed` webhook

### 1.4 Remove/Deprecate Direct Execution

- [x] Removed `ToolExecutor`, `SendEmailTool`, `HttpRequestTool`
- [x] Removed `tools` module entirely
- [x] Executor now dispatches webhooks instead of executing tools
- [ ] Update CLI flags: `--execution-mode=webhook|builtin` (future)

---

## Phase 2: 20 Semantic Tool Schemas ✅ COMPLETE

### 2.1 Tool Registry with Schemas

- [x] Define `ToolSchema` struct with validation rules (`src/policy/schema.rs`)
- [x] Create schema definitions for all 20 tools

**Communication (4):**
- [x] `email_send` - `{to, cc?, bcc?, subject, body, reply_to?}`
- [x] `slack_message` - `{channel, text, thread_ts?}`
- [x] `sms_send` - `{to, body}`
- [x] `notification_push` - `{user_id, title, body, data?}`

**HTTP/API (2):**
- [x] `http_request` - `{method, url, headers?, body?}`
- [x] `graphql_execute` - `{endpoint, query, variables?}`

**Calendar (2):**
- [x] `calendar_event_create` - `{title, start, end, attendees, description?}`
- [x] `calendar_event_cancel` - `{event_id, notify_attendees}`

**Files (2):**
- [x] `file_upload` - `{bucket, path, content_type, size_bytes}`
- [x] `file_delete` - `{bucket, path}`

**Database (1):**
- [x] `database_query` - `{connection_id, query, params?}`

**Ticketing (3):**
- [x] `ticket_create` - `{project, type, title, description, priority?, customer_visible?}`
- [x] `ticket_update` - `{ticket_id, status?, assignee?, comment?}`
- [x] `ticket_reply` - `{ticket_id, body, internal}`

**Financial (2):**
- [x] `payment_charge` - `{amount_cents, currency, customer_id, description?}`
- [x] `invoice_send` - `{invoice_id, recipient_email}`

**Documents (2):**
- [x] `document_sign_request` - `{document_id, signers, message?}`
- [x] `form_submit` - `{form_id, fields, submit_to}`

**Code/DevOps (2):**
- [x] `git_issue_create` - `{repo, title, body, labels?}`
- [x] `git_pr_merge` - `{repo, pr_number, method?}`

### 2.2 Policy Rules per Tool

- [x] `email_send`: internal domain = allow, external = approval, blocked = deny
- [x] `slack_message`: full autonomy = allow, supervised = approval
- [x] `sms_send`: full autonomy = allow, supervised = approval
- [x] `notification_push`: supervised = allow, restricted = approval
- [x] `http_request`: GET = allow, POST/PUT/PATCH/DELETE = approval, localhost = deny
- [x] `graphql_execute`: query = allow, mutation = approval
- [x] `calendar_event_create`: full autonomy = allow, supervised = approval
- [x] `calendar_event_cancel`: full autonomy = allow, supervised = approval
- [x] `file_upload`: private bucket + <100MB = allow, public/large = approval
- [x] `file_delete`: temp path = allow, permanent = approval
- [x] `database_query`: SELECT = allow, INSERT/UPDATE = approval, DELETE/DROP/TRUNCATE = deny
- [x] `ticket_create`: internal project = allow, customer-visible = approval
- [x] `ticket_update`: comment = allow, status change = approval, reassign = approval
- [x] `ticket_reply`: internal note = allow, customer-facing = approval
- [x] `payment_charge`: amount ≤ $100 + full autonomy = allow, else = approval
- [x] `invoice_send`: always require approval
- [x] `document_sign_request`: always require approval
- [x] `form_submit`: internal = allow, external/government = approval
- [x] `git_issue_create`: standard labels = allow, security/critical = approval
- [x] `git_pr_merge`: always require approval

### 2.3 Unknown Tool Handling

- [x] Universal fallback: unknown tool → require_approval
- [x] Custom tools allowed (policy applied, no schema validation)

---

## Phase 3: Policy Webhook Mode ✅ COMPLETE

### 3.1 Configuration

- [x] Add `policy_mode` to tenant: `builtin` | `webhook`
- [x] Add `policy_webhook_url` to tenant
- [x] Add `policy_webhook_secret` to tenant
- [x] Add `policy_webhook_timeout_ms` (default 500ms)
- [x] Migration: `005_policy_webhook.sql`

### 3.2 Webhook Policy Evaluation

- [x] `PolicyWebhookClient` in `src/policy/webhook.rs`
- [x] Request format:
  ```json
  {
    "tool": "payment_charge",
    "params": { "amount_cents": 50000 },
    "session": { "id": "sess_xxx", "autonomy_level": "supervised", "actor_id": "user_123", "actor_type": "user" },
    "agent": { "id": "agt_xxx", "name": "Support Bot" }
  }
  ```
- [x] Response format:
  ```json
  {
    "decision": "allow" | "deny" | "require_approval",
    "reason": "Amount exceeds $100 limit"
  }
  ```
- [x] HMAC-SHA256 signature (X-Nche-Signature header)
- [x] Timeout → fallback to `require_approval`
- [x] Error → fallback to `require_approval`
- [x] 7 new tests for webhook policy mode

---

## Phase 4: Configuration Updates ✅ COMPLETE

### 4.1 nche.yaml Structure

```yaml
server:
  host: 0.0.0.0
  port: 8080

database:
  url: postgres://nche:nche@localhost:5432/nche

# Default modes for new tenants
defaults:
  execution_mode: webhook  # or "builtin" for self-hosted
  policy_mode: builtin     # or "webhook" for custom policies

policy:
  blocked_email_domains:
    - competitor.com
```

- [x] Add `defaults.execution_mode` config (default: "webhook")
- [x] Add `defaults.policy_mode` config (default: "builtin")
- [x] Environment variable overrides: `DEFAULT_EXECUTION_MODE`, `DEFAULT_POLICY_MODE`
- [x] Config tests for defaults parsing and env overrides

### 4.2 Per-Tenant Overrides

- [x] Tenant can override execution_mode via execution webhook config
- [x] Tenant can override policy_mode
- [x] API endpoints to update tenant config:
  - `GET /v1/tenant/config` - Get tenant configuration (Agent API)
  - `PATCH /v1/tenant/config` - Update tenant configuration (Agent API)
  - `GET /dashboard/api/tenant/config` - Get tenant configuration (Dashboard API)
  - `PATCH /dashboard/api/tenant/config` - Update tenant configuration (Dashboard API)
- [x] Database function `update_tenant_config()` for atomic config updates

---

## Phase 5: Documentation & Examples ✅ COMPLETE

### 5.1 Tenant Implementation Guide

- [x] Document execution webhook contract (`docs/WEBHOOKS.md`)
- [x] Document result reporting API (`docs/WEBHOOKS.md`)
- [x] Document policy webhook contract (`docs/WEBHOOKS.md`)
- [x] Signature verification examples (Python + Node.js)

### 5.2 Example Tenant Implementations

- [x] Python Flask example with execution webhook (`examples/python/tenant_webhook_server.py`)
- [x] Node.js Express example (`examples/nodejs/tenant-webhook-server.js`)
- [x] Both examples include:
  - Execution webhook handler
  - Policy webhook handler (optional)
  - Notification webhook handler
  - Signature verification
  - Mock tool implementations

### 5.3 Self-Hosted Mode Docs

- [x] Document builtin execution mode (`docs/SELF_HOSTED.md`)
- [x] How to add custom tool implementations
- [x] Migration guides (webhook ↔ builtin)

---

## Migration Path

### For Existing Deployments

1. Add execution webhook columns (nullable)
2. Default `execution_mode=builtin` for existing tenants
3. New tenants default to `execution_mode=webhook`
4. Deprecation notice for builtin execution
5. Future: builtin execution behind feature flag

---

## What We Keep

- [x] Policy evaluation engine (refactor for 20 tools)
- [x] Approval workflow state machine
- [x] Audit trail / event logging
- [x] Dashboard UI
- [x] Webhook notifications (approval_required, action_approved, etc.)
- [x] Multi-tenancy
- [x] NCHE-Native Records (Tasks, Cases, Documents, Links)
- [x] CLI tools

---

## What We Remove/Deprecate

- [x] `ToolExecutor` direct execution (removed)
- [x] `SendEmailTool` implementation (removed)
- [x] `HttpRequestTool` implementation (removed)
- [x] Entire `tools` module (removed)
- [x] Executor now dispatches webhooks to tenant (refactored)
- [ ] Email provider configurations (SMTP, SendGrid, Mailgun) - N/A, never in core

---

## Priority Order

1. **Execution webhook model** (Phase 1) - Core value prop change
2. **20 tool schemas + policies** (Phase 2) - Differentiated governance
3. **Policy webhook mode** (Phase 3) - Extensibility
4. **Config & docs** (Phase 4-5) - Polish

---

## Success Criteria

Tenant experience:
```python
# This is ALL the tenant needs to build:
@app.post("/nche/execute")
def execute(req):
    match req.json["tool"]:
        case "payment_charge":
            charge = stripe.Charge.create(...)
            return {"success": True, "result": {"charge_id": charge.id}}
        case "email_send":
            sendgrid.send(...)
            return {"success": True}
        case _:
            return {"success": False, "error": f"Unknown tool"}
```

Nche provides:
- 20 semantic policies (no tenant code)
- Approval queue + dashboard
- Immutable audit trail
- Webhook notifications
- Multi-tenancy

Tenant builds:
- One webhook endpoint with a switch statement
