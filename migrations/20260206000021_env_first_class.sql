-- Promote Environment to hold registry + deploy config
ALTER TABLE environments
    ADD COLUMN IF NOT EXISTS source_registry_id UUID REFERENCES registries(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS target_registry_id UUID REFERENCES registries(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS source_project_path TEXT,
    ADD COLUMN IF NOT EXISTS target_project_path TEXT,
    ADD COLUMN IF NOT EXISTS source_auth_type TEXT,
    ADD COLUMN IF NOT EXISTS source_username TEXT,
    ADD COLUMN IF NOT EXISTS source_password_encrypted TEXT,
    ADD COLUMN IF NOT EXISTS source_token_encrypted TEXT,
    ADD COLUMN IF NOT EXISTS target_auth_type TEXT,
    ADD COLUMN IF NOT EXISTS target_username TEXT,
    ADD COLUMN IF NOT EXISTS target_password_encrypted TEXT,
    ADD COLUMN IF NOT EXISTS target_token_encrypted TEXT,
    ADD COLUMN IF NOT EXISTS env_repo_id UUID REFERENCES git_repositories(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS env_repo_path TEXT,
    ADD COLUMN IF NOT EXISTS env_repo_branch TEXT,
    ADD COLUMN IF NOT EXISTS deploy_repo_id UUID REFERENCES git_repositories(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS deploy_repo_path TEXT,
    ADD COLUMN IF NOT EXISTS deploy_repo_branch TEXT,
    ADD COLUMN IF NOT EXISTS allow_auto_release BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS append_env_suffix BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS release_manifest_mode TEXT,
    ADD COLUMN IF NOT EXISTS encjson_key_dir TEXT,
    ADD COLUMN IF NOT EXISTS release_env_var_mappings JSONB NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS extra_env_vars JSONB NOT NULL DEFAULT '{}'::jsonb;

-- Backfill deploy repo config from deploy_target_envs (latest per environment)
WITH ranked AS (
    SELECT dte.*, dt.is_archived,
           ROW_NUMBER() OVER (PARTITION BY dte.environment_id ORDER BY dte.created_at DESC) AS rn
    FROM deploy_target_envs dte
    JOIN deploy_targets dt ON dt.id = dte.deploy_target_id
    WHERE COALESCE(dt.is_archived, FALSE) = FALSE
), selected AS (
    SELECT * FROM ranked WHERE rn = 1
)
UPDATE environments e
SET env_repo_id = s.env_repo_id,
    env_repo_path = s.env_repo_path,
    env_repo_branch = s.env_repo_branch,
    deploy_repo_id = s.deploy_repo_id,
    deploy_repo_path = s.deploy_repo_path,
    deploy_repo_branch = s.deploy_repo_branch,
    allow_auto_release = s.allow_auto_release,
    append_env_suffix = s.append_env_suffix,
    release_manifest_mode = s.release_manifest_mode,
    encjson_key_dir = s.encjson_key_dir
FROM selected s
WHERE e.id = s.environment_id;

-- Backfill env var mappings + extra env vars from selected deploy target
WITH ranked AS (
    SELECT dte.*, dt.is_archived,
           ROW_NUMBER() OVER (PARTITION BY dte.environment_id ORDER BY dte.created_at DESC) AS rn
    FROM deploy_target_envs dte
    JOIN deploy_targets dt ON dt.id = dte.deploy_target_id
    WHERE COALESCE(dt.is_archived, FALSE) = FALSE
), selected AS (
    SELECT * FROM ranked WHERE rn = 1
)
UPDATE environments e
SET release_env_var_mappings = COALESCE((
        SELECT jsonb_object_agg(v.source_key, v.target_key)
        FROM deploy_target_env_vars v
        WHERE v.deploy_target_id = s.deploy_target_id
    ), '{}'::jsonb),
    extra_env_vars = COALESCE((
        SELECT jsonb_object_agg(ev.key, ev.value)
        FROM deploy_target_extra_env_vars ev
        WHERE ev.deploy_target_id = s.deploy_target_id
    ), '{}'::jsonb)
FROM selected s
WHERE e.id = s.environment_id;

-- Backfill source/target registry ids using enabled registry access + role
WITH env_regs AS (
    SELECT e.id AS environment_id, r.id AS registry_id, r.role, r.created_at,
           COALESCE(era.is_enabled, TRUE) AS is_enabled
    FROM environments e
    JOIN registries r ON r.tenant_id = e.tenant_id
    LEFT JOIN environment_registry_access era
        ON era.environment_id = e.id AND era.registry_id = r.id
), source_pick AS (
    SELECT environment_id, registry_id, created_at
    FROM env_regs
    WHERE is_enabled AND role IN ('source','both')
), target_pick AS (
    SELECT environment_id, registry_id, created_at
    FROM env_regs
    WHERE is_enabled AND role IN ('target','both')
), source_pick_distinct AS (
    SELECT DISTINCT ON (environment_id) environment_id, registry_id
    FROM source_pick
    ORDER BY environment_id, created_at
), target_pick_distinct AS (
    SELECT DISTINCT ON (environment_id) environment_id, registry_id
    FROM target_pick
    ORDER BY environment_id, created_at
)
UPDATE environments e
SET source_registry_id = COALESCE(e.source_registry_id, sp.registry_id),
    target_registry_id = COALESCE(e.target_registry_id, tp.registry_id)
FROM source_pick_distinct sp
FULL JOIN target_pick_distinct tp ON tp.environment_id = sp.environment_id
WHERE e.id = COALESCE(sp.environment_id, tp.environment_id);

-- Backfill project paths for selected registries
UPDATE environments e
SET source_project_path = erp.project_path_override
FROM environment_registry_paths erp
WHERE erp.environment_id = e.id
  AND erp.role = 'source'
  AND e.source_registry_id IS NOT NULL
  AND erp.registry_id = e.source_registry_id
  AND e.source_project_path IS NULL;

UPDATE environments e
SET target_project_path = erp.project_path_override
FROM environment_registry_paths erp
WHERE erp.environment_id = e.id
  AND erp.role = 'target'
  AND e.target_registry_id IS NOT NULL
  AND erp.registry_id = e.target_registry_id
  AND e.target_project_path IS NULL;

-- Backfill auth overrides for selected registries
UPDATE environments e
SET source_auth_type = erc.auth_type,
    source_username = erc.username,
    source_password_encrypted = erc.password_encrypted,
    source_token_encrypted = erc.token_encrypted
FROM environment_registry_credentials erc
WHERE erc.environment_id = e.id
  AND e.source_registry_id IS NOT NULL
  AND erc.registry_id = e.source_registry_id
  AND e.source_auth_type IS NULL;

UPDATE environments e
SET target_auth_type = erc.auth_type,
    target_username = erc.username,
    target_password_encrypted = erc.password_encrypted,
    target_token_encrypted = erc.token_encrypted
FROM environment_registry_credentials erc
WHERE erc.environment_id = e.id
  AND e.target_registry_id IS NOT NULL
  AND erc.registry_id = e.target_registry_id
  AND e.target_auth_type IS NULL;
