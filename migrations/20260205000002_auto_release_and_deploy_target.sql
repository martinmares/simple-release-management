ALTER TABLE releases
ADD COLUMN is_auto boolean NOT NULL DEFAULT false,
ADD COLUMN auto_reason text;

ALTER TABLE deploy_targets
ADD COLUMN allow_auto_release boolean NOT NULL DEFAULT false;
