ALTER TABLE deploy_targets
    ADD COLUMN IF NOT EXISTS release_manifest_mode VARCHAR(32) NOT NULL DEFAULT 'match_digest';
