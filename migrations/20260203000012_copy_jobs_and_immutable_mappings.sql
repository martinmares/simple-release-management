-- Introduce copy_jobs + copy_job_images and make image_mappings immutable

-- Copy jobs (runtime executions)
CREATE TABLE copy_jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    bundle_version_id UUID NOT NULL REFERENCES bundle_versions(id) ON DELETE CASCADE,
    target_tag VARCHAR(255) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'in_progress', 'success', 'failed')),
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    created_by VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_copy_jobs_bundle_version ON copy_jobs(bundle_version_id);
CREATE INDEX idx_copy_jobs_status ON copy_jobs(status);
CREATE INDEX idx_copy_jobs_created_at ON copy_jobs(created_at DESC);

-- Copy job images (snapshot + runtime results)
CREATE TABLE copy_job_images (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    copy_job_id UUID NOT NULL REFERENCES copy_jobs(id) ON DELETE CASCADE,
    image_mapping_id UUID NOT NULL REFERENCES image_mappings(id),

    -- Snapshot from template
    source_image VARCHAR(500) NOT NULL,
    source_tag VARCHAR(255) NOT NULL,
    target_image VARCHAR(500) NOT NULL,
    target_tag VARCHAR(255) NOT NULL,

    -- Runtime data
    source_sha256 VARCHAR(71),
    target_sha256 VARCHAR(71),
    copy_status VARCHAR(20) NOT NULL DEFAULT 'pending'
        CHECK (copy_status IN ('pending', 'in_progress', 'success', 'failed')),
    error_message TEXT,
    copied_at TIMESTAMPTZ,
    bytes_copied BIGINT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_copy_job_images_job ON copy_job_images(copy_job_id);
CREATE INDEX idx_copy_job_images_mapping ON copy_job_images(image_mapping_id);
CREATE INDEX idx_copy_job_images_status ON copy_job_images(copy_status);

-- Make image_mappings immutable (drop runtime fields)
ALTER TABLE image_mappings DROP COLUMN IF EXISTS source_sha256;
ALTER TABLE image_mappings DROP COLUMN IF EXISTS target_tag_template;
ALTER TABLE image_mappings DROP COLUMN IF EXISTS target_sha256;
ALTER TABLE image_mappings DROP COLUMN IF EXISTS copy_status;
ALTER TABLE image_mappings DROP COLUMN IF EXISTS copied_at;
ALTER TABLE image_mappings DROP COLUMN IF EXISTS error_message;

DROP INDEX IF EXISTS idx_image_mappings_status;
DROP INDEX IF EXISTS idx_image_mappings_source_sha;

-- Releases should reference a concrete copy job
ALTER TABLE releases DROP CONSTRAINT IF EXISTS releases_bundle_version_id_fkey;
ALTER TABLE releases DROP COLUMN IF EXISTS bundle_version_id;
ALTER TABLE releases ADD COLUMN copy_job_id UUID NOT NULL REFERENCES copy_jobs(id) ON DELETE CASCADE;

DROP INDEX IF EXISTS idx_releases_bundle_version;
CREATE INDEX idx_releases_copy_job ON releases(copy_job_id);
