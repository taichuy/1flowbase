alter table model_provider_request_logs
    add column if not exists plugin_id text,
    add column if not exists input_cache_hit_tokens bigint,
    add column if not exists input_cache_hit_rate double precision;

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
    fields.id,
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
from (
    values
        ('00000000-1535-4000-8000-000000000032'::uuid, 'plugin_id', 'Plugin ID', 'string', false, false, 31),
        ('00000000-1535-4000-8000-000000000033'::uuid, 'input_cache_hit_tokens', 'Input cache hit tokens', 'number', false, false, 32),
        ('00000000-1535-4000-8000-000000000034'::uuid, 'input_cache_hit_rate', 'Input cache hit rate', 'number', false, false, 33)
) as fields(id, code, title, field_kind, is_required, is_unique, sort_order)
join model_definitions definitions
  on definitions.scope_kind = 'system'
 and definitions.scope_id = '00000000-0000-0000-0000-000000000000'::uuid
 and definitions.code = 'model_provider_request_logs'
on conflict (data_model_id, code) do nothing;
