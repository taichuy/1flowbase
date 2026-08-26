alter table application_run_log_summaries
    add column input_tokens bigint,
    add column output_tokens bigint,
    add column input_cache_hit_tokens bigint;

with usage_totals as (
    select
        flow_run_id,
        sum(input_tokens)::bigint as input_tokens,
        sum(output_tokens)::bigint as output_tokens,
        sum(input_cache_hit_tokens)::bigint as input_cache_hit_tokens
    from runtime_usage_ledger
    group by flow_run_id
)
update application_run_log_summaries summaries
set input_tokens = usage_totals.input_tokens,
    output_tokens = usage_totals.output_tokens,
    input_cache_hit_tokens = usage_totals.input_cache_hit_tokens,
    log_updated_at = now()
from usage_totals
where summaries.flow_run_id = usage_totals.flow_run_id
  and (
    summaries.input_tokens is distinct from usage_totals.input_tokens
    or summaries.output_tokens is distinct from usage_totals.output_tokens
    or summaries.input_cache_hit_tokens is distinct from usage_totals.input_cache_hit_tokens
  );

update model_fields target
set sort_order = sort_order + 3,
    updated_at = now()
from model_definitions definitions
where target.data_model_id = definitions.id
  and definitions.data_source_instance_id is null
  and definitions.code = 'application_run_log_summaries'
  and target.sort_order >= 18;

create temporary table application_run_log_token_breakdown_fields (
    field_id uuid primary key,
    code text not null,
    title text not null,
    physical_column_name text not null,
    sort_order integer not null
) on commit drop;

insert into application_run_log_token_breakdown_fields (
    field_id,
    code,
    title,
    physical_column_name,
    sort_order
)
values
    ('00000000-1532-4000-8000-000000000025', 'input_tokens', 'Input tokens', 'input_tokens', 18),
    ('00000000-1532-4000-8000-000000000026', 'output_tokens', 'Output tokens', 'output_tokens', 19),
    ('00000000-1532-4000-8000-000000000027', 'input_cache_hit_tokens', 'Input cache hit tokens', 'input_cache_hit_tokens', 20);

insert into model_fields (
    id,
    data_model_id,
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
    fields.field_id,
    definitions.id,
    fields.code,
    fields.title,
    fields.physical_column_name,
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
    fields.sort_order,
    'available',
    null,
    null
from application_run_log_token_breakdown_fields fields
join model_definitions definitions
  on definitions.data_source_instance_id is null
 and definitions.code = 'application_run_log_summaries'
where not exists (
    select 1
    from model_fields existing
    where existing.data_model_id = definitions.id
      and existing.code = fields.code
);

update model_fields target
set
    title = fields.title,
    physical_column_name = fields.physical_column_name,
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
    sort_order = fields.sort_order,
    availability_status = 'available',
    updated_at = now()
from application_run_log_token_breakdown_fields fields
join model_definitions definitions
  on definitions.data_source_instance_id is null
 and definitions.code = 'application_run_log_summaries'
where target.data_model_id = definitions.id
  and target.code = fields.code;
