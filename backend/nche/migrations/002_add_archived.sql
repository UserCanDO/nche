-- Add archived_at column to NCHE-native records for soft-delete/archive functionality

ALTER TABLE tasks ADD COLUMN archived_at TIMESTAMPTZ;
ALTER TABLE cases ADD COLUMN archived_at TIMESTAMPTZ;
ALTER TABLE documents ADD COLUMN archived_at TIMESTAMPTZ;
ALTER TABLE links ADD COLUMN archived_at TIMESTAMPTZ;

-- Partial indexes for efficient queries on non-archived records
CREATE INDEX idx_tasks_not_archived ON tasks(tenant_id) WHERE archived_at IS NULL;
CREATE INDEX idx_cases_not_archived ON cases(tenant_id) WHERE archived_at IS NULL;
CREATE INDEX idx_documents_not_archived ON documents(tenant_id) WHERE archived_at IS NULL;
CREATE INDEX idx_links_not_archived ON links(tenant_id) WHERE archived_at IS NULL;

-- Indexes for archived record queries
CREATE INDEX idx_tasks_archived ON tasks(tenant_id, archived_at) WHERE archived_at IS NOT NULL;
CREATE INDEX idx_cases_archived ON cases(tenant_id, archived_at) WHERE archived_at IS NOT NULL;
CREATE INDEX idx_documents_archived ON documents(tenant_id, archived_at) WHERE archived_at IS NOT NULL;
CREATE INDEX idx_links_archived ON links(tenant_id, archived_at) WHERE archived_at IS NOT NULL;
