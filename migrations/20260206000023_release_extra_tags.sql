ALTER TABLE copy_jobs
    ADD COLUMN IF NOT EXISTS extra_tags TEXT[];

ALTER TABLE releases
    ADD COLUMN IF NOT EXISTS extra_tags TEXT[];
