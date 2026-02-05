-- Git repositories per tenant (shared by deploy targets)
CREATE TABLE IF NOT EXISTS git_repositories (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    repo_url VARCHAR(500) NOT NULL,
    default_branch VARCHAR(255) NOT NULL DEFAULT 'main',
    git_auth_type VARCHAR(20) NOT NULL DEFAULT 'none'
        CHECK (git_auth_type IN ('none', 'ssh', 'token')),
    git_username VARCHAR(255),
    git_token_encrypted TEXT,
    git_ssh_key_encrypted TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_git_repositories_tenant ON git_repositories(tenant_id);
CREATE INDEX IF NOT EXISTS idx_git_repositories_url ON git_repositories(repo_url);

ALTER TABLE deploy_targets
    ADD COLUMN IF NOT EXISTS env_repo_id UUID REFERENCES git_repositories(id),
    ADD COLUMN IF NOT EXISTS env_repo_path VARCHAR(255),
    ADD COLUMN IF NOT EXISTS deploy_repo_id UUID REFERENCES git_repositories(id),
    ADD COLUMN IF NOT EXISTS deploy_repo_path VARCHAR(255);

-- Backfill: create dedicated git repo rows per deploy target (env + deploy)
WITH env_rows AS (
    SELECT
        dt.id AS deploy_target_id,
        gen_random_uuid() AS repo_id,
        dt.tenant_id AS tenant_id,
        dt.name || ' env' AS name,
        dt.environments_repo_url AS repo_url,
        dt.environments_branch AS default_branch,
        dt.git_auth_type AS git_auth_type,
        dt.git_username AS git_username,
        dt.git_token_encrypted AS git_token_encrypted,
        dt.git_ssh_key_encrypted AS git_ssh_key_encrypted
    FROM deploy_targets dt
)
INSERT INTO git_repositories (id, tenant_id, name, repo_url, default_branch, git_auth_type, git_username, git_token_encrypted, git_ssh_key_encrypted)
SELECT repo_id, tenant_id, name, repo_url, default_branch, git_auth_type, git_username, git_token_encrypted, git_ssh_key_encrypted
FROM env_rows;

WITH deploy_rows AS (
    SELECT
        dt.id AS deploy_target_id,
        gen_random_uuid() AS repo_id,
        dt.tenant_id AS tenant_id,
        dt.name || ' deploy' AS name,
        dt.deploy_repo_url AS repo_url,
        dt.deploy_branch AS default_branch,
        dt.git_auth_type AS git_auth_type,
        dt.git_username AS git_username,
        dt.git_token_encrypted AS git_token_encrypted,
        dt.git_ssh_key_encrypted AS git_ssh_key_encrypted
    FROM deploy_targets dt
)
INSERT INTO git_repositories (id, tenant_id, name, repo_url, default_branch, git_auth_type, git_username, git_token_encrypted, git_ssh_key_encrypted)
SELECT repo_id, tenant_id, name, repo_url, default_branch, git_auth_type, git_username, git_token_encrypted, git_ssh_key_encrypted
FROM deploy_rows;

-- Wire deploy_targets to the newly created repos
WITH env_map AS (
    SELECT dt.id AS deploy_target_id, gr.id AS repo_id
    FROM deploy_targets dt
    JOIN git_repositories gr
      ON gr.tenant_id = dt.tenant_id
     AND gr.repo_url = dt.environments_repo_url
     AND gr.default_branch = dt.environments_branch
     AND gr.name = dt.name || ' env'
)
UPDATE deploy_targets dt
SET env_repo_id = env_map.repo_id,
    env_repo_path = COALESCE(dt.env_name, ''),
    deploy_repo_path = COALESCE(dt.deploy_path, '')
FROM env_map
WHERE dt.id = env_map.deploy_target_id;

WITH deploy_map AS (
    SELECT dt.id AS deploy_target_id, gr.id AS repo_id
    FROM deploy_targets dt
    JOIN git_repositories gr
      ON gr.tenant_id = dt.tenant_id
     AND gr.repo_url = dt.deploy_repo_url
     AND gr.default_branch = dt.deploy_branch
     AND gr.name = dt.name || ' deploy'
)
UPDATE deploy_targets dt
SET deploy_repo_id = deploy_map.repo_id
FROM deploy_map
WHERE dt.id = deploy_map.deploy_target_id;
