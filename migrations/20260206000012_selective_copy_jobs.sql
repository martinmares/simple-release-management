ALTER TABLE copy_jobs
ADD COLUMN is_selective BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE copy_jobs
ADD COLUMN base_copy_job_id UUID;

ALTER TABLE copy_job_images
ADD COLUMN source_registry_id UUID;
