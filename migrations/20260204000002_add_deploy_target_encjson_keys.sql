-- Deploy target encjson keys (multiple key pairs)
CREATE TABLE IF NOT EXISTS deploy_target_encjson_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deploy_target_id UUID NOT NULL REFERENCES deploy_targets(id) ON DELETE CASCADE,
    public_key VARCHAR(128) NOT NULL,
    private_key_encrypted TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_deploy_target_encjson_keys_unique
    ON deploy_target_encjson_keys(deploy_target_id, public_key);
