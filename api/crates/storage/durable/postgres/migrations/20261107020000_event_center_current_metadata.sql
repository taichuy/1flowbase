-- Current Event Center metadata; safe to replay without reintroducing retired fields.

DELETE FROM model_fields f USING model_definitions m
WHERE f.data_model_id = m.id AND f.code = 'contract_version'
  AND m.code IN ('event_definitions', 'workflow_event_triggers')
  AND m.scope_kind = 'system' AND m.scope_id = '00000000-0000-0000-0000-000000000000'
  AND m.source_kind = 'main_source' AND m.owner_kind = 'core' AND m.is_protected;

-- Protected metadata for durable configuration only. No event instances, payload history,
-- queues or observation logs. Dedicated services own record writes; no workspace grant is seeded.
-- Existing presentation metadata and grants are never reset.
INSERT INTO model_definitions (
    id, scope_kind, scope_id, source_kind, code, title, physical_table_name,
    acl_namespace, audit_namespace, availability_status, status,
    owner_kind, is_protected, template_provider, template_code, template_version
) VALUES
    ('00000000-2611-4000-8000-000000000001','system','00000000-0000-0000-0000-000000000000','main_source','event_definitions','Event definitions','event_definitions','state_model.event_definitions','audit.state_model.event_definitions','available','published','core',true,'core','general','v1'),
    ('00000000-2611-4000-8000-000000000002','system','00000000-0000-0000-0000-000000000000','main_source','workflow_event_triggers','Workflow event triggers','workflow_event_triggers','state_model.workflow_event_triggers','audit.state_model.workflow_event_triggers','available','published','core',true,'core','general','v1')
ON CONFLICT DO NOTHING;

INSERT INTO model_fields (id,data_model_id,scope_id,code,title,physical_column_name,field_kind,is_system,is_writable,is_required,api_required,is_unique,display_interface,display_options,sort_order,availability_status)
SELECT gen_random_uuid(),m.id,m.scope_id,f.code,f.title,f.code,f.kind,true,false,true,false,f.code='id',null,'{}'::jsonb,f.position,'available'
FROM model_definitions m JOIN (VALUES
    ('event_definitions','id','ID','string',0),
    ('event_definitions','workspace_id','Workspace ID','many_to_one',1),
    ('event_definitions','contract_id','Contract ID','string',2),
    ('event_definitions','name','Name','string',4),
    ('event_definitions','description','Description','text',5),
    ('event_definitions','payload_schema','Payload schema','json',6),
    ('event_definitions','enabled','Enabled','boolean',7),
    ('event_definitions','created_by','Created by','string',8),
    ('event_definitions','created_at','Created at','datetime',9),
    ('event_definitions','updated_at','Updated at','datetime',10),
    ('workflow_event_triggers','id','ID','string',0),
    ('workflow_event_triggers','workspace_id','Workspace ID','many_to_one',1),
    ('workflow_event_triggers','application_id','Application ID','many_to_one',2),
    ('workflow_event_triggers','enabled','Enabled','boolean',3),
    ('workflow_event_triggers','contract_id','Contract ID','string',4),
    ('workflow_event_triggers','trigger_phase','Trigger phase','string',6),
    ('workflow_event_triggers','input_mapping','Input mapping','json',7),
    ('workflow_event_triggers','updated_by','Updated by','string',8),
    ('workflow_event_triggers','updated_at','Updated at','datetime',9)
) AS f(model_code,code,title,kind,position) ON m.code=f.model_code
WHERE m.scope_kind='system' AND m.scope_id='00000000-0000-0000-0000-000000000000'
  AND m.source_kind='main_source' AND m.owner_kind='core' AND m.is_protected
ON CONFLICT DO NOTHING;

INSERT INTO model_fields (id,data_model_id,scope_id,code,title,physical_column_name,field_kind,is_system,is_writable,is_required,api_required,is_unique,display_interface,display_options,sort_order,availability_status)
SELECT gen_random_uuid(),m.id,m.scope_id,'source','Source','source','json',true,false,false,false,false,null,'{}'::jsonb,10,'available'
FROM model_definitions m
WHERE m.code='workflow_event_triggers' AND m.scope_kind='system'
  AND m.scope_id='00000000-0000-0000-0000-000000000000'
  AND m.source_kind='main_source' AND m.owner_kind='core' AND m.is_protected
ON CONFLICT DO NOTHING;
