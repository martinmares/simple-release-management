ALTER TABLE bundles
ADD COLUMN auto_tag_enabled boolean NOT NULL DEFAULT false;

CREATE TABLE IF NOT EXISTS bundle_tag_counters (
    bundle_id uuid NOT NULL REFERENCES bundles(id) ON DELETE CASCADE,
    target_registry_id uuid NOT NULL REFERENCES registries(id) ON DELETE CASCADE,
    date date NOT NULL,
    counter integer NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (bundle_id, target_registry_id, date)
);
