ALTER TABLE deploy_targets
    ALTER COLUMN environments_repo_url DROP NOT NULL,
    ALTER COLUMN deploy_repo_url DROP NOT NULL;
