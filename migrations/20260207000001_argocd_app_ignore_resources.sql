ALTER TABLE environment_argocd_apps
ADD COLUMN ignore_resources JSONB NOT NULL DEFAULT '[]';
