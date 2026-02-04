-- Persisted copy job logs
CREATE TABLE IF NOT EXISTS copy_job_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    copy_job_id UUID NOT NULL REFERENCES copy_jobs(id) ON DELETE CASCADE,
    line TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_copy_job_logs_job ON copy_job_logs(copy_job_id);
CREATE INDEX IF NOT EXISTS idx_copy_job_logs_created ON copy_job_logs(created_at);
