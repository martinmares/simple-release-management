CREATE TABLE IF NOT EXISTS deploy_target_extra_env_vars (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deploy_target_id UUID NOT NULL REFERENCES deploy_targets(id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_deploy_target_extra_env_vars_target_id ON deploy_target_extra_env_vars(deploy_target_id);
