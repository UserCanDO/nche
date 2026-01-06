-- Migration: Execution Webhook Model
--
-- Shifts Nche from direct execution to governance middleware.
-- Tenant executes tools via webhook, reports results back.

-- Add execution webhook configuration to tenants
ALTER TABLE tenants ADD COLUMN execution_webhook_url TEXT;
ALTER TABLE tenants ADD COLUMN execution_webhook_secret TEXT;
ALTER TABLE tenants ADD COLUMN execution_webhook_timeout_ms INTEGER DEFAULT 30000;

-- Add execution result tracking to actions
ALTER TABLE actions ADD COLUMN execution_result JSONB;
ALTER TABLE actions ADD COLUMN executed_by TEXT;

-- Note: ActionState enum changes handled in Rust code:
-- - Add 'pending_execution' state (after approval, before tenant executes)
-- - Remove 'executing' state (Nche no longer executes directly)
--
-- New flow:
--   proposed -> [policy] -> ready_to_execute | paused_for_approval | denied
--   paused_for_approval -> [human] -> ready_to_execute | denied
--   ready_to_execute -> pending_execution (webhook sent to tenant)
--   pending_execution -> executed | failed (tenant reports result)

-- Index for finding actions pending execution
CREATE INDEX idx_actions_pending_execution ON actions(tenant_id, state)
WHERE state = 'pending_execution';
