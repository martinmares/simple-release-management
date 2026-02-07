CREATE TABLE IF NOT EXISTS argocd_instances (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    base_url TEXT NOT NULL,
    auth_type TEXT NOT NULL,
    username TEXT,
    password_encrypted TEXT,
    token_encrypted TEXT,
    verify_tls BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS argocd_instances_tenant_id_idx ON argocd_instances(tenant_id);

CREATE TABLE IF NOT EXISTS environment_argocd_apps (
    id UUID PRIMARY KEY,
    environment_id UUID NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
    argocd_instance_id UUID NOT NULL REFERENCES argocd_instances(id) ON DELETE CASCADE,
    application_name TEXT NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    last_sync_status TEXT,
    last_health_status TEXT,
    last_operation_phase TEXT,
    last_operation_message TEXT,
    last_revision TEXT,
    last_checked_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS env_argocd_env_idx ON environment_argocd_apps(environment_id);
CREATE INDEX IF NOT EXISTS env_argocd_instance_idx ON environment_argocd_apps(argocd_instance_id);

ALTER TABLE environments
    ADD COLUMN IF NOT EXISTS argocd_poll_interval_seconds INT NOT NULL DEFAULT 0;
