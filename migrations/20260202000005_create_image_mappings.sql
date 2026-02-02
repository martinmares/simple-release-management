-- Create image_mappings table
CREATE TABLE image_mappings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    bundle_version_id UUID NOT NULL REFERENCES bundle_versions(id) ON DELETE CASCADE,

    -- Source image info
    source_image VARCHAR(500) NOT NULL,
    source_tag VARCHAR(255) NOT NULL,
    source_sha256 VARCHAR(71) NOT NULL,

    -- Target image info
    target_image VARCHAR(500) NOT NULL,
    target_tag_template VARCHAR(255) NOT NULL,
    target_sha256 VARCHAR(71),

    -- Copy status tracking
    copy_status VARCHAR(20) NOT NULL DEFAULT 'pending'
        CHECK (copy_status IN ('pending', 'in_progress', 'success', 'failed')),
    copied_at TIMESTAMPTZ,
    error_message TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for faster lookups
CREATE INDEX idx_image_mappings_version ON image_mappings(bundle_version_id);
CREATE INDEX idx_image_mappings_status ON image_mappings(copy_status);
CREATE INDEX idx_image_mappings_source ON image_mappings(source_image, source_tag);
CREATE INDEX idx_image_mappings_source_sha ON image_mappings(source_sha256);
