-- Add app/container metadata to image_mappings
ALTER TABLE image_mappings
    ADD COLUMN IF NOT EXISTS app_name VARCHAR(255),
    ADD COLUMN IF NOT EXISTS container_name VARCHAR(255);

-- Backfill app_name from target_image last path segment
UPDATE image_mappings
SET app_name = split_part(target_image, '/', array_length(string_to_array(target_image, '/'), 1))
WHERE app_name IS NULL OR app_name = '';

ALTER TABLE image_mappings
    ALTER COLUMN app_name SET NOT NULL;

-- Deploy targets (per-tenant deployment configuration)
CREATE TABLE IF NOT EXISTS deploy_targets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    env_name VARCHAR(50) NOT NULL,
    environments_repo_url VARCHAR(500) NOT NULL,
    environments_branch VARCHAR(255) NOT NULL DEFAULT 'main',
    deploy_repo_url VARCHAR(500) NOT NULL,
    deploy_branch VARCHAR(255) NOT NULL DEFAULT 'main',
    deploy_path VARCHAR(255) NOT NULL,
    git_auth_type VARCHAR(20) NOT NULL DEFAULT 'none'
        CHECK (git_auth_type IN ('none', 'ssh', 'token')),
    git_username VARCHAR(255),
    git_token_encrypted TEXT,
    git_ssh_key_encrypted TEXT,
    encjson_key_dir VARCHAR(500),
    encjson_private_key_encrypted TEXT,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_deploy_targets_tenant ON deploy_targets(tenant_id);
CREATE INDEX IF NOT EXISTS idx_deploy_targets_env ON deploy_targets(env_name);

-- Deploy jobs (release -> deployment pipeline)
CREATE TABLE IF NOT EXISTS deploy_jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    release_id UUID NOT NULL REFERENCES releases(id) ON DELETE CASCADE,
    deploy_target_id UUID NOT NULL REFERENCES deploy_targets(id) ON DELETE CASCADE,
    status VARCHAR(20) NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'in_progress', 'success', 'failed')),
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    error_message TEXT,
    commit_sha VARCHAR(64),
    tag_name VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_deploy_jobs_release ON deploy_jobs(release_id);
CREATE INDEX IF NOT EXISTS idx_deploy_jobs_target ON deploy_jobs(deploy_target_id);
CREATE INDEX IF NOT EXISTS idx_deploy_jobs_status ON deploy_jobs(status);
