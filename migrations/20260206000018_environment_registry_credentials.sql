CREATE TABLE IF NOT EXISTS environment_registry_credentials (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    environment_id UUID NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
    registry_id UUID NOT NULL REFERENCES registries(id) ON DELETE CASCADE,
    auth_type TEXT NOT NULL,
    username TEXT,
    password_encrypted TEXT,
    token_encrypted TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (environment_id, registry_id)
);
