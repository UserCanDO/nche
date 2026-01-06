"""
Nche Tenant Webhook Server Example

This Flask app demonstrates how to implement:
1. Execution webhook endpoint (receive actions to execute)
2. Policy webhook endpoint (optional custom policy logic)
3. Notification webhook endpoint (receive event notifications)

Run with: flask run --port 5001
Configure Nche: execution_webhook_url = "http://localhost:5001/nche/execute"
"""

import hashlib
import hmac
import os
import time
from functools import wraps

import requests
from flask import Flask, jsonify, request

app = Flask(__name__)

# Configuration
WEBHOOK_SECRET = os.getenv("NCHE_WEBHOOK_SECRET", "your-webhook-secret")
POLICY_WEBHOOK_SECRET = os.getenv("NCHE_POLICY_SECRET", "your-policy-secret")
NCHE_API_URL = os.getenv("NCHE_API_URL", "http://localhost:3000")
NCHE_API_KEY = os.getenv("NCHE_API_KEY", "your-api-key")


# =============================================================================
# Signature Verification
# =============================================================================

def verify_signature(secret: str):
    """Decorator to verify Nche webhook signatures."""
    def decorator(f):
        @wraps(f)
        def decorated_function(*args, **kwargs):
            timestamp = request.headers.get("X-Nche-Timestamp")
            signature = request.headers.get("X-Nche-Signature")

            if not timestamp or not signature:
                return jsonify({"error": "Missing signature headers"}), 401

            # Check timestamp is recent (within 5 minutes)
            try:
                ts = int(timestamp)
                if abs(time.time() - ts) > 300:
                    return jsonify({"error": "Timestamp too old"}), 401
            except ValueError:
                return jsonify({"error": "Invalid timestamp"}), 401

            # Verify HMAC signature
            body = request.get_data(as_text=True)
            message = f"{timestamp}.{body}"
            expected = hmac.new(
                secret.encode(),
                message.encode(),
                hashlib.sha256
            ).hexdigest()

            if not hmac.compare_digest(expected, signature):
                return jsonify({"error": "Invalid signature"}), 401

            return f(*args, **kwargs)
        return decorated_function
    return decorator


# =============================================================================
# Execution Webhook - Handle tool execution requests from Nche
# =============================================================================

@app.route("/nche/execute", methods=["POST"])
@verify_signature(WEBHOOK_SECRET)
def handle_execution():
    """
    Receive execution webhooks from Nche and execute tools.

    This is where you implement your actual tool logic.
    """
    data = request.json
    event_type = data.get("event_type")

    if event_type != "execute_action":
        return jsonify({"error": f"Unknown event type: {event_type}"}), 400

    action = data.get("action", {})
    action_id = action.get("id")
    tool = action.get("tool")
    params = action.get("params", {})

    app.logger.info(f"Executing action {action_id}: {tool}")

    try:
        # Dispatch to tool handler
        result = execute_tool(tool, params)
        return jsonify({
            "success": True,
            "result": result
        })
    except NotImplementedError as e:
        return jsonify({
            "success": False,
            "error": f"Tool not implemented: {tool}"
        })
    except Exception as e:
        app.logger.error(f"Tool execution failed: {e}")
        return jsonify({
            "success": False,
            "error": str(e)
        })


def execute_tool(tool: str, params: dict) -> dict:
    """
    Execute a tool and return the result.

    This is where you implement your actual tool logic.
    Each tool should:
    1. Validate params
    2. Execute the operation
    3. Return a result dict
    """
    handlers = {
        "email_send": handle_email_send,
        "slack_message": handle_slack_message,
        "sms_send": handle_sms_send,
        "http_request": handle_http_request,
        "payment_charge": handle_payment_charge,
        "ticket_create": handle_ticket_create,
        "calendar_event_create": handle_calendar_event_create,
        "database_query": handle_database_query,
    }

    handler = handlers.get(tool)
    if not handler:
        raise NotImplementedError(f"Tool '{tool}' not implemented")

    return handler(params)


# =============================================================================
# Tool Implementations
# =============================================================================

