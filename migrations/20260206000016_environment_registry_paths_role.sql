ALTER TABLE environment_registry_paths
    ADD COLUMN IF NOT EXISTS role VARCHAR(10) NOT NULL DEFAULT 'target'
        CHECK (role IN ('source', 'target'));

UPDATE environment_registry_paths
SET role = 'target'
WHERE role IS NULL;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_indexes WHERE indexname = 'idx_environment_registry_paths_unique'
    ) THEN
        DROP INDEX idx_environment_registry_paths_unique;
    END IF;
END $$;

CREATE UNIQUE INDEX IF NOT EXISTS idx_environment_registry_paths_unique
    ON environment_registry_paths(environment_id, registry_id, role);
