with legacy_models as (
    select distinct
        instance.provider_code,
        configured_model ->> 'model_id' as upstream_model_id
    from model_provider_instances instance
    cross join lateral jsonb_array_elements(instance.configured_models_json) configured_model
    where jsonb_typeof(configured_model) = 'object'
      and nullif(btrim(configured_model ->> 'model_id'), '') is not null
)
insert into model_pricing_rules (
    id,
    provider_code,
    upstream_model_id,
    input_token_unit_size,
    input_token_unit_price,
    output_token_unit_size,
    output_token_unit_price,
    cache_hit_token_unit_size,
    cache_hit_token_unit_price,
    currency_code,
    effective_from,
    effective_to,
    timezone,
    weekday_mask,
    local_time_start,
    local_time_end,
    priority,
    enabled,
    source_kind,
    source_catalog_id,
    source_version,
    source_checksum,
    extensions,
    created_by
)
select
    gen_random_uuid(),
    legacy.provider_code,
    legacy.upstream_model_id,
    1000000,
    0,
    1000000,
    0,
    1000000,
    0,
    'USD',
    now(),
    null,
    'UTC',
    127,
    null,
    null,
    0,
    true,
    'manual',
    null,
    null,
    null,
    jsonb_build_object(
        'origin', 'upgrade_compat',
        'policy', 'legacy_configured_model_zero'
    ),
    null
from legacy_models legacy
where not exists (
    select 1
    from model_pricing_rules existing
    where existing.provider_code = legacy.provider_code
      and existing.upstream_model_id = legacy.upstream_model_id
);
