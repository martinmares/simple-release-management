ALTER TABLE copy_jobs
    ADD COLUMN IF NOT EXISTS environment_id UUID REFERENCES environments(id);

CREATE INDEX IF NOT EXISTS idx_copy_jobs_environment
    ON copy_jobs(environment_id);
