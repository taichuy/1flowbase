-- Preserve the immutable workspace/template/version key while adding platform row identity.
ALTER TABLE application_template_releases
    ADD COLUMN id uuid NOT NULL DEFAULT gen_random_uuid(),
    ADD COLUMN scope_id uuid GENERATED ALWAYS AS (workspace_id) STORED NOT NULL,
    ADD COLUMN created_at timestamptz,
    ADD COLUMN created_by uuid,
    ADD COLUMN updated_by uuid;

-- Existing releases only retain their last known timestamp; do not invent an earlier one.
UPDATE application_template_releases SET created_at = updated_at;
ALTER TABLE application_template_releases
    ALTER COLUMN id DROP DEFAULT,
    ALTER COLUMN created_at SET DEFAULT now(),
    ALTER COLUMN created_at SET NOT NULL;

CREATE UNIQUE INDEX application_template_releases_id_idx
    ON application_template_releases (id);
CREATE INDEX application_template_releases_scope_created_id_idx
    ON application_template_releases (scope_id, created_at DESC, id DESC);
