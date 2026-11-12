-- Existing free-form definitions retain their historical shape and command mappings.
ALTER TABLE event_definitions ADD COLUMN event_kind text;
UPDATE event_definitions SET event_kind = 'interface' WHERE interface_event IS NOT NULL;
ALTER TABLE event_definitions ADD CONSTRAINT event_definitions_event_kind
  CHECK (event_kind IS NULL
    OR (event_kind = 'interface' AND interface_event IS NOT NULL)
    OR (event_kind = 'communication_command' AND interface_event IS NULL));

INSERT INTO model_fields (id,data_model_id,scope_id,code,title,physical_column_name,field_kind,is_system,is_writable,is_required,api_required,is_unique,display_interface,display_options,sort_order,availability_status)
SELECT gen_random_uuid(),m.id,m.scope_id,'event_kind','Event kind','event_kind','string',true,false,false,false,false,null,'{}'::jsonb,12,'available'
FROM model_definitions m
WHERE m.code='event_definitions' AND m.scope_kind='system'
  AND m.scope_id='00000000-0000-0000-0000-000000000000'
  AND m.source_kind='main_source' AND m.owner_kind='core' AND m.is_protected
ON CONFLICT DO NOTHING;
