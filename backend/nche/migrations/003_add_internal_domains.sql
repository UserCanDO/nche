-- Add internal_domains column to tenants table
-- This stores a JSON array of domain strings that are considered "internal" for this tenant
-- Emails to internal domains may be auto-approved in supervised mode

ALTER TABLE tenants ADD COLUMN internal_domains JSONB;

-- Add a comment for documentation
COMMENT ON COLUMN tenants.internal_domains IS 'JSON array of internal email domains. Emails to these domains may be auto-approved in supervised mode. Supports wildcards like *.example.com';
