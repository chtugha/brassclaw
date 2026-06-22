-- V31: Add capability_permissions table for V2 permission storage
--
-- This table stores permission overrides for V2 capabilities, allowing
-- administrators to control which capabilities are allowed, denied, or
-- require approval on a per-tenant basis.

CREATE TABLE IF NOT EXISTS capability_permissions (
    tenant_id TEXT NOT NULL,
    capability_id TEXT NOT NULL,
    permission_mode TEXT NOT NULL CHECK (permission_mode IN ('allow', 'ask', 'deny')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (tenant_id, capability_id)
);

-- Index for efficient tenant-based queries
CREATE INDEX IF NOT EXISTS idx_capability_permissions_tenant 
    ON capability_permissions(tenant_id);

-- Index for efficient capability lookup
CREATE INDEX IF NOT EXISTS idx_capability_permissions_capability 
    ON capability_permissions(capability_id);

-- Made with Bob
