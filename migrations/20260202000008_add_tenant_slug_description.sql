-- Add slug and description to tenants table (idempotent)
DO $$
BEGIN
    -- Add slug column if it doesn't exist
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_name = 'tenants' AND column_name = 'slug') THEN
        ALTER TABLE tenants ADD COLUMN slug VARCHAR(100);
    END IF;

    -- Add description column if it doesn't exist
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_name = 'tenants' AND column_name = 'description') THEN
        ALTER TABLE tenants ADD COLUMN description TEXT;
    END IF;
END $$;

-- For existing rows, generate slug from name
UPDATE tenants
SET slug = lower(regexp_replace(regexp_replace(name, '[^a-zA-Z0-9\s-]', '', 'g'), '\s+', '-', 'g'))
WHERE slug IS NULL;

-- Make slug NOT NULL and UNIQUE after populating
DO $$
BEGIN
    -- Add NOT NULL constraint if not present
    IF EXISTS (SELECT 1 FROM information_schema.columns
               WHERE table_name = 'tenants' AND column_name = 'slug' AND is_nullable = 'YES') THEN
        ALTER TABLE tenants ALTER COLUMN slug SET NOT NULL;
    END IF;

    -- Add unique constraint if not present
    IF NOT EXISTS (SELECT 1 FROM pg_constraint
                   WHERE conname = 'tenants_slug_key') THEN
        ALTER TABLE tenants ADD CONSTRAINT tenants_slug_key UNIQUE (slug);
    END IF;

    -- Create index if it doesn't exist
    IF NOT EXISTS (SELECT 1 FROM pg_indexes
                   WHERE indexname = 'idx_tenants_slug') THEN
        CREATE INDEX idx_tenants_slug ON tenants(slug);
    END IF;
END $$;
