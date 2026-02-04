-- Add archive flag to bundle_versions
ALTER TABLE bundle_versions
    ADD COLUMN IF NOT EXISTS is_archived BOOLEAN NOT NULL DEFAULT FALSE;

CREATE INDEX IF NOT EXISTS idx_bundle_versions_archived ON bundle_versions(is_archived);
