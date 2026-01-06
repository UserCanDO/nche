# Nche Webhook Integration Guide

This guide documents the webhook contracts for integrating with Nche. There are three types of webhooks:

1. **Execution Webhooks** - Nche tells your app to execute a tool
2. **Notification Webhooks** - Nche notifies you about events (approvals, completions, etc.)
3. **Policy Webhooks** (optional) - Your app makes policy decisions

## Architecture Overview

```
                                  Execution Webhook
Agent ─────▶ Nche ──────────────────────────────────────▶ Your App
       propose    policy check,                           executes
       action     approval flow                           tool
                       │
                       ▼
                  ┌─────────┐
                  │Dashboard│ (human approval if needed)
                  └─────────┘
                       │
                       ▼
              Your App reports result back
                       │
Agent ◀───── Nche ◀────┘
       result    audit log
```

---

## 1. Execution Webhooks

When an action is approved and ready to execute, Nche sends a webhook to your configured `execution_webhook_url`.

### Configuration

Set these via the API:

```bash
PATCH /v1/tenant/config
{
  "execution_webhook_url": "https://your-app.com/nche/execute",
  "execution_webhook_secret": "your-webhook-secret",
  "execution_webhook_timeout_ms": 30000
}
```

### Request Format

Nche sends a POST request to your execution webhook URL:

```http
POST /nche/execute HTTP/1.1
Host: your-app.com
Content-Type: application/json
X-Nche-Timestamp: 1704067200
X-Nche-Signature: a1b2c3d4e5f6...
```

```json
{
  "event_type": "execute_action",
  "timestamp": 1704067200,
  "action": {
    "id": "act_abc123",
    "session_id": "sess_xyz789",
    "tool": "email_send",
    "params": {
      "to": "customer@example.com",
      "subject": "Order Confirmation",
      "body": "Your order #1234 has shipped."
    }
  }
}
```

### Your Response

**For synchronous execution** (tool completes immediately):

Return HTTP 200 with a success response. Nche will immediately mark the action as executed.

```json
{
  "success": true,
  "result": {
    "message_id": "msg_12345",
    "delivered": true
  }
}
```

**For errors**:

```json
{
  "success": false,
  "error": "SMTP connection failed: timeout after 30s"
}
```

**For asynchronous execution** (tool takes time):

Return HTTP 202 to acknowledge receipt. Then call the result reporting API when done.

```json
{
  "acknowledged": true
}
```

### Timeout Behavior

If your webhook doesn't respond within `execution_webhook_timeout_ms` (default: 30s), Nche will:
1. Mark the action as `failed`
2. Record the timeout in the audit log
3. Send a notification webhook if configured

---

## 2. Result Reporting API

For async execution, report results back to Nche when your tool completes.

### Endpoint

```http
POST /v1/actions/{action_id}/result
Authorization: Bearer {api_key}
Content-Type: application/json
```

### Request Body

**Success:**

```json
{
  "success": true,
  "result": {
    "charge_id": "ch_abc123",
    "amount_cents": 5000,
    "currency": "usd"
  }
}
```

**Failure:**

```json
{
  "success": false,
  "error": "Payment declined: insufficient funds"
}
```

### Response

```json
{
  "id": "act_abc123",
  "state": "executed",
  "tool": "payment_charge",
  "params": { ... },
  "execution_result": {
    "charge_id": "ch_abc123"
  },
  "executed_by": "tenant_webhook"
}
```

### State Requirements

- Action must be in `pending_execution` state
- Only the tenant that owns the action can report results

---

## 3. Notification Webhooks

Nche can notify you about events via your tenant's `webhook_url`.

### Configuration

```bash
# Via CLI
nche tenants create --name "My App" --webhook-url "https://your-app.com/nche/events"

# Or via API
PATCH /dashboard/api/tenant/config
{
  "webhook_url": "https://your-app.com/nche/events",
  "webhook_secret": "notification-secret",
  "webhook_events": ["approval_required", "action_executed", "action_failed"]
}
```

### Event Types

| Event | Description |
|-------|-------------|
| `approval_required` | An action needs human approval |
| `action_approved` | An action was approved |
| `action_denied` | An action was denied |
| `action_executed` | An action completed successfully |
| `action_failed` | An action failed |

### Request Format

```http
POST /nche/events HTTP/1.1
Host: your-app.com
Content-Type: application/json
X-Nche-Timestamp: 1704067200
X-Nche-Signature: a1b2c3d4e5f6...
```

```json
{
  "event_type": "approval_required",
  "timestamp": 1704067200,
  "data": {
    "action_id": "act_abc123",
    "session_id": "sess_xyz789",
    "tool": "payment_charge",
    "params": {
      "amount_cents": 50000,
      "customer_id": "cus_123"
    },
    "policy_reason": "Amount exceeds $100 autonomous limit"
  }
}
```

---

## 4. Policy Webhooks (Optional)

For custom policy logic, configure Nche to delegate policy decisions to your webhook.

### Configuration

```bash
PATCH /v1/tenant/config
{
  "policy_mode": "webhook",
  "policy_webhook_url": "https://your-app.com/nche/policy",
  "policy_webhook_secret": "policy-secret",
  "policy_webhook_timeout_ms": 500
}
```

