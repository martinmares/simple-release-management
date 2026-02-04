CREATE TABLE IF NOT EXISTS deploy_job_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deploy_job_id UUID NOT NULL REFERENCES deploy_jobs(id) ON DELETE CASCADE,
    log_line TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_deploy_job_logs_job_id ON deploy_job_logs(deploy_job_id);
