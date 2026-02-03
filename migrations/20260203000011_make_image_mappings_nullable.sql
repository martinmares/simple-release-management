-- Make source_sha256 and target_tag_template nullable in image_mappings
-- These fields are not known at bundle creation time and are filled later

ALTER TABLE image_mappings
    ALTER COLUMN source_sha256 DROP NOT NULL;

ALTER TABLE image_mappings
    ALTER COLUMN target_tag_template DROP NOT NULL;
