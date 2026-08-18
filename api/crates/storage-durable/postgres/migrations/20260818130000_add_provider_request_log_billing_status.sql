alter table model_provider_request_logs
    add column if not exists billing_status text;

alter table model_provider_request_logs
    add constraint model_provider_request_logs_billing_status_check
    check (
        billing_status is null
        or billing_status in ('settled', 'pending', 'reconciliation_failed')
    );

insert into model_fields (
    id, data_model_id, scope_id, code, title, physical_column_name,
    external_field_key, field_kind, is_system, is_writable, is_required,
    api_required, is_unique, default_value, display_interface, display_options,
    relation_target_model_id, relation_options, sort_order, availability_status,
    created_by, updated_by
)
select
    '00000000-1535-4000-8000-000000000039'::uuid,
    definitions.id,
    definitions.scope_id,
    'billing_status',
    'Billing status',
    'billing_status',
    null,
    'string',
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
    38,
    'available',
    null,
    null
from model_definitions definitions
where definitions.scope_kind = 'system'
  and definitions.scope_id = '00000000-0000-0000-0000-000000000000'::uuid
  and definitions.code = 'model_provider_request_logs'
on conflict (data_model_id, code) do nothing;
