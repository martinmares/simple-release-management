CREATE TABLE IF NOT EXISTS deploy_job_diffs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deploy_job_id UUID NOT NULL REFERENCES deploy_jobs(id) ON DELETE CASCADE,
    files_changed TEXT NOT NULL,
    diff_patch TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_deploy_job_diffs_job_id ON deploy_job_diffs(deploy_job_id);
