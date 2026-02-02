-- Create registries table
CREATE TABLE registries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    registry_type VARCHAR(50) NOT NULL CHECK (
        registry_type IN ('harbor', 'docker', 'quay', 'gcr', 'ecr', 'acr', 'generic')
    ),
    base_url VARCHAR(500) NOT NULL,
    credentials_path VARCHAR(500) NOT NULL,
    role VARCHAR(20) NOT NULL CHECK (role IN ('source', 'target', 'both')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(tenant_id, name)
);

-- Create indexes for faster lookups
CREATE INDEX idx_registries_tenant ON registries(tenant_id);
CREATE INDEX idx_registries_type ON registries(registry_type);
CREATE INDEX idx_registries_role ON registries(role);
