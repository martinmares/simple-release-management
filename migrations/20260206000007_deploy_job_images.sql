CREATE TABLE IF NOT EXISTS deploy_job_images (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deploy_job_id UUID NOT NULL REFERENCES deploy_jobs(id) ON DELETE CASCADE,
    file_path TEXT NOT NULL,
    container_name TEXT NOT NULL,
    image TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_deploy_job_images_job_id ON deploy_job_images(deploy_job_id);
