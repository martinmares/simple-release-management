ALTER TABLE deploy_jobs
    ADD COLUMN IF NOT EXISTS kube_build_inventory JSONB,
    ADD COLUMN IF NOT EXISTS generated_profiles JSONB;

