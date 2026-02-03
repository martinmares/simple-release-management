-- Add credential fields to registries table
ALTER TABLE registries
    ADD COLUMN auth_type VARCHAR(50) CHECK (auth_type IN ('none', 'basic', 'token', 'bearer')),
    ADD COLUMN username VARCHAR(255),
    ADD COLUMN password_encrypted TEXT,
    ADD COLUMN token_encrypted TEXT,
    ADD COLUMN description TEXT,
    ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT true;

-- Drop the old credentials_path column
ALTER TABLE registries DROP COLUMN credentials_path;

-- Set default auth_type for existing rows
UPDATE registries SET auth_type = 'none' WHERE auth_type IS NULL;

-- Make auth_type NOT NULL after setting defaults
ALTER TABLE registries ALTER COLUMN auth_type SET NOT NULL;
