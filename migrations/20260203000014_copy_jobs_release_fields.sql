-- Extend copy_jobs with registry context and release metadata
ALTER TABLE copy_jobs
    ADD COLUMN IF NOT EXISTS source_registry_id UUID,
    ADD COLUMN IF NOT EXISTS target_registry_id UUID,
    ADD COLUMN IF NOT EXISTS is_release_job BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS release_id VARCHAR(255),
    ADD COLUMN IF NOT EXISTS release_notes TEXT;

-- Backfill registry ids for existing jobs from bundle
UPDATE copy_jobs cj
SET source_registry_id = b.source_registry_id,
    target_registry_id = b.target_registry_id
FROM bundle_versions bv
JOIN bundles b ON b.id = bv.bundle_id
WHERE cj.bundle_version_id = bv.id
  AND (cj.source_registry_id IS NULL OR cj.target_registry_id IS NULL);

ALTER TABLE copy_jobs
    ADD CONSTRAINT copy_jobs_source_registry_fkey
        FOREIGN KEY (source_registry_id) REFERENCES registries(id),
    ADD CONSTRAINT copy_jobs_target_registry_fkey
        FOREIGN KEY (target_registry_id) REFERENCES registries(id);

CREATE INDEX IF NOT EXISTS idx_copy_jobs_source_registry ON copy_jobs(source_registry_id);
CREATE INDEX IF NOT EXISTS idx_copy_jobs_target_registry ON copy_jobs(target_registry_id);
