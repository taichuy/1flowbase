-- Keep the persistent event catalog projection aligned with typed chat definitions.
CREATE OR REPLACE FUNCTION event_catalog_custom_metadata(definition event_definitions) RETURNS jsonb
LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE AS $$
    SELECT jsonb_build_object(
        'id', definition.id,
        'scope', jsonb_build_object('kind', 'workspace', 'workspace_id', definition.workspace_id),
        'event_kind', definition.event_kind,
        'interface_event', definition.interface_event,
        'group_path', jsonb_build_array(CASE
            WHEN definition.event_kind = 'communication_command' THEN 'communication_commands'
            WHEN definition.interface_event IS NOT NULL THEN 'interface_events'
            ELSE 'custom_events' END),
        'enabled', definition.enabled, 'owner_id', NULL,
        'contract_id', definition.contract_id, 'name', definition.name,
        'description', definition.description, 'payload_schema', definition.payload_schema,
        'origin', 'custom',
        'supported_phases', jsonb_build_array('subscribed', 'published', 'consumed')
    );
$$;

UPDATE event_catalog AS catalog
SET metadata = event_catalog_custom_metadata(definition)
FROM event_definitions AS definition
WHERE catalog.origin = 'custom' AND catalog.metadata->>'id' = definition.id::text
  AND catalog.metadata IS DISTINCT FROM event_catalog_custom_metadata(definition);
