-- Migration: Add policy webhook mode
-- Allows tenants to delegate policy evaluation to their own webhook

-- Policy mode: 'builtin' (default) uses Nche's built-in policies
--              'webhook' delegates to tenant's policy webhook
ALTER TABLE tenants ADD COLUMN policy_mode TEXT DEFAULT 'builtin';

-- Webhook URL for policy evaluation (when policy_mode = 'webhook')
ALTER TABLE tenants ADD COLUMN policy_webhook_url TEXT;

-- Secret for signing policy webhook requests (HMAC-SHA256)
ALTER TABLE tenants ADD COLUMN policy_webhook_secret TEXT;

-- Timeout for policy webhook (default 500ms - must be fast)
ALTER TABLE tenants ADD COLUMN policy_webhook_timeout_ms INTEGER DEFAULT 500;

-- Add constraint to validate policy_mode values
ALTER TABLE tenants ADD CONSTRAINT tenants_policy_mode_check
    CHECK (policy_mode IN ('builtin', 'webhook'));
