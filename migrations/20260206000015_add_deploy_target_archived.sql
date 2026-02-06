ALTER TABLE deploy_targets
    ADD COLUMN IF NOT EXISTS is_archived BOOLEAN NOT NULL DEFAULT FALSE;

CREATE INDEX IF NOT EXISTS idx_deploy_targets_archived
    ON deploy_targets(is_archived);
