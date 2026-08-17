create unique index model_pricing_rules_schedule_identity_uidx
    on model_pricing_rules (
        provider_code,
        upstream_model_id,
        priority,
        effective_from,
        effective_to,
        timezone,
        weekday_mask,
        local_time_start,
        local_time_end
    ) nulls not distinct;