def handle_email_send(params: dict) -> dict:
    """Send an email via your preferred provider (SendGrid, SES, etc.)"""
    to = params.get("to")
    subject = params.get("subject")
    body = params.get("body")

    # TODO: Replace with your actual email sending logic
    # Example with SendGrid:
    # import sendgrid
    # sg = sendgrid.SendGridAPIClient(api_key=os.environ.get('SENDGRID_API_KEY'))
    # message = Mail(from_email='noreply@yourapp.com', to_emails=to, subject=subject, html_content=body)
    # response = sg.send(message)

    app.logger.info(f"[MOCK] Sending email to {to}: {subject}")
    return {
        "message_id": f"msg_{int(time.time())}",
        "to": to,
        "status": "sent"
    }


def handle_slack_message(params: dict) -> dict:
    """Post a message to Slack."""
    channel = params.get("channel")
    text = params.get("text")

    # TODO: Replace with your Slack webhook or API call
    # import slack_sdk
    # client = slack_sdk.WebClient(token=os.environ["SLACK_TOKEN"])
    # response = client.chat_postMessage(channel=channel, text=text)

    app.logger.info(f"[MOCK] Posting to Slack #{channel}: {text[:50]}...")
    return {
        "channel": channel,
        "ts": str(time.time()),
        "status": "posted"
    }


def handle_sms_send(params: dict) -> dict:
    """Send an SMS via Twilio or similar."""
    to = params.get("to")
    body = params.get("body")

    # TODO: Replace with your SMS provider
    # from twilio.rest import Client
    # client = Client(account_sid, auth_token)
    # message = client.messages.create(to=to, from_="+1234567890", body=body)

    app.logger.info(f"[MOCK] Sending SMS to {to}: {body[:50]}...")
    return {
        "sid": f"SM{int(time.time())}",
        "to": to,
        "status": "sent"
    }


def handle_http_request(params: dict) -> dict:
    """Make an HTTP request."""
    method = params.get("method", "GET")
    url = params.get("url")
    headers = params.get("headers", {})
    body = params.get("body")

    response = requests.request(
        method=method,
        url=url,
        headers=headers,
        json=body if body else None,
        timeout=30
    )

    return {
        "status_code": response.status_code,
        "headers": dict(response.headers),
        "body": response.text[:1000]  # Truncate large responses
    }


def handle_payment_charge(params: dict) -> dict:
    """Charge a payment via Stripe or similar."""
    amount_cents = params.get("amount_cents")
    currency = params.get("currency", "usd")
    customer_id = params.get("customer_id")
    description = params.get("description", "")

    # TODO: Replace with your payment provider
    # import stripe
    # stripe.api_key = os.environ["STRIPE_SECRET_KEY"]
    # charge = stripe.Charge.create(
    #     amount=amount_cents,
    #     currency=currency,
    #     customer=customer_id,
    #     description=description
    # )

    app.logger.info(f"[MOCK] Charging ${amount_cents/100:.2f} to customer {customer_id}")
    return {
        "charge_id": f"ch_{int(time.time())}",
        "amount_cents": amount_cents,
        "currency": currency,
        "status": "succeeded"
    }


def handle_ticket_create(params: dict) -> dict:
    """Create a support ticket in your system."""
    project = params.get("project")
    title = params.get("title")
    description = params.get("description")
    priority = params.get("priority", "medium")

    # TODO: Replace with your ticketing system (Zendesk, Jira, etc.)

    app.logger.info(f"[MOCK] Creating ticket in {project}: {title}")
    return {
        "ticket_id": f"TICKET-{int(time.time())}",
        "project": project,
        "title": title,
        "status": "open"
    }


def handle_calendar_event_create(params: dict) -> dict:
    """Create a calendar event."""
    title = params.get("title")
    start = params.get("start")
    end = params.get("end")
    attendees = params.get("attendees", [])

    # TODO: Replace with your calendar provider (Google Calendar, etc.)

    app.logger.info(f"[MOCK] Creating calendar event: {title}")
    return {
        "event_id": f"evt_{int(time.time())}",
        "title": title,
        "status": "confirmed"
    }


def handle_database_query(params: dict) -> dict:
    """Execute a database query (read-only recommended)."""
    connection_id = params.get("connection_id")
    query = params.get("query")
    query_params = params.get("params", [])

    # WARNING: Be very careful with database queries from agents
    # Only allow SELECT queries and use parameterized queries

    app.logger.info(f"[MOCK] Executing query on {connection_id}: {query[:50]}...")
    return {
        "rows": [],
        "row_count": 0,
        "query": query
    }


