create index if not exists model_provider_request_logs_scope_created_idx
    on model_provider_request_logs (scope_id, created_at asc, id asc);

insert into model_definitions (
    id,
    scope_kind,
    scope_id,
    data_source_instance_id,
    source_kind,
    external_resource_key,
    external_table_id,
    external_capability_snapshot,
    code,
    title,
    physical_table_name,
    acl_namespace,
    audit_namespace,
    availability_status,
    status,
    owner_kind,
    owner_id,
    is_protected,
    created_by,
    updated_by
)
values (
    '00000000-0535-4000-8000-000000000001',
    'system',
    '00000000-0000-0000-0000-000000000000'::uuid,
    null,
    'main_source',
    null,
    null,
    null,
    'model_provider_request_logs',
    'Model provider request logs',
    'model_provider_request_logs',
    'state_model.model_provider_request_logs',
    'audit.state_model.model_provider_request_logs',
    'available',
    'published',
    'core',
    null,
    true,
    null,
    null
)
on conflict do nothing;

update model_definitions
set data_source_instance_id = null,
    source_kind = 'main_source',
    external_resource_key = null,
    external_table_id = null,
    external_capability_snapshot = null,
    physical_table_name = 'model_provider_request_logs',
    acl_namespace = 'state_model.model_provider_request_logs',
    audit_namespace = 'audit.state_model.model_provider_request_logs',
    availability_status = 'available',
    status = 'published',
    owner_kind = 'core',
    owner_id = null,
    is_protected = true,
    updated_at = now()
where scope_kind = 'system'
  and scope_id = '00000000-0000-0000-0000-000000000000'::uuid
  and code = 'model_provider_request_logs';

create temporary table model_provider_request_log_fields (
    field_id uuid primary key,
    code text not null unique,
    title text not null,
    field_kind text not null,
    is_required boolean not null,
    is_unique boolean not null,
    sort_order integer not null
) on commit drop;

insert into model_provider_request_log_fields (
    field_id, code, title, field_kind, is_required, is_unique, sort_order
)
values
    ('00000000-1535-4000-8000-000000000001', 'id', 'ID', 'string', true, true, 0),
    ('00000000-1535-4000-8000-000000000002', 'scope_id', 'Scope ID', 'many_to_one', true, false, 1),
    ('00000000-1535-4000-8000-000000000003', 'attempt_id', 'Attempt ID', 'string', true, true, 2),
    ('00000000-1535-4000-8000-000000000004', 'flow_run_id', 'Flow run ID', 'many_to_one', true, false, 3),
    ('00000000-1535-4000-8000-000000000005', 'application_id', 'Application ID', 'many_to_one', false, false, 4),
    ('00000000-1535-4000-8000-000000000006', 'conversation_id', 'Conversation ID', 'string', false, false, 5),
    ('00000000-1535-4000-8000-000000000007', 'application_name', 'Application name', 'string', true, false, 6),
    ('00000000-1535-4000-8000-000000000008', 'attempt_index', 'Attempt index', 'number', true, false, 7),
    ('00000000-1535-4000-8000-000000000009', 'is_retry', 'Is retry', 'boolean', true, false, 8),
    ('00000000-1535-4000-8000-000000000010', 'retry_reason', 'Retry reason', 'string', false, false, 9),
    ('00000000-1535-4000-8000-000000000011', 'provider_instance_id', 'Provider instance ID', 'many_to_one', false, false, 10),
    ('00000000-1535-4000-8000-000000000012', 'provider_instance_display_name', 'Provider instance display name', 'string', false, false, 11),
    ('00000000-1535-4000-8000-000000000013', 'provider_code', 'Provider code', 'string', true, false, 12),
    ('00000000-1535-4000-8000-000000000014', 'protocol', 'Protocol', 'string', true, false, 13),
    ('00000000-1535-4000-8000-000000000015', 'upstream_model_id', 'Upstream model ID', 'string', true, false, 14),
    ('00000000-1535-4000-8000-000000000016', 'reasoning_effort', 'Reasoning effort', 'string', false, false, 15),
    ('00000000-1535-4000-8000-000000000017', 'status', 'Status', 'string', true, false, 16),
    ('00000000-1535-4000-8000-000000000018', 'error_code', 'Error code', 'string', false, false, 17),
    ('00000000-1535-4000-8000-000000000019', 'failed_after_first_token', 'Failed after first token', 'boolean', true, false, 18),
    ('00000000-1535-4000-8000-000000000020', 'input_tokens', 'Input tokens', 'number', false, false, 19),
    ('00000000-1535-4000-8000-000000000021', 'output_tokens', 'Output tokens', 'number', false, false, 20),
    ('00000000-1535-4000-8000-000000000022', 'total_tokens', 'Total tokens', 'number', false, false, 21),
    ('00000000-1535-4000-8000-000000000023', 'started_at', 'Started at', 'datetime', true, false, 22),
    ('00000000-1535-4000-8000-000000000024', 'first_token_at', 'First token at', 'datetime', false, false, 23),
    ('00000000-1535-4000-8000-000000000025', 'finished_at', 'Finished at', 'datetime', false, false, 24),
    ('00000000-1535-4000-8000-000000000026', 'time_to_first_token_ms', 'Time to first token (ms)', 'number', false, false, 25),
    ('00000000-1535-4000-8000-000000000027', 'total_duration_ms', 'Total duration (ms)', 'number', false, false, 26),
    ('00000000-1535-4000-8000-000000000028', 'created_at', 'Created at', 'datetime', true, false, 27);

