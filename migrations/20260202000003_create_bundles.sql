-- Create bundles table
CREATE TABLE bundles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    source_registry_id UUID NOT NULL REFERENCES registries(id),
    target_registry_id UUID NOT NULL REFERENCES registries(id),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    current_version INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(tenant_id, name)
);

-- Create indexes for faster lookups
CREATE INDEX idx_bundles_tenant ON bundles(tenant_id);
CREATE INDEX idx_bundles_source_registry ON bundles(source_registry_id);
CREATE INDEX idx_bundles_target_registry ON bundles(target_registry_id);
CREATE INDEX idx_bundles_name ON bundles(name);
