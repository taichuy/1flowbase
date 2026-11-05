-- Only configuration is durable. No event instances, delivery queues or observation logs.
CREATE TABLE event_definitions (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    contract_id TEXT NOT NULL CHECK (contract_id ~ '^custom\.[a-z0-9_-]+(\.[a-z0-9_-]+)*$'),
    contract_version TEXT NOT NULL CHECK (btrim(contract_version) <> ''),
    name TEXT NOT NULL CHECK (btrim(name) <> ''),
    description TEXT NOT NULL,
    payload_schema JSONB NOT NULL,
    enabled BOOLEAN NOT NULL,
    created_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (workspace_id, contract_id, contract_version)
);
CREATE TABLE workflow_event_triggers (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    application_id UUID NOT NULL REFERENCES applications(id) ON DELETE CASCADE,
    enabled BOOLEAN NOT NULL,
    contract_id TEXT NOT NULL,
    contract_version TEXT NOT NULL,
    trigger_phase TEXT NOT NULL CHECK (trigger_phase IN ('subscribed', 'published', 'consumed')),
    input_mapping JSONB NOT NULL CHECK (jsonb_typeof(input_mapping) = 'object'),
    updated_by UUID NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (workspace_id, application_id)
);
CREATE INDEX workflow_event_triggers_enabled ON workflow_event_triggers(workspace_id) WHERE enabled;