### Request Format

```http
POST /nche/policy HTTP/1.1
Host: your-app.com
Content-Type: application/json
X-Nche-Timestamp: 1704067200
X-Nche-Signature: a1b2c3d4e5f6...
```

```json
{
  "tool": "email_send",
  "params": {
    "to": "external@competitor.com",
    "subject": "Partnership Inquiry",
    "body": "..."
  },
  "session": {
    "id": "sess_xyz789",
    "autonomy_level": "supervised",
    "actor_id": "user_123",
    "actor_type": "user"
  },
  "agent": {
    "id": "agt_abc",
    "name": "Customer Support Bot"
  }
}
```

### Response Format

```json
{
  "decision": "allow",
  "reason": "Email to partner domain is allowed"
}
```

| Decision | Effect |
|----------|--------|
| `allow` | Action proceeds immediately |
| `deny` | Action is rejected |
| `require_approval` | Action waits for human approval |

### Timeout/Error Behavior

If your policy webhook times out or returns an error, Nche falls back to `require_approval` (fail-safe).

---

## 5. Signature Verification

All webhooks are signed with HMAC-SHA256. **Always verify signatures in production.**

### Signature Format

```
X-Nche-Signature: <hex-encoded HMAC-SHA256>
X-Nche-Timestamp: <unix timestamp>
```

The signature is computed over: `{timestamp}.{request_body}`

### Verification (Python)

```python
import hmac
import hashlib

def verify_signature(secret: str, timestamp: str, body: str, signature: str) -> bool:
    message = f"{timestamp}.{body}"
    expected = hmac.new(
        secret.encode(),
        message.encode(),
        hashlib.sha256
    ).hexdigest()
    return hmac.compare_digest(expected, signature)
```

### Verification (Node.js)

```javascript
const crypto = require('crypto');

function verifySignature(secret, timestamp, body, signature) {
  const message = `${timestamp}.${body}`;
  const expected = crypto
    .createHmac('sha256', secret)
    .update(message)
    .digest('hex');
  return crypto.timingSafeEqual(
    Buffer.from(expected),
    Buffer.from(signature)
  );
}
```

### Replay Attack Prevention

Reject requests with timestamps older than 5 minutes:

```python
import time

def is_timestamp_valid(timestamp: str, max_age_seconds: int = 300) -> bool:
    try:
        ts = int(timestamp)
        return abs(time.time() - ts) < max_age_seconds
    except ValueError:
        return False
```

---

## 6. Supported Tools

Nche includes 20 semantic tool schemas with built-in policies. Your execution webhook should handle these tool types:

### Communication
- `email_send` - Send emails
- `slack_message` - Post Slack messages
- `sms_send` - Send SMS
- `notification_push` - Push notifications

### HTTP/API
- `http_request` - Make HTTP requests
- `graphql_execute` - Execute GraphQL operations

### Calendar
- `calendar_event_create` - Create calendar events
- `calendar_event_cancel` - Cancel calendar events

### Files
- `file_upload` - Upload files
- `file_delete` - Delete files

### Database
- `database_query` - Execute database queries

### Ticketing
- `ticket_create` - Create tickets
- `ticket_update` - Update tickets
- `ticket_reply` - Reply to tickets

### Financial
- `payment_charge` - Charge payments
- `invoice_send` - Send invoices

### Documents
- `document_sign_request` - Request signatures
- `form_submit` - Submit forms

### Code/DevOps
- `git_issue_create` - Create GitHub issues
- `git_pr_merge` - Merge pull requests

Unknown tools are also supported - they require approval by default.

---

## 7. Complete Flow Example

```
1. Agent proposes action:
   POST /v1/actions { tool: "payment_charge", params: { amount_cents: 5000 } }

2. Nche evaluates policy:
   - Built-in: amount <= $100 + full_autonomy → allow
   - Or calls your policy webhook if configured

3. If approval required:
   - Action state → paused_for_approval
   - Notification webhook → approval_required
   - Human approves via dashboard
   - Action state → ready_to_execute

4. Nche sends execution webhook:
   POST your-app.com/nche/execute { tool: "payment_charge", ... }

5. Your app executes:
   stripe.Charge.create(amount=5000, ...)

6. Your app reports result:
   POST /v1/actions/{id}/result { success: true, result: { charge_id: "ch_xxx" } }

7. Nche updates state:
   - Action state → executed
   - Audit event created
   - Notification webhook → action_executed
```

---

## 8. Error Handling Best Practices

1. **Always return quickly** - Execution webhooks should respond within the timeout
2. **Use async for slow operations** - Return 202 and report results later
3. **Log everything** - Include action_id in your logs for debugging
4. **Handle retries** - Your webhook may be called multiple times (use idempotency keys)
5. **Verify signatures** - Never trust unsigned webhooks in production

---

## 9. Testing Webhooks

### Local Development

Use ngrok or similar to expose your local server:

```bash
ngrok http 5000
# Then configure: execution_webhook_url: "https://abc123.ngrok.io/nche/execute"
```

### Webhook Debugging

Check Nche's audit log for webhook delivery status:

```bash
# Via dashboard
GET /dashboard/api/events?action_id=act_xxx

# Or CLI
nche events list --action act_xxx
```
