-- Bundle should only be tied to source registry. Target is selected per environment/copy job.

-- Drop tenant consistency trigger that checks target registry as well
DROP TRIGGER IF EXISTS bundle_tenant_consistency_trigger ON bundles;

CREATE OR REPLACE FUNCTION check_bundle_tenant_consistency()
RETURNS TRIGGER AS $$
DECLARE
    source_tenant_id UUID;
BEGIN
    SELECT tenant_id INTO source_tenant_id
    FROM registries
    WHERE id = NEW.source_registry_id;

    IF source_tenant_id != NEW.tenant_id THEN
        RAISE EXCEPTION 'Source registry does not belong to bundle tenant (registry tenant: %, bundle tenant: %)',
            source_tenant_id, NEW.tenant_id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER bundle_tenant_consistency_trigger
    BEFORE INSERT OR UPDATE ON bundles
    FOR EACH ROW
    EXECUTE FUNCTION check_bundle_tenant_consistency();

-- Migrate bundle tag counters to environment_id
ALTER TABLE bundle_tag_counters
    ADD COLUMN IF NOT EXISTS environment_id UUID;

WITH latest_env AS (
    SELECT DISTINCT ON (bv.bundle_id, cj.target_registry_id)
        bv.bundle_id,
        cj.target_registry_id,
        cj.environment_id
    FROM copy_jobs cj
    JOIN bundle_versions bv ON bv.id = cj.bundle_version_id
    WHERE cj.environment_id IS NOT NULL
    ORDER BY bv.bundle_id, cj.target_registry_id, cj.started_at DESC NULLS LAST, cj.created_at DESC
)
UPDATE bundle_tag_counters btc
SET environment_id = latest_env.environment_id
FROM latest_env
WHERE btc.bundle_id = latest_env.bundle_id
  AND btc.target_registry_id = latest_env.target_registry_id;

WITH env_pick AS (
    SELECT DISTINCT ON (target_registry_id)
        id,
        target_registry_id
    FROM environments
    WHERE target_registry_id IS NOT NULL
    ORDER BY target_registry_id, created_at ASC
)
UPDATE bundle_tag_counters btc
SET environment_id = env_pick.id
FROM env_pick
WHERE btc.environment_id IS NULL
  AND btc.target_registry_id = env_pick.target_registry_id;

DELETE FROM bundle_tag_counters
WHERE environment_id IS NULL;

ALTER TABLE bundle_tag_counters
    ALTER COLUMN environment_id SET NOT NULL;

ALTER TABLE bundle_tag_counters
    DROP CONSTRAINT IF EXISTS bundle_tag_counters_pkey;

ALTER TABLE bundle_tag_counters
    ADD PRIMARY KEY (bundle_id, environment_id, date);

ALTER TABLE bundle_tag_counters
    DROP COLUMN IF EXISTS target_registry_id;

-- Drop target registry reference from bundles
ALTER TABLE bundles
    DROP COLUMN IF EXISTS target_registry_id;

DROP INDEX IF EXISTS idx_bundles_target_registry;
