-- migrations/001_initial.sql

-- Tenants are the top-level isolation boundary
CREATE TABLE tenants (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    webhook_url TEXT,
    webhook_secret TEXT,
    webhook_events TEXT,  -- JSON array of event types
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Agents belong to a tenant
CREATE TABLE agents (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    name TEXT NOT NULL,
    api_key_hash TEXT NOT NULL,
    api_key_prefix TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(tenant_id, name)
);

-- Sessions scope agent runs
CREATE TABLE sessions (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    agent_id TEXT NOT NULL REFERENCES agents(id),
    actor_id TEXT NOT NULL,
    actor_type TEXT NOT NULL CHECK (actor_type IN ('user', 'org', 'system')),
    autonomy_level TEXT NOT NULL CHECK (autonomy_level IN ('full', 'supervised', 'restricted')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ended_at TIMESTAMPTZ
);

-- Actions are the core unit
CREATE TABLE actions (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    session_id TEXT NOT NULL REFERENCES sessions(id),
    tool TEXT NOT NULL,
    params JSONB NOT NULL,
    state TEXT NOT NULL CHECK (state IN (
        'proposed', 'paused_for_approval', 'ready_to_execute',
        'executing', 'executed', 'denied', 'failed'
    )),
    policy_result TEXT CHECK (policy_result IN ('allow', 'deny', 'require_approval')),
    policy_reason TEXT,
    result JSONB,
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Approvals link to actions (one per action in v0.1)
CREATE TABLE approvals (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    action_id TEXT NOT NULL REFERENCES actions(id),
    status TEXT NOT NULL CHECK (status IN ('pending', 'approved', 'denied')),
    approver_id TEXT,
    approver_note TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    decided_at TIMESTAMPTZ,
    UNIQUE(action_id)  -- One approval per action in v0.1
);

-- Immutable audit log
CREATE TABLE events (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    session_id TEXT REFERENCES sessions(id),  -- NULL for session-level events
    action_id TEXT REFERENCES actions(id),    -- NULL for session-level events
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Webhook delivery log (for retries and debugging)
CREATE TABLE webhook_deliveries (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'delivered', 'failed')),
    attempts INTEGER NOT NULL DEFAULT 0,
    last_attempt_at TIMESTAMPTZ,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_error TEXT,
    attempt_metadata JSONB NOT NULL DEFAULT '[]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- NCHE-native records: Tasks
CREATE TABLE tasks (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    session_id TEXT REFERENCES sessions(id),
    title TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'in_progress', 'completed')),
    notes JSONB DEFAULT '[]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- NCHE-native records: Cases
CREATE TABLE cases (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    session_id TEXT REFERENCES sessions(id),
    title TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'escalated', 'resolved')),
    severity TEXT NOT NULL DEFAULT 'medium' CHECK (severity IN ('low', 'medium', 'high', 'critical')),
    evidence JSONB DEFAULT '[]',
    external_ref TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- NCHE-native records: Documents (metadata only)
CREATE TABLE documents (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    session_id TEXT REFERENCES sessions(id),
    doc_type TEXT NOT NULL,
    filename TEXT,
    checksum TEXT,
    storage_uri TEXT,
    tags JSONB DEFAULT '[]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- NCHE-native records: Links
CREATE TABLE links (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    source_type TEXT NOT NULL CHECK (source_type IN ('action', 'task', 'case', 'document')),
    source_id TEXT NOT NULL,
    target_type TEXT NOT NULL CHECK (target_type IN ('action', 'task', 'case', 'document', 'approval')),
    target_id TEXT NOT NULL,
    relation TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Dashboard users (for web UI authentication)
CREATE TABLE dashboard_users (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    email TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    name TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(tenant_id, email)
);

-- Dashboard sessions
CREATE TABLE dashboard_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES dashboard_users(id),
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Performance indexes
CREATE INDEX idx_agents_tenant ON agents(tenant_id);
CREATE INDEX idx_sessions_tenant ON sessions(tenant_id);
CREATE INDEX idx_sessions_agent ON sessions(agent_id);
CREATE INDEX idx_actions_tenant ON actions(tenant_id);
CREATE INDEX idx_actions_session ON actions(session_id);
CREATE INDEX idx_actions_state_created ON actions(state, created_at);
CREATE INDEX idx_approvals_tenant ON approvals(tenant_id);
CREATE INDEX idx_approvals_action ON approvals(action_id);
CREATE INDEX idx_webhook_deliveries_tenant ON webhook_deliveries(tenant_id);
CREATE INDEX idx_webhook_deliveries_next_attempt ON webhook_deliveries(next_attempt_at) WHERE status = 'pending';
CREATE INDEX idx_events_tenant_created ON events(tenant_id, created_at DESC);
CREATE INDEX idx_events_action_timeline ON events(action_id, created_at ASC);
CREATE INDEX idx_tasks_tenant ON tasks(tenant_id);
CREATE INDEX idx_cases_tenant ON cases(tenant_id);
CREATE INDEX idx_documents_tenant ON documents(tenant_id);
CREATE INDEX idx_links_tenant ON links(tenant_id);
CREATE INDEX idx_dashboard_users_tenant ON dashboard_users(tenant_id);

-- Updated_at triggers
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_tenants_updated_at BEFORE UPDATE ON tenants
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_actions_updated_at BEFORE UPDATE ON actions
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_tasks_updated_at BEFORE UPDATE ON tasks
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_cases_updated_at BEFORE UPDATE ON cases
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_dashboard_sessions_updated_at BEFORE UPDATE ON dashboard_sessions
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
