/**
 * Nche Tenant Webhook Server Example (Node.js/Express)
 *
 * This Express app demonstrates how to implement:
 * 1. Execution webhook endpoint (receive actions to execute)
 * 2. Policy webhook endpoint (optional custom policy logic)
 * 3. Notification webhook endpoint (receive event notifications)
 *
 * Run with: node tenant-webhook-server.js
 * Configure Nche: execution_webhook_url = "http://localhost:5001/nche/execute"
 */

const express = require('express');
const crypto = require('crypto');

const app = express();
app.use(express.json());

// Configuration
const WEBHOOK_SECRET = process.env.NCHE_WEBHOOK_SECRET || 'your-webhook-secret';
const POLICY_WEBHOOK_SECRET = process.env.NCHE_POLICY_SECRET || 'your-policy-secret';
const NCHE_API_URL = process.env.NCHE_API_URL || 'http://localhost:3000';
const NCHE_API_KEY = process.env.NCHE_API_KEY || 'your-api-key';

// =============================================================================
// Signature Verification Middleware
// =============================================================================

function verifySignature(secret) {
  return (req, res, next) => {
    const timestamp = req.headers['x-nche-timestamp'];
    const signature = req.headers['x-nche-signature'];

    if (!timestamp || !signature) {
      return res.status(401).json({ error: 'Missing signature headers' });
    }

    // Check timestamp is recent (within 5 minutes)
    const ts = parseInt(timestamp, 10);
    if (isNaN(ts) || Math.abs(Date.now() / 1000 - ts) > 300) {
      return res.status(401).json({ error: 'Timestamp too old or invalid' });
    }

    // Verify HMAC signature
    const body = JSON.stringify(req.body);
    const message = `${timestamp}.${body}`;
    const expected = crypto
      .createHmac('sha256', secret)
      .update(message)
      .digest('hex');

    try {
      if (!crypto.timingSafeEqual(Buffer.from(expected), Buffer.from(signature))) {
        return res.status(401).json({ error: 'Invalid signature' });
      }
    } catch (e) {
      return res.status(401).json({ error: 'Invalid signature' });
    }

    next();
  };
}

// =============================================================================
// Execution Webhook - Handle tool execution requests from Nche
// =============================================================================

app.post('/nche/execute', verifySignature(WEBHOOK_SECRET), async (req, res) => {
  const { event_type, action } = req.body;

  if (event_type !== 'execute_action') {
    return res.status(400).json({ error: `Unknown event type: ${event_type}` });
  }

  const { id: actionId, tool, params } = action;
  console.log(`Executing action ${actionId}: ${tool}`);

  try {
    const result = await executeTool(tool, params);
    return res.json({ success: true, result });
  } catch (error) {
    console.error(`Tool execution failed: ${error.message}`);
    return res.json({ success: false, error: error.message });
  }
});

async function executeTool(tool, params) {
  const handlers = {
    email_send: handleEmailSend,
    slack_message: handleSlackMessage,
    sms_send: handleSmsSend,
    http_request: handleHttpRequest,
    payment_charge: handlePaymentCharge,
    ticket_create: handleTicketCreate,
    calendar_event_create: handleCalendarEventCreate,
    database_query: handleDatabaseQuery,
  };

  const handler = handlers[tool];
  if (!handler) {
    throw new Error(`Tool '${tool}' not implemented`);
  }

  return handler(params);
}

// =============================================================================
// Tool Implementations
// =============================================================================

async function handleEmailSend(params) {
  const { to, subject, body } = params;

  // TODO: Replace with your actual email sending logic
  // Example with SendGrid:
  // const sgMail = require('@sendgrid/mail');
  // sgMail.setApiKey(process.env.SENDGRID_API_KEY);
  // await sgMail.send({ to, from: 'noreply@yourapp.com', subject, text: body });

  console.log(`[MOCK] Sending email to ${to}: ${subject}`);
  return {
    message_id: `msg_${Date.now()}`,
    to,
    status: 'sent',
  };
}

async function handleSlackMessage(params) {
  const { channel, text } = params;

  // TODO: Replace with your Slack API call
  // const { WebClient } = require('@slack/web-api');
  // const client = new WebClient(process.env.SLACK_TOKEN);
  // const response = await client.chat.postMessage({ channel, text });

  console.log(`[MOCK] Posting to Slack #${channel}: ${text.substring(0, 50)}...`);
  return {
    channel,
    ts: String(Date.now() / 1000),
    status: 'posted',
  };
}

async function handleSmsSend(params) {
  const { to, body } = params;

  // TODO: Replace with your SMS provider (Twilio, etc.)
  // const twilio = require('twilio');
  // const client = twilio(accountSid, authToken);
  // const message = await client.messages.create({ to, from: '+1234567890', body });

  console.log(`[MOCK] Sending SMS to ${to}: ${body.substring(0, 50)}...`);
  return {
    sid: `SM${Date.now()}`,
    to,
    status: 'sent',
  };
}

async function handleHttpRequest(params) {
  const { method = 'GET', url, headers = {}, body } = params;

  const response = await fetch(url, {
    method,
    headers: { 'Content-Type': 'application/json', ...headers },
    body: body ? JSON.stringify(body) : undefined,
  });

  const responseText = await response.text();
  return {
    status_code: response.status,
    headers: Object.fromEntries(response.headers.entries()),
    body: responseText.substring(0, 1000), // Truncate large responses
  };
}

