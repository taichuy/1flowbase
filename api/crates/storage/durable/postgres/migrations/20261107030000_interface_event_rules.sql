-- Additive interface rule binding. Existing definitions and their references are preserved.
ALTER TABLE event_definitions ADD COLUMN IF NOT EXISTS interface_event jsonb;
ALTER TABLE event_definitions ADD CONSTRAINT event_definitions_interface_event_object
  CHECK (interface_event IS NULL OR jsonb_typeof(interface_event) = 'object');

INSERT INTO model_fields (id,data_model_id,scope_id,code,title,physical_column_name,field_kind,is_system,is_writable,is_required,api_required,is_unique,display_interface,display_options,sort_order,availability_status)
SELECT gen_random_uuid(),m.id,m.scope_id,'interface_event','Interface event','interface_event','json',true,false,false,false,false,null,'{}'::jsonb,11,'available'
FROM model_definitions m
WHERE m.code='event_definitions' AND m.scope_kind='system'
  AND m.scope_id='00000000-0000-0000-0000-000000000000'
  AND m.source_kind='main_source' AND m.owner_kind='core' AND m.is_protected
ON CONFLICT DO NOTHING;
