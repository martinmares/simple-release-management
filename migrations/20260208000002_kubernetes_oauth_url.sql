ALTER TABLE kubernetes_instances
    ADD COLUMN IF NOT EXISTS oauth_base_url TEXT;
