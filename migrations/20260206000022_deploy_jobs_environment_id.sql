ALTER TABLE deploy_jobs
    ADD COLUMN IF NOT EXISTS environment_id UUID;

UPDATE deploy_jobs dj
SET environment_id = dte.environment_id
FROM deploy_target_envs dte
WHERE dj.deploy_target_env_id = dte.id
  AND dj.environment_id IS NULL;

ALTER TABLE deploy_jobs
    ALTER COLUMN environment_id SET NOT NULL;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'deploy_jobs_deploy_target_env_fk'
    ) THEN
        ALTER TABLE deploy_jobs
            DROP CONSTRAINT deploy_jobs_deploy_target_env_fk;
    END IF;
END $$;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'deploy_jobs_deploy_target_id_fkey'
    ) THEN
        ALTER TABLE deploy_jobs
            DROP CONSTRAINT deploy_jobs_deploy_target_id_fkey;
    END IF;
END $$;

ALTER TABLE deploy_jobs
    ALTER COLUMN deploy_target_id DROP NOT NULL,
    ALTER COLUMN deploy_target_env_id DROP NOT NULL;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'deploy_jobs_environment_fk'
    ) THEN
        ALTER TABLE deploy_jobs
            ADD CONSTRAINT deploy_jobs_environment_fk
                FOREIGN KEY (environment_id)
                REFERENCES environments(id)
                ON DELETE CASCADE;
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_deploy_jobs_environment
    ON deploy_jobs(environment_id);
