-- Add CHECK constraint to ensure bundle registries belong to the same tenant
-- This prevents mixing registries from different tenants in a single bundle

-- First, verify existing data doesn't violate this constraint
DO $$
DECLARE
    violation_count INTEGER;
BEGIN
    SELECT COUNT(*) INTO violation_count
    FROM bundles b
    JOIN registries sr ON b.source_registry_id = sr.id
    JOIN registries tr ON b.target_registry_id = tr.id
    WHERE sr.tenant_id != b.tenant_id
       OR tr.tenant_id != b.tenant_id;

    IF violation_count > 0 THEN
        RAISE EXCEPTION 'Cannot add constraint: % existing bundles have registries from different tenants', violation_count;
    END IF;
END $$;

-- Add CHECK constraint
-- Note: PostgreSQL doesn't support subqueries in CHECK constraints directly,
-- so we'll use a trigger instead for runtime validation

-- Create function to validate bundle tenant consistency
CREATE OR REPLACE FUNCTION check_bundle_tenant_consistency()
RETURNS TRIGGER AS $$
DECLARE
    source_tenant_id UUID;
    target_tenant_id UUID;
BEGIN
    -- Get tenant_id of source registry
    SELECT tenant_id INTO source_tenant_id
    FROM registries
    WHERE id = NEW.source_registry_id;

    -- Get tenant_id of target registry
    SELECT tenant_id INTO target_tenant_id
    FROM registries
    WHERE id = NEW.target_registry_id;

    -- Validate that both registries belong to the same tenant as the bundle
    IF source_tenant_id != NEW.tenant_id THEN
        RAISE EXCEPTION 'Source registry does not belong to bundle tenant (registry tenant: %, bundle tenant: %)',
            source_tenant_id, NEW.tenant_id;
    END IF;

    IF target_tenant_id != NEW.tenant_id THEN
        RAISE EXCEPTION 'Target registry does not belong to bundle tenant (registry tenant: %, bundle tenant: %)',
            target_tenant_id, NEW.tenant_id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create trigger for INSERT and UPDATE
DROP TRIGGER IF EXISTS bundle_tenant_consistency_trigger ON bundles;
CREATE TRIGGER bundle_tenant_consistency_trigger
    BEFORE INSERT OR UPDATE ON bundles
    FOR EACH ROW
    EXECUTE FUNCTION check_bundle_tenant_consistency();
