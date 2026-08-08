alter table model_provider_request_logs
    add column if not exists user_id uuid,
    add column if not exists user_account text;

alter table model_provider_request_logs
    add constraint model_provider_request_logs_user_attribution_pair_check
    check (
        (user_id is null and user_account is null)
        or (user_id is not null and user_account is not null)
    );

create index if not exists model_provider_request_logs_scope_user_started_idx
    on model_provider_request_logs (scope_id, user_id, started_at desc, id desc)
    where user_id is not null;

create temporary table model_provider_request_log_user_fields (
    field_id uuid primary key,
    code text not null unique,
    title text not null,
    field_kind text not null,
    sort_order integer not null
) on commit drop;

insert into model_provider_request_log_user_fields (
    field_id, code, title, field_kind, sort_order
)
values
    ('00000000-1535-4000-8000-000000000030', 'user_id', 'User ID', 'many_to_one', 29),
    ('00000000-1535-4000-8000-000000000031', 'user_account', 'User account', 'string', 30);

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
    false,
    false,
    false,
    null,
    null,
    '{}'::jsonb,
    null,
    '{}'::jsonb,
    fields.sort_order,
    'available',
    null,
    null
from model_provider_request_log_user_fields fields
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

-- Reconcile only the system-owned physical contract; presentation metadata remains user-owned.
update model_fields target
set scope_id = definitions.scope_id,
    physical_column_name = fields.code,
    external_field_key = null,
    field_kind = fields.field_kind,
    is_system = true,
    is_writable = false,
    is_required = false,
    api_required = false,
    is_unique = false,
    availability_status = 'available',
    updated_at = now()
from model_provider_request_log_user_fields fields
join model_definitions definitions
  on definitions.scope_kind = 'system'
 and definitions.scope_id = '00000000-0000-0000-0000-000000000000'::uuid
 and definitions.code = 'model_provider_request_logs'
where target.data_model_id = definitions.id
  and target.code = fields.code;
