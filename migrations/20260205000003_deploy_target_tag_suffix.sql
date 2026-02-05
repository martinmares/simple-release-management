ALTER TABLE deploy_targets
ADD COLUMN append_env_suffix boolean NOT NULL DEFAULT false;
