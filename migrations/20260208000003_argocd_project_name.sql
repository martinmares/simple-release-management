ALTER TABLE environment_argocd_apps
    ADD COLUMN IF NOT EXISTS project_name TEXT;

UPDATE environment_argocd_apps
SET project_name = COALESCE(project_name, 'default');

ALTER TABLE environment_argocd_apps
    ALTER COLUMN project_name SET NOT NULL;
