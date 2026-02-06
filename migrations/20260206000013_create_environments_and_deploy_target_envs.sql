CREATE TABLE IF NOT EXISTS environments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(255) NOT NULL,
    color VARCHAR(32),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_environments_tenant_slug
    ON environments(tenant_id, slug);

CREATE TABLE IF NOT EXISTS deploy_target_envs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deploy_target_id UUID NOT NULL REFERENCES deploy_targets(id) ON DELETE CASCADE,
    environment_id UUID NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
    env_repo_id UUID REFERENCES git_repositories(id),
    env_repo_path VARCHAR(255),
    env_repo_branch VARCHAR(255),
    deploy_repo_id UUID REFERENCES git_repositories(id),
    deploy_repo_path VARCHAR(255),
    deploy_repo_branch VARCHAR(255),
    allow_auto_release BOOLEAN NOT NULL DEFAULT false,
    append_env_suffix BOOLEAN NOT NULL DEFAULT false,
    is_active BOOLEAN NOT NULL DEFAULT true,
    release_manifest_mode VARCHAR(32) NOT NULL DEFAULT 'match_digest',
    encjson_key_dir VARCHAR(500),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT deploy_target_envs_env_path_branch_check
        CHECK (
            ((env_repo_path IS NOT NULL AND env_repo_path <> '')::int +
             (env_repo_branch IS NOT NULL AND env_repo_branch <> '')::int) = 1
        ),
    CONSTRAINT deploy_target_envs_deploy_path_branch_check
        CHECK (
            ((deploy_repo_path IS NOT NULL AND deploy_repo_path <> '')::int +
             (deploy_repo_branch IS NOT NULL AND deploy_repo_branch <> '')::int) = 1
        )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_deploy_target_envs_unique
    ON deploy_target_envs(deploy_target_id, environment_id);

CREATE TABLE IF NOT EXISTS environment_registry_paths (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    environment_id UUID NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
    registry_id UUID NOT NULL REFERENCES registries(id) ON DELETE CASCADE,
    project_path_override VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_environment_registry_paths_unique
    ON environment_registry_paths(environment_id, registry_id);

-- Backfill environments from deploy_targets env_name
WITH envs AS (
    SELECT DISTINCT tenant_id, env_name
    FROM deploy_targets
    WHERE env_name IS NOT NULL AND env_name <> ''
)
INSERT INTO environments (tenant_id, name, slug)
SELECT tenant_id, env_name, lower(env_name)
FROM envs
ON CONFLICT (tenant_id, slug) DO NOTHING;

-- Backfill deploy_target_envs from deploy_targets
WITH env_map AS (
    SELECT dt.id AS deploy_target_id, e.id AS environment_id
    FROM deploy_targets dt
    JOIN environments e
      ON e.tenant_id = dt.tenant_id
     AND e.slug = lower(dt.env_name)
)
INSERT INTO deploy_target_envs (
    deploy_target_id,
    environment_id,
    env_repo_id,
    env_repo_path,
    deploy_repo_id,
    deploy_repo_path,
    allow_auto_release,
    append_env_suffix,
    is_active,
    release_manifest_mode,
    encjson_key_dir
)
SELECT
    dt.id,
    env_map.environment_id,
    dt.env_repo_id,
    dt.env_repo_path,
    dt.deploy_repo_id,
    dt.deploy_repo_path,
    dt.allow_auto_release,
    dt.append_env_suffix,
    dt.is_active,
    dt.release_manifest_mode,
    dt.encjson_key_dir
FROM deploy_targets dt
JOIN env_map ON env_map.deploy_target_id = dt.id
ON CONFLICT (deploy_target_id, environment_id) DO NOTHING;
