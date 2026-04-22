ALTER TABLE copy_jobs
    ADD COLUMN IF NOT EXISTS current_transfer_stage VARCHAR(64),
    ADD COLUMN IF NOT EXISTS current_transfer_message TEXT,
    ADD COLUMN IF NOT EXISTS current_bytes_copied BIGINT,
    ADD COLUMN IF NOT EXISTS current_total_bytes BIGINT;
