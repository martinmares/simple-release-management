-- Create releases table
CREATE TABLE releases (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    bundle_version_id UUID NOT NULL REFERENCES bundle_versions(id),
    release_id VARCHAR(255) NOT NULL UNIQUE,
    status VARCHAR(20) NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'released', 'deployed')),
    notes TEXT,
    created_by VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for faster lookups
CREATE INDEX idx_releases_bundle_version ON releases(bundle_version_id);
CREATE INDEX idx_releases_release_id ON releases(release_id);
CREATE INDEX idx_releases_status ON releases(status);
CREATE INDEX idx_releases_created_at ON releases(created_at DESC);
