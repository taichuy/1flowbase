alter table application_run_log_summaries
    add column input_cache_hit_rate double precision;

update application_run_log_summaries
set input_cache_hit_rate = case
        when input_cache_hit_tokens is not null
         and coalesce(input_tokens, 0) + input_cache_hit_tokens > 0
        then input_cache_hit_tokens::double precision
           / (coalesce(input_tokens, 0) + input_cache_hit_tokens)::double precision
        else null
    end,
    log_updated_at = now()
where input_cache_hit_rate is distinct from case
        when input_cache_hit_tokens is not null
         and coalesce(input_tokens, 0) + input_cache_hit_tokens > 0
        then input_cache_hit_tokens::double precision
           / (coalesce(input_tokens, 0) + input_cache_hit_tokens)::double precision
        else null
    end;

update model_fields target
set sort_order = sort_order + 1,
    updated_at = now()
from model_definitions definitions
where target.data_model_id = definitions.id
  and definitions.data_source_instance_id is null
  and definitions.code = 'application_run_log_summaries'
  and target.sort_order >= 21;

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
    '00000000-1532-4000-8000-000000000028',
    definitions.id,
    definitions.scope_id,
    'input_cache_hit_rate',
    'Input cache hit rate',
    'input_cache_hit_rate',
    null,
    'number',
    true,
    false,
    false,
    false,
    null,
    null,
    '{}'::jsonb,
    null,
    '{}'::jsonb,
    21,
    'available',
    null,
    null
from model_definitions definitions
where definitions.data_source_instance_id is null
  and definitions.code = 'application_run_log_summaries'
  and not exists (
      select 1
      from model_fields existing
      where existing.data_model_id = definitions.id
        and existing.code = 'input_cache_hit_rate'
  );

update model_fields target
set
    title = 'Input cache hit rate',
    physical_column_name = 'input_cache_hit_rate',
    external_field_key = null,
    field_kind = 'number',
    is_system = true,
    is_writable = false,
    is_required = false,
    is_unique = false,
    default_value = null,
    display_interface = null,
    display_options = '{}'::jsonb,
    relation_target_model_id = null,
    relation_options = '{}'::jsonb,
    sort_order = 21,
    availability_status = 'available',
    updated_at = now()
from model_definitions definitions
where target.data_model_id = definitions.id
  and definitions.data_source_instance_id is null
  and definitions.code = 'application_run_log_summaries'
  and target.code = 'input_cache_hit_rate';
