-- A durable definition read model. Event deliveries, runtime leases and queues retain their owners.
CREATE FUNCTION event_catalog_identity(metadata jsonb) RETURNS jsonb
LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE AS $$
    SELECT jsonb_build_array(
        metadata->'scope', metadata->'contract_id',
        metadata#>'{source,selection,source_id}',
        COALESCE(metadata#>'{source,selection,filters}', '{}'::jsonb),
        metadata->'id'
    );
$$;

CREATE TABLE event_catalog (
    metadata jsonb NOT NULL CHECK (jsonb_typeof(metadata) = 'object'),
    identity jsonb GENERATED ALWAYS AS (event_catalog_identity(metadata)) STORED PRIMARY KEY,
    origin text GENERATED ALWAYS AS (metadata->>'origin') STORED NOT NULL
        CHECK (origin IN ('custom', 'core', 'plugin')),
    scope_kind text GENERATED ALWAYS AS (metadata#>>'{scope,kind}') STORED NOT NULL
        CHECK (scope_kind IN ('workspace', 'system', 'unbound')),
    scope_id uuid GENERATED ALWAYS AS (
        CASE WHEN metadata#>>'{scope,kind}' = 'workspace'
            THEN (metadata#>>'{scope,workspace_id}')::uuid
            ELSE '00000000-0000-0000-0000-000000000000'::uuid END
    ) STORED NOT NULL,
    available boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (metadata ? 'contract_id' AND metadata ? 'id'),
    CHECK (NOT (metadata ? 'created_at' OR metadata ? 'updated_at'))
);
CREATE INDEX event_catalog_scope_updated ON event_catalog(scope_kind, scope_id, updated_at DESC);
CREATE INDEX event_catalog_custom_definition ON event_catalog((metadata->>'id')) WHERE origin = 'custom';

CREATE FUNCTION event_catalog_custom_metadata(definition event_definitions) RETURNS jsonb
LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE AS $$
    SELECT jsonb_build_object(
        'id', definition.id,
        'scope', jsonb_build_object('kind', 'workspace', 'workspace_id', definition.workspace_id),
        'interface_event', definition.interface_event,
        'group_path', jsonb_build_array('interface_events'),
        'enabled', definition.enabled, 'owner_id', NULL,
        'contract_id', definition.contract_id, 'name', definition.name,
        'description', definition.description, 'payload_schema', definition.payload_schema,
        'origin', 'custom',
        'supported_phases', jsonb_build_array('subscribed', 'published', 'consumed')
    );
$$;

-- Preserve real custom timestamps. Imported/updated rules project in the same database transaction.
INSERT INTO event_catalog(metadata, created_at, updated_at)
SELECT event_catalog_custom_metadata(definition), definition.created_at, definition.updated_at
FROM event_definitions definition;

CREATE FUNCTION project_custom_event_catalog() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        UPDATE event_catalog SET available = false, updated_at = now()
        WHERE origin = 'custom' AND metadata->>'id' = OLD.id::text AND available;
        RETURN OLD;
    END IF;
    INSERT INTO event_catalog(metadata, created_at, updated_at)
    VALUES (event_catalog_custom_metadata(NEW), NEW.created_at, NEW.updated_at)
    ON CONFLICT (identity) DO UPDATE SET
        metadata = EXCLUDED.metadata, available = true, updated_at = EXCLUDED.updated_at;
    RETURN NEW;
END;
$$;
CREATE TRIGGER event_definitions_catalog_projection
AFTER INSERT OR UPDATE OR DELETE ON event_definitions
FOR EACH ROW EXECUTE FUNCTION project_custom_event_catalog();
