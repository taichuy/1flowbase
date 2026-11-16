-- These mutable configurations belong to a caller workspace. Keep workspace_id
-- as the repository contract and derive scope_id so alternate writers cannot drift.
ALTER TABLE custom_frontend_blocks ADD COLUMN scope_id uuid GENERATED ALWAYS AS (workspace_id) STORED NOT NULL;
CREATE INDEX custom_frontend_blocks_scope_created_id ON custom_frontend_blocks(scope_id, created_at, id);

ALTER TABLE event_definitions ADD COLUMN scope_id uuid GENERATED ALWAYS AS (workspace_id) STORED NOT NULL;
CREATE INDEX event_definitions_scope_created_id ON event_definitions(scope_id, created_at, id);

ALTER TABLE plugin_record_permission_policies ADD COLUMN scope_id uuid GENERATED ALWAYS AS (workspace_id) STORED NOT NULL;
CREATE INDEX plugin_record_permission_policies_scope_created_id ON plugin_record_permission_policies(scope_id, created_at, id);

-- Existing rows have no recorded creation time; their last known configuration
-- timestamp is the conservative historical backfill. Future updates retain it.
ALTER TABLE workflow_event_triggers ADD COLUMN created_at timestamptz;
UPDATE workflow_event_triggers SET created_at = updated_at;
ALTER TABLE workflow_event_triggers ALTER COLUMN created_at SET NOT NULL;
ALTER TABLE workflow_event_triggers ALTER COLUMN created_at SET DEFAULT now();
ALTER TABLE workflow_event_triggers ADD COLUMN scope_id uuid GENERATED ALWAYS AS (workspace_id) STORED NOT NULL;
CREATE INDEX workflow_event_triggers_scope_created_id ON workflow_event_triggers(scope_id, created_at, id);
