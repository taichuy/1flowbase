-- Event Center is a fresh configuration feature: discard only its authorized draft
-- definitions, triggers and user event command bindings. Original durable protocol rows,
-- applications and other communication configuration remain owned by their existing services.
DELETE FROM workflow_event_triggers;
DELETE FROM event_definitions;
UPDATE communication_bindings
SET config_json = config_json - 'event_commands'
WHERE config_json ? 'event_commands';

ALTER TABLE event_definitions DROP COLUMN contract_version;
ALTER TABLE event_definitions ADD CONSTRAINT event_definitions_workspace_contract_unique
    UNIQUE (workspace_id, contract_id);
ALTER TABLE workflow_event_triggers DROP COLUMN contract_version;
