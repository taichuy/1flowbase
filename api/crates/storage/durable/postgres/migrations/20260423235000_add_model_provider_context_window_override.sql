update model_provider_instances
set configured_models_json = coalesce(
    (
        select jsonb_agg(
            case
                when jsonb_typeof(configured_model) = 'object'
                    and not (configured_model ? 'context_window_override_tokens')
                    then configured_model || jsonb_build_object('context_window_override_tokens', null)
                else configured_model
            end
        )
        from jsonb_array_elements(configured_models_json) as configured_model
    ),
    '[]'::jsonb
)
where exists (
    select 1
    from jsonb_array_elements(configured_models_json) as configured_model
    where jsonb_typeof(configured_model) = 'object'
      and not (configured_model ? 'context_window_override_tokens')
);
