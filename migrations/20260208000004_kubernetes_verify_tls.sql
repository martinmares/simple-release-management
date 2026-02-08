ALTER TABLE kubernetes_instances
    ADD COLUMN IF NOT EXISTS verify_tls BOOLEAN NOT NULL DEFAULT TRUE;

UPDATE kubernetes_instances
SET verify_tls = NOT insecure
WHERE verify_tls IS DISTINCT FROM NOT insecure;

ALTER TABLE kubernetes_instances
    DROP COLUMN IF EXISTS insecure;
