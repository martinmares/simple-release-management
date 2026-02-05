CREATE TABLE IF NOT EXISTS deploy_target_env_vars (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deploy_target_id UUID NOT NULL REFERENCES deploy_targets(id) ON DELETE CASCADE,
    source_key VARCHAR(128) NOT NULL,
    target_key VARCHAR(128) NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_deploy_target_env_vars_target ON deploy_target_env_vars(deploy_target_id);

INSERT INTO deploy_target_env_vars (deploy_target_id, source_key, target_key)
SELECT id, 'SIMPLE_RELEASE_ID', 'TSM_RELEASE_ID'
FROM deploy_targets
WHERE NOT EXISTS (
    SELECT 1
    FROM deploy_target_env_vars v
    WHERE v.deploy_target_id = deploy_targets.id
      AND v.target_key = 'TSM_RELEASE_ID'
);
