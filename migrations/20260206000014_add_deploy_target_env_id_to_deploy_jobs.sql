ALTER TABLE deploy_jobs
    ADD COLUMN IF NOT EXISTS deploy_target_env_id UUID;

UPDATE deploy_jobs dj
SET deploy_target_env_id = dte.id
FROM deploy_targets dt
JOIN environments e
  ON e.tenant_id = dt.tenant_id
 AND e.slug = lower(dt.env_name)
JOIN deploy_target_envs dte
  ON dte.deploy_target_id = dt.id
 AND dte.environment_id = e.id
WHERE dj.deploy_target_id = dt.id
  AND dj.deploy_target_env_id IS NULL;

ALTER TABLE deploy_jobs
    ALTER COLUMN deploy_target_env_id SET NOT NULL;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'deploy_jobs_deploy_target_env_fk'
    ) THEN
        ALTER TABLE deploy_jobs
            ADD CONSTRAINT deploy_jobs_deploy_target_env_fk
                FOREIGN KEY (deploy_target_env_id)
                REFERENCES deploy_target_envs(id)
                ON DELETE CASCADE;
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_deploy_jobs_target_env
    ON deploy_jobs(deploy_target_env_id);
