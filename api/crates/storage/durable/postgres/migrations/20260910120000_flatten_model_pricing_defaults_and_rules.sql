-- Defaults have one durable owner; conditional rules only override named fields.
-- Convert each installed record in place. User schedules, identities, and billing
-- references are preserved; official source consolidation happens in the catalog.
alter table model_pricing_rules
    add column cache_write_token_unit_size bigint,
    add column cache_write_token_unit_price numeric,
    add column rules jsonb not null default '[]'::jsonb;

-- Legacy prices counted cache creation as ordinary input. Moving that meter out
-- of input therefore uses the same rate, not an inferred vendor cache quote.
update model_pricing_rules
set cache_write_token_unit_size = input_token_unit_size,
    cache_write_token_unit_price = input_token_unit_price;

do $$
declare
    pricing record;
    policy jsonb;
    defaults jsonb;
    tiers jsonb;
    tier jsonb;
    rates jsonb;
    converted jsonb;
    replacements jsonb;
    condition jsonb;
    write_rates jsonb;
    write_price text;
    bucket record;
    meter text;
    unit_size bigint;
begin
    for pricing in select * from model_pricing_rules where rating_policy_enabled loop
        policy := pricing.rating_policy;
        converted := '[]'::jsonb;
        if policy->>'schema_version' = '1flowbase.model-rating-policy/v2' then
            unit_size := (policy->>'unit_size')::bigint;
            defaults := policy->'rates';
            write_rates := defaults->'cache_write';
            if write_rates ? 'unit_price' then
                write_price := write_rates->>'unit_price';
            else
                select value into strict write_price
                from jsonb_each_text(write_rates->'by_ttl_seconds')
                order by key::bigint limit 1;
            end if;
            update model_pricing_rules set
                input_token_unit_size = unit_size,
                input_token_unit_price = (defaults->>'input')::numeric,
                output_token_unit_size = unit_size,
                output_token_unit_price = (defaults->>'output')::numeric,
                cache_hit_token_unit_size = unit_size,
                cache_hit_token_unit_price = (defaults->>'cache_hit')::numeric,
                cache_write_token_unit_size = unit_size,
                cache_write_token_unit_price = write_price::numeric
            where id = pricing.id;
            if write_rates ? 'by_ttl_seconds' then
                for bucket in select key, value from jsonb_each_text(write_rates->'by_ttl_seconds') order by key::bigint loop
                    if bucket.value::numeric <> write_price::numeric then
                        converted := converted || jsonb_build_array(jsonb_build_object(
                            'when', jsonb_build_object('cache_write_ttl_seconds', bucket.key::bigint),
                            'overrides', jsonb_build_object('cache_write_token_unit_price', bucket.value)));
                    end if;
                end loop;
            end if;
            tiers := coalesce(policy->'input_token_tiers', '[]'::jsonb);
        elsif policy->>'schema_version' = '1flowbase.model-rating-policy/v1' then
            tiers := policy->'tiers';
        else
            raise exception 'unsupported stored model pricing policy for %', pricing.id;
        end if;

        for tier in select value from jsonb_array_elements(tiers) loop
            rates := tier->'rates';
            condition := jsonb_build_object('input_tokens', tier->'when');
            replacements := '{}'::jsonb;
            foreach meter in array array['input','output','cache_hit'] loop
                if policy->>'schema_version' = '1flowbase.model-rating-policy/v2' then
                    replacements := replacements || jsonb_build_object(
                        meter || '_token_unit_size', unit_size,
                        meter || '_token_unit_price', rates->>meter);
                else
                    replacements := replacements || jsonb_build_object(
                        meter || '_token_unit_size', (rates->meter->>'unit_size')::bigint,
                        meter || '_token_unit_price', rates->meter->>'unit_price');
                end if;
            end loop;
            if policy->>'schema_version' = '1flowbase.model-rating-policy/v2' then
                write_rates := rates->'cache_write';
                if write_rates ? 'unit_price' then
                    write_price := write_rates->>'unit_price';
                else
                    select value into strict write_price
                    from jsonb_each_text(write_rates->'by_ttl_seconds') order by key::bigint limit 1;
                end if;
                replacements := replacements || jsonb_build_object(
                    'cache_write_token_unit_size', unit_size, 'cache_write_token_unit_price', write_price);
            else
                replacements := replacements || jsonb_build_object(
                    'cache_write_token_unit_size', (rates->'input'->>'unit_size')::bigint,
                    'cache_write_token_unit_price', rates->'input'->>'unit_price');
            end if;
            converted := converted || jsonb_build_array(jsonb_build_object('when', condition, 'overrides', replacements));
            if policy->>'schema_version' = '1flowbase.model-rating-policy/v2' and write_rates ? 'by_ttl_seconds' then
                for bucket in select key, value from jsonb_each_text(write_rates->'by_ttl_seconds') order by key::bigint loop
                    if bucket.value::numeric <> write_price::numeric then
                        converted := converted || jsonb_build_array(jsonb_build_object(
                            'when', condition || jsonb_build_object('cache_write_ttl_seconds', bucket.key::bigint),
                            'overrides', jsonb_build_object('cache_write_token_unit_price', bucket.value)));
                    end if;
                end loop;
            end if;
        end loop;
        update model_pricing_rules set rules = converted where id = pricing.id;
    end loop;
end $$;

alter table model_pricing_rules
    alter column cache_write_token_unit_size set not null,
    alter column cache_write_token_unit_size set default 1000000,
    alter column cache_write_token_unit_price set not null,
    alter column cache_write_token_unit_price set default 0,
    add constraint model_pricing_cache_write_unit_positive check (cache_write_token_unit_size > 0),
    add constraint model_pricing_cache_write_price_nonnegative check (cache_write_token_unit_price >= 0),
    add constraint model_pricing_rules_array check (jsonb_typeof(rules) = 'array'),
    drop column rating_policy_enabled,
    drop column rating_policy;

-- Retire only the removed system-owned physical field bindings. Other model
-- titles, descriptions, user fields and grants are untouched.
delete from model_fields fields using model_definitions models
where fields.data_model_id = models.id
  and models.physical_table_name = 'model_pricing_rules'
  and models.scope_kind = 'system'
  and models.data_source_instance_id is null
  and fields.code in ('rating_policy_enabled', 'rating_policy');