# =============================================================================
# Policy Webhook (Optional) - Custom policy decisions
# =============================================================================

@app.route("/nche/policy", methods=["POST"])
@verify_signature(POLICY_WEBHOOK_SECRET)
def handle_policy():
    """
    Custom policy evaluation endpoint.

    Return: { "decision": "allow" | "deny" | "require_approval", "reason": "..." }
    """
    data = request.json
    tool = data.get("tool")
    params = data.get("params", {})
    session = data.get("session", {})

    app.logger.info(f"Policy check for tool '{tool}'")

    # Example: Custom policy rules

    # Block certain email domains
    if tool == "email_send":
        to = params.get("to", "")
        blocked_domains = ["competitor.com", "spam.com"]
        if any(to.endswith(f"@{d}") for d in blocked_domains):
            return jsonify({
                "decision": "deny",
                "reason": f"Email to blocked domain: {to}"
            })

    # Require approval for large payments
    if tool == "payment_charge":
        amount = params.get("amount_cents", 0)
        if amount > 10000:  # > $100
            return jsonify({
                "decision": "require_approval",
                "reason": f"Payment amount ${amount/100:.2f} exceeds autonomous limit"
            })

    # Block dangerous database operations
    if tool == "database_query":
        query = params.get("query", "").upper()
        if any(kw in query for kw in ["DROP", "DELETE", "TRUNCATE", "ALTER"]):
            return jsonify({
                "decision": "deny",
                "reason": "Destructive database operations are blocked"
            })

    # Default: allow
    return jsonify({
        "decision": "allow",
        "reason": "Passed custom policy checks"
    })


# =============================================================================
# Notification Webhook - Receive event notifications
# =============================================================================

@app.route("/nche/events", methods=["POST"])
@verify_signature(WEBHOOK_SECRET)
def handle_notification():
    """
    Receive notification webhooks from Nche.

    Use these to update your UI, send alerts, etc.
    """
    data = request.json
    event_type = data.get("event_type")
    event_data = data.get("data", {})

    app.logger.info(f"Received notification: {event_type}")

    if event_type == "approval_required":
        # An action needs human approval
        action_id = event_data.get("action_id")
        tool = event_data.get("tool")
        reason = event_data.get("policy_reason")
        app.logger.info(f"Action {action_id} ({tool}) needs approval: {reason}")
        # TODO: Send Slack notification, update UI, etc.

    elif event_type == "action_approved":
        action_id = event_data.get("action_id")
        approver = event_data.get("approver_id")
        app.logger.info(f"Action {action_id} approved by {approver}")

    elif event_type == "action_denied":
        action_id = event_data.get("action_id")
        reason = event_data.get("note")
        app.logger.info(f"Action {action_id} denied: {reason}")

    elif event_type == "action_executed":
        action_id = event_data.get("action_id")
        app.logger.info(f"Action {action_id} executed successfully")

    elif event_type == "action_failed":
        action_id = event_data.get("action_id")
        error = event_data.get("error")
        app.logger.warning(f"Action {action_id} failed: {error}")

    return jsonify({"received": True})


# =============================================================================
# Async Execution Helper - For long-running operations
# =============================================================================

def report_result_async(action_id: str, success: bool, result: dict = None, error: str = None):
    """
    Report execution result back to Nche asynchronously.

    Use this for operations that take longer than the webhook timeout.
    """
    headers = {
        "Authorization": f"Bearer {NCHE_API_KEY}",
        "Content-Type": "application/json"
    }

    payload = {"success": success}
    if success:
        payload["result"] = result or {}
    else:
        payload["error"] = error or "Unknown error"

    response = requests.post(
        f"{NCHE_API_URL}/v1/actions/{action_id}/result",
        headers=headers,
        json=payload,
        timeout=10
    )
    response.raise_for_status()
    return response.json()


# =============================================================================
# Health Check
# =============================================================================

@app.route("/health", methods=["GET"])
def health():
    return jsonify({"status": "ok"})


if __name__ == "__main__":
    app.run(host="0.0.0.0", port=5001, debug=True)
