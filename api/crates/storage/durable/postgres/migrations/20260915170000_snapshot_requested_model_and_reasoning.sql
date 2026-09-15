-- Log-list fields are immutable request snapshots. Pagination continues to read
-- application_run_log_tasks only; JSON extraction is limited to writes/backfill.
alter table application_run_log_summaries
    add column requested_model_id text,
    add column reasoning_effort text;

alter table application_run_log_tasks
    add column requested_model_id text,
    add column reasoning_effort text;

update application_run_log_summaries summaries
set requested_model_id = coalesce(
        runs.input_payload #>> '{sys,requested_model_id}',
        runs.input_payload #>> '{node-start,model}'
    ),
    reasoning_effort = runs.input_payload #>> '{sys,model_parameters,reasoning,effort}'
from flow_runs runs
where runs.id = summaries.flow_run_id;

update application_run_log_tasks tasks
set requested_model_id = summaries.requested_model_id,
    reasoning_effort = summaries.reasoning_effort
from application_run_log_summaries summaries
where summaries.flow_run_id = tasks.id;

-- The existing task refresh owns all other task fields. This write-side trigger
-- copies the anchor summary snapshots into every inserted/refreshed task row.
create function application_run_log_task_request_snapshot_sync()
returns trigger
language plpgsql as $$
begin
    select summaries.requested_model_id, summaries.reasoning_effort
    into new.requested_model_id, new.reasoning_effort
    from application_run_log_summaries summaries
    where summaries.flow_run_id = new.id;
    return new;
end
$$;

create trigger application_run_log_task_request_snapshot_sync
before insert or update on application_run_log_tasks
for each row execute function application_run_log_task_request_snapshot_sync();

insert into model_fields (
    id, data_model_id, scope_id, code, title, physical_column_name, external_field_key, field_kind,
    is_system, is_writable, is_required, api_required, is_unique, default_value, display_interface,
    display_options, relation_target_model_id, relation_options, sort_order, availability_status,
    created_by, updated_by
)
select fields.field_id, definitions.id, definitions.scope_id, fields.code, fields.title, fields.code,
    null, 'string', true, false, false, false, false, null, null, '{}'::jsonb, null, '{}'::jsonb,
    fields.sort_order, 'available', null, null
from (values
    ('00000000-1636-4000-8000-000000000001'::uuid, 'requested_model_id', 'Requested model ID', 1000),
    ('00000000-1636-4000-8000-000000000002'::uuid, 'reasoning_effort', 'Reasoning effort', 1001)
) fields(field_id, code, title, sort_order)
join model_definitions definitions on definitions.scope_kind = 'system'
    and definitions.scope_id = '00000000-0000-0000-0000-000000000000'::uuid
    and definitions.code = 'application_run_log_summaries'
where not exists (
    select 1 from model_fields existing
    where existing.data_model_id = definitions.id and existing.code = fields.code
);

insert into model_fields (
    id, data_model_id, scope_id, code, title, physical_column_name, external_field_key, field_kind,
    is_system, is_writable, is_required, api_required, is_unique, default_value, display_interface,
    display_options, relation_target_model_id, relation_options, sort_order, availability_status,
    created_by, updated_by
)
select fields.field_id, definitions.id, definitions.scope_id, fields.code, fields.title, fields.code,
    null, 'string', true, false, false, false, false, null, null, '{}'::jsonb, null, '{}'::jsonb,
    fields.sort_order, 'available', null, null
from (values
    ('00000000-1636-4000-8000-000000000003'::uuid, 'requested_model_id', 'Requested model ID', 42),
    ('00000000-1636-4000-8000-000000000004'::uuid, 'reasoning_effort', 'Reasoning effort', 43)
) fields(field_id, code, title, sort_order)
join model_definitions definitions on definitions.scope_kind = 'system'
    and definitions.scope_id = '00000000-0000-0000-0000-000000000000'::uuid
    and definitions.code = 'application_run_log_tasks'
where not exists (
    select 1 from model_fields existing
    where existing.data_model_id = definitions.id and existing.code = fields.code
);

do $$
begin
    if exists (
        select 1
        from application_run_log_tasks tasks
        join application_run_log_summaries summaries on summaries.flow_run_id = tasks.id
        where tasks.requested_model_id is distinct from summaries.requested_model_id
           or tasks.reasoning_effort is distinct from summaries.reasoning_effort
    ) then
        raise exception 'task projection: request snapshots differ from anchor summary';
    end if;
end
$$;
