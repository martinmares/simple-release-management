-- Fix foreign key constraint on releases to cascade delete
ALTER TABLE releases
    DROP CONSTRAINT releases_bundle_version_id_fkey,
    ADD CONSTRAINT releases_bundle_version_id_fkey
        FOREIGN KEY (bundle_version_id)
        REFERENCES bundle_versions(id)
        ON DELETE CASCADE;
