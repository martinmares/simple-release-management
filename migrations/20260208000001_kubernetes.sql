CREATE TABLE IF NOT EXISTS kubernetes_instances (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    base_url TEXT NOT NULL,
    auth_type TEXT NOT NULL,
    username TEXT,
    password_encrypted TEXT,
    token_encrypted TEXT,
    insecure BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS kubernetes_instances_tenant_id_idx ON kubernetes_instances(tenant_id);

CREATE TABLE IF NOT EXISTS environment_kubernetes_namespaces (
    id UUID PRIMARY KEY,
    environment_id UUID NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
    kubernetes_instance_id UUID NOT NULL REFERENCES kubernetes_instances(id) ON DELETE CASCADE,
    namespace TEXT NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE INDEX IF NOT EXISTS env_kubernetes_env_idx ON environment_kubernetes_namespaces(environment_id);
CREATE INDEX IF NOT EXISTS env_kubernetes_instance_idx ON environment_kubernetes_namespaces(kubernetes_instance_id);
