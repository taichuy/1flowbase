delete from model_pricing_rules
where (
        extensions ->> 'origin' = 'upgrade_compat'
        and extensions ->> 'policy' = 'legacy_configured_model_zero'
    )
    or (
        extensions ->> 'pricing_policy' = 'official_zero_default'
        and extensions ->> 'reason' = 'upgrade_compatibility'
    );

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
values (
    '10000000-0000-4000-8000-000000000001',
    'zero',
    'any',
    1000000,
    0,
    1000000,
    0,
    1000000,
    0,
    'USD',
    '2026-08-17T00:00:00Z',
    null,
    'UTC',
    127,
    null,
    null,
    0,
    true,
    'official',
    '10000000-0000-4000-8000-000000000001',
    '2026-08-17.3',
    'sha256:8b0d406bb9b1bb66616f82f291a84180a99b55d41415f6f11bcfdc9d51b8cb82',
    '{"pricing_policy":"global_zero_fallback","owner":"model_pricing_catalog"}'::jsonb,
    null
)
on conflict (id) do update set
    provider_code = excluded.provider_code,
    upstream_model_id = excluded.upstream_model_id,
    input_token_unit_size = excluded.input_token_unit_size,
    input_token_unit_price = excluded.input_token_unit_price,
    output_token_unit_size = excluded.output_token_unit_size,
    output_token_unit_price = excluded.output_token_unit_price,
    cache_hit_token_unit_size = excluded.cache_hit_token_unit_size,
    cache_hit_token_unit_price = excluded.cache_hit_token_unit_price,
    currency_code = excluded.currency_code,
    effective_from = excluded.effective_from,
    effective_to = excluded.effective_to,
    timezone = excluded.timezone,
    weekday_mask = excluded.weekday_mask,
    local_time_start = excluded.local_time_start,
    local_time_end = excluded.local_time_end,
    priority = excluded.priority,
    enabled = excluded.enabled,
    source_kind = excluded.source_kind,
    source_catalog_id = excluded.source_catalog_id,
    source_version = excluded.source_version,
    source_checksum = excluded.source_checksum,
    extensions = excluded.extensions,
    updated_at = now();
