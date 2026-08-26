alter table model_pricing_rules
    add column rating_policy_enabled boolean not null default false,
    add column rating_policy jsonb not null default '{}'::jsonb
        check (jsonb_typeof(rating_policy) = 'object'),
    add constraint model_pricing_rules_enabled_rating_policy_not_empty
        check (not rating_policy_enabled or rating_policy <> '{}'::jsonb);

do $$
declare
    constraint_name text;
begin
    select conname into constraint_name
    from pg_constraint
    where conrelid = 'model_pricing_rules'::regclass
      and contype = 'c'
      and pg_get_constraintdef(oid) like '%local_time_start < local_time_end%';
    if constraint_name is not null then
        execute format(
            'alter table model_pricing_rules drop constraint %I',
            constraint_name
        );
    end if;
end
$$;

alter table model_pricing_rules
    add constraint model_pricing_rules_local_time_window
        check (
            local_time_start is null
            or (
                local_time_start <> local_time_end
                and (
                    local_time_start < local_time_end
                    or weekday_mask = 127
                )
            )
        );
