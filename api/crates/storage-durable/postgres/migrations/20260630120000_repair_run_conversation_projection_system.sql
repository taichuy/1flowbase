create temporary table application_run_conversation_system_projection_repair on commit drop as
with projected_runs as (
    select distinct
        flow_run_id,
        projection_version
    from application_run_conversation_message_items
),
available_system as (
    select
        projected_runs.flow_run_id,
        projected_runs.projection_version,
        applications.workspace_id as scope_id,
        flow_runs.application_id,
        coalesce(
            case when jsonb_typeof(flow_runs.input_payload #> '{node-start,system}') = 'string'
                then nullif(btrim(flow_runs.input_payload #>> '{node-start,system}'), '') end,
            case when jsonb_typeof(flow_runs.input_payload #> '{start,system}') = 'string'
                then nullif(btrim(flow_runs.input_payload #>> '{start,system}'), '') end,
            case when jsonb_typeof(flow_runs.input_payload -> 'system') = 'string'
                then nullif(btrim(flow_runs.input_payload #>> '{system}'), '') end,
            (
                select nullif(btrim(prompt_message #>> '{content}'), '')
                from node_runs
                cross join lateral jsonb_array_elements(
                    case
                        when jsonb_typeof(node_runs.input_payload -> 'prompt_messages') = 'array'
                        then node_runs.input_payload -> 'prompt_messages'
                        else '[]'::jsonb
                    end
                ) as prompt_message
                where node_runs.flow_run_id = flow_runs.id
                  and node_runs.node_type = 'llm'
                  and prompt_message #>> '{role}' = 'system'
                  and jsonb_typeof(prompt_message -> 'content') = 'string'
                  and nullif(btrim(prompt_message #>> '{content}'), '') is not null
                order by node_runs.started_at asc, node_runs.id asc
                limit 1
            ),
            (
                select nullif(btrim(node_runs.debug_payload #>> '{llm_context,effective_system}'), '')
                from node_runs
                where node_runs.flow_run_id = flow_runs.id
                  and node_runs.node_type = 'llm'
                  and jsonb_typeof(node_runs.debug_payload #> '{llm_context,effective_system}') = 'string'
                  and nullif(btrim(node_runs.debug_payload #>> '{llm_context,effective_system}'), '') is not null
                order by node_runs.started_at asc, node_runs.id asc
                limit 1
            )
        ) as system_content,
        coalesce(
            case when jsonb_typeof(flow_runs.input_payload -> 'model') = 'string'
                then nullif(btrim(flow_runs.input_payload #>> '{model}'), '') end,
            case when jsonb_typeof(flow_runs.input_payload #> '{node-start,model}') = 'string'
                then nullif(btrim(flow_runs.input_payload #>> '{node-start,model}'), '') end,
            case when jsonb_typeof(flow_runs.input_payload #> '{start,model}') = 'string'
                then nullif(btrim(flow_runs.input_payload #>> '{start,model}'), '') end
        ) as model,
        flow_runs.started_at,
        flow_runs.finished_at,
        flow_runs.created_at,
        flow_runs.updated_at
    from projected_runs
    join flow_runs on flow_runs.id = projected_runs.flow_run_id
    join applications on applications.id = flow_runs.application_id
    where not exists (
        select 1
        from application_run_conversation_message_items system_items
        where system_items.flow_run_id = projected_runs.flow_run_id
          and system_items.projection_version = projected_runs.projection_version
          and system_items.role = 'system'
    )
)
select *
from available_system
where system_content is not null;

update application_run_conversation_message_items items
set display_sequence = display_sequence + 1000000000,
    updated_at = greatest(items.updated_at, repair.updated_at)
from application_run_conversation_system_projection_repair repair
where items.flow_run_id = repair.flow_run_id
  and items.projection_version = repair.projection_version;

insert into application_run_conversation_message_items (
    id,
    scope_id,
    application_id,
    flow_run_id,
    display_sequence,
    source_kind,
    role,
    content,
    query,
    model,
    answer,
    detail_run_id,
    can_open_detail,
    is_current,
    status,
    started_at,
    finished_at,
    projection_version,
    created_at,
    updated_at
)
select
    (
        substr(md5('application_run_conversation_message_items:system:' || flow_run_id::text || ':' || projection_version::text), 1, 8)
        || '-'
        || substr(md5('application_run_conversation_message_items:system:' || flow_run_id::text || ':' || projection_version::text), 9, 4)
        || '-'
        || substr(md5('application_run_conversation_message_items:system:' || flow_run_id::text || ':' || projection_version::text), 13, 4)
        || '-'
        || substr(md5('application_run_conversation_message_items:system:' || flow_run_id::text || ':' || projection_version::text), 17, 4)
        || '-'
        || substr(md5('application_run_conversation_message_items:system:' || flow_run_id::text || ':' || projection_version::text), 21, 12)
    )::uuid,
    scope_id,
    application_id,
    flow_run_id,
    0,
    'imported_context',
    'system',
    system_content,
    null,
    model,
    null,
    null,
    false,
    false,
    'succeeded',
    started_at,
    finished_at,
    projection_version,
    created_at,
    updated_at
from application_run_conversation_system_projection_repair
on conflict do nothing;

update application_run_conversation_message_items items
set display_sequence = display_sequence - 999999999
from application_run_conversation_system_projection_repair repair
where items.flow_run_id = repair.flow_run_id
  and items.projection_version = repair.projection_version
  and items.display_sequence >= 1000000000;
