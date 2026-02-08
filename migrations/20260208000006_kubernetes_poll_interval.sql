ALTER TABLE environments
    ADD COLUMN IF NOT EXISTS kubernetes_poll_interval_seconds INT NOT NULL DEFAULT 0;
