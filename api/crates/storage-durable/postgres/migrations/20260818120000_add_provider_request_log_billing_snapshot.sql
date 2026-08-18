alter table model_provider_request_logs
    add column if not exists pricing_provider_code text,
    add column if not exists pricing_model_id text,
    add column if not exists total_cost numeric(38, 18),
    add column if not exists currency_code text;

alter table model_provider_request_logs
    add constraint model_provider_request_logs_billing_snapshot_check
    check (
        (
            pricing_provider_code is null
            and pricing_model_id is null
            and total_cost is null
            and currency_code is null
        )
        or (
            pricing_provider_code is not null
            and pricing_model_id is not null
            and total_cost >= 0
            and currency_code = 'USD'
        )
    );

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
from (
    values
        ('00000000-1535-4000-8000-000000000035'::uuid, 'pricing_provider_code', 'Pricing provider code', 'string', 34),
        ('00000000-1535-4000-8000-000000000036'::uuid, 'pricing_model_id', 'Pricing model ID', 'string', 35),
        ('00000000-1535-4000-8000-000000000037'::uuid, 'total_cost', 'Total cost', 'number', 36),
        ('00000000-1535-4000-8000-000000000038'::uuid, 'currency_code', 'Currency code', 'string', 37)
) as fields(id, code, title, field_kind, sort_order)
join model_definitions definitions
  on definitions.scope_kind = 'system'
 and definitions.scope_id = '00000000-0000-0000-0000-000000000000'::uuid
 and definitions.code = 'model_provider_request_logs'
on conflict (data_model_id, code) do nothing;
