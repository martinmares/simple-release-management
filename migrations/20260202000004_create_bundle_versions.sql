-- Create bundle_versions table
CREATE TABLE bundle_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    bundle_id UUID NOT NULL REFERENCES bundles(id) ON DELETE CASCADE,
    version INTEGER NOT NULL,
    change_note TEXT,
    created_by VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(bundle_id, version)
);

-- Create indexes for faster lookups
CREATE INDEX idx_bundle_versions_bundle ON bundle_versions(bundle_id);
CREATE INDEX idx_bundle_versions_bundle_version ON bundle_versions(bundle_id, version);