insert into model_fields (
    id,
    data_model_id,
    scope_id,
    code,
    title,
    physical_column_name,
    external_field_key,
    field_kind,
    is_system,
    is_writable,
    is_required,
    api_required,
    is_unique,
    default_value,
    display_interface,
    display_options,
    relation_target_model_id,
    relation_options,
    sort_order,
    availability_status,
    created_by,
    updated_by
)
select
    fields.field_id,
    definitions.id,
    definitions.scope_id,
    fields.code,
    fields.title,
    fields.code,
    null,
    fields.field_kind,
    true,
    false,
    fields.is_required,
    false,
    fields.is_unique,
    null,
    null,
    '{}'::jsonb,
    null,
    '{}'::jsonb,
    fields.sort_order,
    'available',
    null,
    null
from model_provider_request_log_fields fields
join model_definitions definitions
  on definitions.scope_kind = 'system'
 and definitions.scope_id = '00000000-0000-0000-0000-000000000000'::uuid
 and definitions.code = 'model_provider_request_logs'
where not exists (
    select 1
    from model_fields existing
    where existing.data_model_id = definitions.id
      and existing.code = fields.code
);

update model_fields target
set scope_id = definitions.scope_id,
    physical_column_name = fields.code,
    external_field_key = null,
    field_kind = fields.field_kind,
    is_system = true,
    is_writable = false,
    is_required = fields.is_required,
    api_required = false,
    is_unique = fields.is_unique,
    default_value = null,
    relation_target_model_id = null,
    relation_options = '{}'::jsonb,
    sort_order = fields.sort_order,
    availability_status = 'available',
    updated_at = now()
from model_provider_request_log_fields fields
join model_definitions definitions
  on definitions.scope_kind = 'system'
 and definitions.scope_id = '00000000-0000-0000-0000-000000000000'::uuid
 and definitions.code = 'model_provider_request_logs'
where target.data_model_id = definitions.id
  and target.code = fields.code;

with request_log_model as (
    select id
    from model_definitions
    where scope_kind = 'system'
      and scope_id = '00000000-0000-0000-0000-000000000000'::uuid
      and code = 'model_provider_request_logs'
), model_scope_grants as (
    select
        'system'::text as scope_kind,
        '00000000-0000-0000-0000-000000000000'::uuid as scope_id,
        request_log_model.id as data_model_id,
        'system_all'::text as permission_profile
    from request_log_model
    union all
    select
        'workspace',
        workspaces.id,
        request_log_model.id,
        'scope_all'
    from workspaces
    cross join request_log_model
), hashed_model_scope_grants as (
    select
        (
            substr(md5(scope_kind || ':' || scope_id::text || ':' || data_model_id::text), 1, 8) || '-' ||
            substr(md5(scope_kind || ':' || scope_id::text || ':' || data_model_id::text), 9, 4) || '-' ||
            substr(md5(scope_kind || ':' || scope_id::text || ':' || data_model_id::text), 13, 4) || '-' ||
            substr(md5(scope_kind || ':' || scope_id::text || ':' || data_model_id::text), 17, 4) || '-' ||
            substr(md5(scope_kind || ':' || scope_id::text || ':' || data_model_id::text), 21, 12)
        )::uuid as id,
        scope_kind,
        scope_id,
        data_model_id,
        permission_profile
    from model_scope_grants
)
insert into scope_data_model_grants (
    id,
    scope_kind,
    scope_id,
    data_model_id,
    enabled,
    permission_profile,
    created_by
)
select
    id,
    scope_kind,
    scope_id,
    data_model_id,
    true,
    permission_profile,
    null
from hashed_model_scope_grants
on conflict (scope_kind, scope_id, data_model_id) do nothing;
