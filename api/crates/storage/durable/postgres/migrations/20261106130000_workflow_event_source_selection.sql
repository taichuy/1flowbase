ALTER TABLE workflow_event_triggers ADD COLUMN source jsonb;
COMMENT ON COLUMN workflow_event_triggers.source IS 'Selected registered source identity and trusted scope; NULL retains custom event selection. No event payload.';

-- The protected configuration model projects the same nullable selection field.
INSERT INTO model_fields (id,data_model_id,scope_id,code,title,physical_column_name,field_kind,is_system,is_writable,is_required,api_required,is_unique,display_interface,display_options,sort_order,availability_status)
SELECT gen_random_uuid(),m.id,m.scope_id,'source','Source','source','json',true,false,false,false,false,null,'{}'::jsonb,10,'available'
FROM model_definitions m
WHERE m.code='workflow_event_triggers' AND m.scope_kind='system'
  AND m.scope_id='00000000-0000-0000-0000-000000000000'
  AND m.source_kind='main_source' AND m.owner_kind='core' AND m.is_protected
ON CONFLICT DO NOTHING;
