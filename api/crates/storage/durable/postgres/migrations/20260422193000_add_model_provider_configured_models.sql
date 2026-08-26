alter table model_provider_instances
    add column configured_models_json jsonb not null default '[]'::jsonb;

update model_provider_instances
set configured_models_json = coalesce(
    (
        select jsonb_agg(
            jsonb_build_object(
                'model_id', enabled_model_id,
                'enabled', true
            )
        )
        from unnest(enabled_model_ids) as enabled_model_id
    ),
    '[]'::jsonb
);