async function handlePaymentCharge(params) {
  const { amount_cents, currency = 'usd', customer_id, description = '' } = params;

  // TODO: Replace with your payment provider (Stripe, etc.)
  // const stripe = require('stripe')(process.env.STRIPE_SECRET_KEY);
  // const charge = await stripe.charges.create({
  //   amount: amount_cents,
  //   currency,
  //   customer: customer_id,
  //   description,
  // });

  console.log(`[MOCK] Charging $${(amount_cents / 100).toFixed(2)} to customer ${customer_id}`);
  return {
    charge_id: `ch_${Date.now()}`,
    amount_cents,
    currency,
    status: 'succeeded',
  };
}

async function handleTicketCreate(params) {
  const { project, title, description, priority = 'medium' } = params;

  // TODO: Replace with your ticketing system (Zendesk, Jira, etc.)

  console.log(`[MOCK] Creating ticket in ${project}: ${title}`);
  return {
    ticket_id: `TICKET-${Date.now()}`,
    project,
    title,
    status: 'open',
  };
}

async function handleCalendarEventCreate(params) {
  const { title, start, end, attendees = [] } = params;

  // TODO: Replace with your calendar provider (Google Calendar, etc.)

  console.log(`[MOCK] Creating calendar event: ${title}`);
  return {
    event_id: `evt_${Date.now()}`,
    title,
    status: 'confirmed',
  };
}

async function handleDatabaseQuery(params) {
  const { connection_id, query, params: queryParams = [] } = params;

  // WARNING: Be very careful with database queries from agents
  // Only allow SELECT queries and use parameterized queries

  console.log(`[MOCK] Executing query on ${connection_id}: ${query.substring(0, 50)}...`);
  return {
    rows: [],
    row_count: 0,
    query,
  };
}

// =============================================================================
// Policy Webhook (Optional) - Custom policy decisions
// =============================================================================

app.post('/nche/policy', verifySignature(POLICY_WEBHOOK_SECRET), (req, res) => {
  const { tool, params, session } = req.body;
  console.log(`Policy check for tool '${tool}'`);

  // Example: Custom policy rules

  // Block certain email domains
  if (tool === 'email_send') {
    const to = params.to || '';
    const blockedDomains = ['competitor.com', 'spam.com'];
    if (blockedDomains.some(d => to.endsWith(`@${d}`))) {
      return res.json({
        decision: 'deny',
        reason: `Email to blocked domain: ${to}`,
      });
    }
  }

  // Require approval for large payments
  if (tool === 'payment_charge') {
    const amount = params.amount_cents || 0;
    if (amount > 10000) {
      // > $100
      return res.json({
        decision: 'require_approval',
        reason: `Payment amount $${(amount / 100).toFixed(2)} exceeds autonomous limit`,
      });
    }
  }

  // Block dangerous database operations
  if (tool === 'database_query') {
    const query = (params.query || '').toUpperCase();
    if (['DROP', 'DELETE', 'TRUNCATE', 'ALTER'].some(kw => query.includes(kw))) {
      return res.json({
        decision: 'deny',
        reason: 'Destructive database operations are blocked',
      });
    }
  }

  // Default: allow
  return res.json({
    decision: 'allow',
    reason: 'Passed custom policy checks',
  });
});

// =============================================================================
// Notification Webhook - Receive event notifications
// =============================================================================

app.post('/nche/events', verifySignature(WEBHOOK_SECRET), (req, res) => {
  const { event_type, data } = req.body;
  console.log(`Received notification: ${event_type}`);

  switch (event_type) {
    case 'approval_required':
      console.log(
        `Action ${data.action_id} (${data.tool}) needs approval: ${data.policy_reason}`
      );
      // TODO: Send Slack notification, update UI, etc.
      break;

    case 'action_approved':
      console.log(`Action ${data.action_id} approved by ${data.approver_id}`);
      break;

    case 'action_denied':
      console.log(`Action ${data.action_id} denied: ${data.note}`);
      break;

    case 'action_executed':
      console.log(`Action ${data.action_id} executed successfully`);
      break;

    case 'action_failed':
      console.warn(`Action ${data.action_id} failed: ${data.error}`);
      break;

    default:
      console.log(`Unknown event type: ${event_type}`);
  }

  return res.json({ received: true });
});

// =============================================================================
// Async Execution Helper - For long-running operations
// =============================================================================

async function reportResultAsync(actionId, success, result = null, error = null) {
  const payload = { success };
  if (success) {
    payload.result = result || {};
  } else {
    payload.error = error || 'Unknown error';
  }

  const response = await fetch(`${NCHE_API_URL}/v1/actions/${actionId}/result`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${NCHE_API_KEY}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(payload),
  });

  if (!response.ok) {
    throw new Error(`Failed to report result: ${response.status}`);
  }

  return response.json();
}

// =============================================================================
// Health Check
// =============================================================================

app.get('/health', (req, res) => {
  res.json({ status: 'ok' });
});

// =============================================================================
// Start Server
// =============================================================================

const PORT = process.env.PORT || 5001;
app.listen(PORT, () => {
  console.log(`Nche tenant webhook server listening on port ${PORT}`);
  console.log(`Execution webhook: http://localhost:${PORT}/nche/execute`);
  console.log(`Policy webhook:    http://localhost:${PORT}/nche/policy`);
  console.log(`Events webhook:    http://localhost:${PORT}/nche/events`);
});
