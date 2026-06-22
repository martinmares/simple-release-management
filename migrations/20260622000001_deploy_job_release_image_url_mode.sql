ALTER TABLE deploy_jobs
    ADD COLUMN IF NOT EXISTS release_image_url_mode VARCHAR(32) NOT NULL DEFAULT 'manifest_urls';
