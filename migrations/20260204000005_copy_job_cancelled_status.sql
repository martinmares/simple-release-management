ALTER TABLE copy_jobs
    DROP CONSTRAINT IF EXISTS copy_jobs_status_check,
    ADD CONSTRAINT copy_jobs_status_check
        CHECK (status IN ('pending', 'in_progress', 'success', 'failed', 'cancelled'));

ALTER TABLE copy_job_images
    DROP CONSTRAINT IF EXISTS copy_job_images_copy_status_check,
    ADD CONSTRAINT copy_job_images_copy_status_check
        CHECK (copy_status IN ('pending', 'in_progress', 'success', 'failed', 'cancelled'));
