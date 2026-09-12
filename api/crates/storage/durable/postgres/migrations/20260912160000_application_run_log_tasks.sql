-- #2035: the task is the list unit. One row per client turn (or per run without
-- client identity). The run projection stays one row per run; the task row is
-- recomputed by the projection writer whenever a member run's summary changes.
create table application_run_log_tasks (
    id uuid primary key references application_run_log_summaries(flow_run_id) on delete cascade,
    application_id uuid not null references applications(id) on delete cascade,
    scope_id uuid not null references workspaces(id) on delete cascade,
    member_run_ids uuid[] not null check (cardinality(member_run_ids) >= 1),
    parent_task_run_id uuid references flow_runs(id) on delete set null,
    is_root boolean generated always as (parent_task_run_id is null) stored,
    log_conversation_id uuid references application_conversations(id) on delete set null,
    client_thread_id text,
    client_turn_id text,
    subagent_kind text,
    run_mode text not null,
    status text not null,
    outcome text not null check (outcome in ('final_answer_observed','in_progress','no_final_answer')),
    user_input text,
    final_output text,
    final_output_run_id uuid references flow_runs(id) on delete set null,
    target_node_id text,
    title text not null,
    external_user text,
    created_by uuid,
    authorized_account text,
    api_key_id uuid,
    api_key_name_snapshot text,
    publication_version_id uuid,
    external_conversation_id text,
    external_trace_id text,
    compatibility_mode text,
    idempotency_key text,
    call_kind text not null,
    invocation_count bigint not null default 0,
    compaction_count bigint not null default 0,
    total_tokens bigint,
    input_tokens bigint,
    output_tokens bigint,
    input_cache_hit_tokens bigint,
    input_cache_hit_rate double precision,
    unique_node_count bigint not null default 0,
    tool_callback_count bigint not null default 0,
    started_at timestamptz not null,
    finished_at timestamptz,
    created_at timestamptz not null,
    updated_at timestamptz not null
);
create index application_run_log_tasks_root_page on application_run_log_tasks(application_id, is_root, created_at desc, id desc);
create index application_run_log_tasks_started on application_run_log_tasks(application_id, is_root, started_at desc, id desc);
create index application_run_log_tasks_parent on application_run_log_tasks(parent_task_run_id) where parent_task_run_id is not null;
create index application_run_log_tasks_scope on application_run_log_tasks(scope_id, created_at desc);

-- The final answer of a task is the last assistant message projected for any
-- member call that the client marked as a final answer, or the run answer of
-- a run without native output. Reading it never rebuilds projections.
create function application_run_log_task_final_output(members uuid[])
returns table(run_id uuid, content text)
language sql stable as $$
    select m.flow_run_id, coalesce(m.content, m.answer)
    from application_run_conversation_message_items m
    join unnest($1) with ordinality member(run_id, position) on member.run_id = m.flow_run_id
    where (m.role = 'assistant' and m.native_message #>> '{_source_item,phase}' = 'final_answer')
       or (m.role is null and m.answer is not null and m.native_message is null)
    order by member.position desc, m.display_sequence desc
    limit 1
$$;

create function application_run_log_task_refresh(anchor uuid) returns void
language sql as $$
    with members as (
        select s.*, f.log_context
        from application_run_log_summaries s
        join flow_runs f on f.id = s.flow_run_id
        where s.flow_run_id = $1 or s.log_task_run_id = $1
    ), anchor_row as (
        select * from members where flow_run_id = $1
    ), ordered as (
        select array_agg(flow_run_id order by started_at, flow_run_id) as member_run_ids from members
    ), aggregate as (
        select
            sum(invocation_count)::bigint as invocation_count,
            sum(compaction_count)::bigint as compaction_count,
            sum(total_tokens)::bigint as total_tokens,
            sum(input_tokens)::bigint as input_tokens,
            sum(output_tokens)::bigint as output_tokens,
            sum(input_cache_hit_tokens)::bigint as input_cache_hit_tokens,
            sum(unique_node_count)::bigint as unique_node_count,
            sum(tool_callback_count)::bigint as tool_callback_count,
            min(started_at) as started_at,
            min(created_at) as created_at,
            max(updated_at) as updated_at,
            case when bool_and(finished_at is not null) then max(finished_at) end as finished_at,
            (array_agg(status order by case status
                when 'running' then 0 when 'waiting_callback' then 1
                when 'waiting_human' then 2 when 'paused' then 3
                when 'queued' then 4 when 'failed' then 5
                when 'cancelled' then 6 when 'incomplete' then 7
                when 'succeeded' then 8 else 9 end,
                created_at desc, flow_run_id))[1] as status,
            bool_or(status in ('queued','running','waiting_callback','waiting_human','paused')) as active
        from members
    ), final_output as (
        select * from application_run_log_task_final_output((select member_run_ids from ordered))
    )
    insert into application_run_log_tasks (
        id, application_id, scope_id, member_run_ids, parent_task_run_id, log_conversation_id,
        client_thread_id, client_turn_id, subagent_kind, run_mode, status, outcome,
        user_input, final_output, final_output_run_id, target_node_id, title, external_user,
        created_by, authorized_account, api_key_id, api_key_name_snapshot, publication_version_id,
        external_conversation_id, external_trace_id, compatibility_mode, idempotency_key, call_kind,
        invocation_count, compaction_count, total_tokens, input_tokens, output_tokens,
        input_cache_hit_tokens, input_cache_hit_rate, unique_node_count, tool_callback_count,
        started_at, finished_at, created_at, updated_at
    )
    select
        a.flow_run_id, a.application_id, a.scope_id, o.member_run_ids, a.parent_run_id, a.log_conversation_id,
        a.log_context ->> 'thread_id', a.log_context ->> 'turn_id', a.log_context ->> 'subagent_kind',
        a.run_mode, g.status,
        -- A later active call reopens the task even after an answer was observed;
        -- the observed answer stays on the row but does not claim completion.
        case when g.active then 'in_progress'
             when fo.content is not null then 'final_answer_observed'
             else 'no_final_answer' end,
        -- The projected user message is already plain text (content parts are
        -- flattened there); the raw client prompt only helps when it is a string.
        coalesce(
            (select m.content from application_run_conversation_message_items m
              where m.flow_run_id = a.flow_run_id and m.role = 'user' order by m.display_sequence limit 1),
            (select m.query from application_run_conversation_message_items m
              where m.flow_run_id = a.flow_run_id and m.query is not null order by m.display_sequence limit 1),
            case when jsonb_typeof(a.log_context #> '{prompt,content}') = 'string'
                 then a.log_context #>> '{prompt,content}' end),
        fo.content, fo.run_id, a.target_node_id, a.title, a.external_user,
        a.created_by, a.authorized_account, a.api_key_id, a.api_key_name_snapshot, a.publication_version_id,
        a.external_conversation_id, a.external_trace_id, a.compatibility_mode, a.idempotency_key, a.call_kind,
        g.invocation_count, g.compaction_count, g.total_tokens, g.input_tokens, g.output_tokens,
        g.input_cache_hit_tokens,
        case when coalesce(g.input_tokens,0) + g.input_cache_hit_tokens > 0
             then g.input_cache_hit_tokens::double precision / (coalesce(g.input_tokens,0) + g.input_cache_hit_tokens)::double precision end,
        g.unique_node_count, g.tool_callback_count,
        g.started_at, g.finished_at, g.created_at, g.updated_at
    from anchor_row a cross join ordered o cross join aggregate g left join final_output fo on true
    on conflict (id) do update set
        member_run_ids = excluded.member_run_ids, parent_task_run_id = excluded.parent_task_run_id,
        log_conversation_id = excluded.log_conversation_id, client_thread_id = excluded.client_thread_id,
        client_turn_id = excluded.client_turn_id, subagent_kind = excluded.subagent_kind,
        run_mode = excluded.run_mode, status = excluded.status, outcome = excluded.outcome,
        user_input = excluded.user_input, final_output = excluded.final_output,
        final_output_run_id = excluded.final_output_run_id, target_node_id = excluded.target_node_id,
        title = excluded.title, external_user = excluded.external_user, created_by = excluded.created_by,
        authorized_account = excluded.authorized_account, api_key_id = excluded.api_key_id,
        api_key_name_snapshot = excluded.api_key_name_snapshot, publication_version_id = excluded.publication_version_id,
        external_conversation_id = excluded.external_conversation_id, external_trace_id = excluded.external_trace_id,
        compatibility_mode = excluded.compatibility_mode, idempotency_key = excluded.idempotency_key,
        call_kind = excluded.call_kind, invocation_count = excluded.invocation_count,
        compaction_count = excluded.compaction_count, total_tokens = excluded.total_tokens,
        input_tokens = excluded.input_tokens, output_tokens = excluded.output_tokens,
        input_cache_hit_tokens = excluded.input_cache_hit_tokens, input_cache_hit_rate = excluded.input_cache_hit_rate,
        unique_node_count = excluded.unique_node_count, tool_callback_count = excluded.tool_callback_count,
        started_at = excluded.started_at, finished_at = excluded.finished_at,
        created_at = excluded.created_at, updated_at = excluded.updated_at;
$$;

-- Backfill every existing run into its task. A run whose anchor is another run
-- is only a member; the anchor row carries it.
select application_run_log_task_refresh(anchor)
from (select distinct coalesce(log_task_run_id, flow_run_id) as anchor from application_run_log_summaries) anchors;

do $$ begin
 if exists(select 1 from application_run_log_summaries s where not exists(
    select 1 from application_run_log_tasks t where t.id = coalesce(s.log_task_run_id, s.flow_run_id))) then
  raise exception 'task projection: run without task';
 end if;
 if (select count(*) from application_run_log_summaries) <> (select coalesce(sum(cardinality(member_run_ids)),0) from application_run_log_tasks) then
  raise exception 'task projection: member count mismatch';
 end if;
end $$;

-- The read-time task aggregation is retired with its shared scope helper; the
-- task table now owns membership.
drop function application_run_log_task_runs(uuid, uuid);

-- Runtime Data Model registration (read-only, mirrors the summaries model).
insert into model_definitions (
    id, scope_kind, scope_id, data_source_instance_id, source_kind, external_resource_key,
    external_table_id, external_capability_snapshot, code, title, physical_table_name,
    acl_namespace, audit_namespace, availability_status, status, owner_kind, owner_id,
    is_protected, created_by, updated_by, template_provider, template_code, template_version
) values (
    '00000000-0536-4000-8000-000000000001', 'system', '00000000-0000-0000-0000-000000000000'::uuid,
    null, 'main_source', null, null, null, 'application_run_log_tasks', 'Application run log tasks',
    'application_run_log_tasks', 'state_model.application_run_log_tasks',
    'audit.state_model.application_run_log_tasks', 'available', 'published', 'core', null, true, null, null,
    'core', 'general', 'v1'
) on conflict do nothing;

create temporary table application_run_log_task_fields (
    field_id uuid primary key, code text not null unique, title text not null, field_kind text not null,
    is_required boolean not null, is_unique boolean not null, sort_order integer not null
) on commit drop;
insert into application_run_log_task_fields values
    ('00000000-1536-4000-8000-000000000001','id','ID','string',true,true,0),
    ('00000000-1536-4000-8000-000000000002','application_id','Application ID','many_to_one',true,false,1),
    ('00000000-1536-4000-8000-000000000003','scope_id','Scope ID','many_to_one',true,false,2),
    ('00000000-1536-4000-8000-000000000004','member_run_ids','Member run IDs','json',true,false,3),
    ('00000000-1536-4000-8000-000000000005','parent_task_run_id','Parent task run ID','string',false,false,4),
    ('00000000-1536-4000-8000-000000000006','is_root','Is root','boolean',true,false,5),
    ('00000000-1536-4000-8000-000000000007','log_conversation_id','Log conversation ID','string',false,false,6),
    ('00000000-1536-4000-8000-000000000008','client_thread_id','Client thread ID','string',false,false,7),
    ('00000000-1536-4000-8000-000000000009','client_turn_id','Client turn ID','string',false,false,8),
    ('00000000-1536-4000-8000-000000000010','subagent_kind','Subagent kind','string',false,false,9),
    ('00000000-1536-4000-8000-000000000011','run_mode','Run mode','string',true,false,10),
    ('00000000-1536-4000-8000-000000000012','status','Status','string',true,false,11),
    ('00000000-1536-4000-8000-000000000013','outcome','Outcome','string',true,false,12),
    ('00000000-1536-4000-8000-000000000014','user_input','User input','text',false,false,13),
    ('00000000-1536-4000-8000-000000000015','final_output','Final output','text',false,false,14),
    ('00000000-1536-4000-8000-000000000016','final_output_run_id','Final output run ID','string',false,false,15),
    ('00000000-1536-4000-8000-000000000017','target_node_id','Target node ID','string',false,false,16),
    ('00000000-1536-4000-8000-000000000018','title','Title','string',true,false,17),
    ('00000000-1536-4000-8000-000000000019','external_user','External user','string',false,false,18),
    ('00000000-1536-4000-8000-000000000020','created_by','Created by','string',false,false,19),
    ('00000000-1536-4000-8000-000000000021','authorized_account','Authorized account','string',false,false,20),
    ('00000000-1536-4000-8000-000000000022','api_key_id','API key ID','many_to_one',false,false,21),
    ('00000000-1536-4000-8000-000000000023','api_key_name_snapshot','API key name snapshot','string',false,false,22),
    ('00000000-1536-4000-8000-000000000024','publication_version_id','Publication version ID','many_to_one',false,false,23),
    ('00000000-1536-4000-8000-000000000025','external_conversation_id','External conversation ID','string',false,false,24),
    ('00000000-1536-4000-8000-000000000026','external_trace_id','External trace ID','string',false,false,25),
    ('00000000-1536-4000-8000-000000000027','compatibility_mode','Compatibility mode','string',false,false,26),
    ('00000000-1536-4000-8000-000000000028','idempotency_key','Idempotency key','string',false,false,27),
    ('00000000-1536-4000-8000-000000000029','call_kind','Call kind','string',true,false,28),
    ('00000000-1536-4000-8000-000000000030','invocation_count','Invocation count','number',true,false,29),
    ('00000000-1536-4000-8000-000000000031','compaction_count','Compaction count','number',true,false,30),
    ('00000000-1536-4000-8000-000000000032','total_tokens','Total tokens','number',false,false,31),
    ('00000000-1536-4000-8000-000000000033','input_tokens','Input tokens','number',false,false,32),
    ('00000000-1536-4000-8000-000000000034','output_tokens','Output tokens','number',false,false,33),
    ('00000000-1536-4000-8000-000000000035','input_cache_hit_tokens','Input cache hit tokens','number',false,false,34),
    ('00000000-1536-4000-8000-000000000036','input_cache_hit_rate','Input cache hit rate','number',false,false,35),
    ('00000000-1536-4000-8000-000000000037','unique_node_count','Unique node count','number',true,false,36),
    ('00000000-1536-4000-8000-000000000038','tool_callback_count','Tool callback count','number',true,false,37),
    ('00000000-1536-4000-8000-000000000039','started_at','Started at','datetime',true,false,38),
    ('00000000-1536-4000-8000-000000000040','finished_at','Finished at','datetime',false,false,39),
    ('00000000-1536-4000-8000-000000000041','created_at','Created at','datetime',true,false,40),
    ('00000000-1536-4000-8000-000000000042','updated_at','Updated at','datetime',true,false,41);

insert into model_fields (
    id, data_model_id, scope_id, code, title, physical_column_name, external_field_key, field_kind,
    is_system, is_writable, is_required, api_required, is_unique, default_value, display_interface,
    display_options, relation_target_model_id, relation_options, sort_order, availability_status,
    created_by, updated_by
)
select fields.field_id, definitions.id, definitions.scope_id, fields.code, fields.title, fields.code, null,
    fields.field_kind, true, false, fields.is_required, false, fields.is_unique, null, null, '{}'::jsonb,
    null, '{}'::jsonb, fields.sort_order, 'available', null, null
from application_run_log_task_fields fields
join model_definitions definitions on definitions.scope_kind = 'system'
    and definitions.scope_id = '00000000-0000-0000-0000-000000000000'::uuid
    and definitions.code = 'application_run_log_tasks'
where not exists (select 1 from model_fields existing where existing.data_model_id = definitions.id and existing.code = fields.code);

with task_model as (
    select id from model_definitions where scope_kind = 'system'
      and scope_id = '00000000-0000-0000-0000-000000000000'::uuid and code = 'application_run_log_tasks'
), model_scope_grants as (
    select 'system'::text as scope_kind, '00000000-0000-0000-0000-000000000000'::uuid as scope_id,
        task_model.id as data_model_id, 'system_all'::text as permission_profile from task_model
    union all
    select 'workspace', workspaces.id, task_model.id, 'scope_all' from workspaces cross join task_model
), hashed as (
    select (
        substr(md5(scope_kind || ':' || scope_id::text || ':' || data_model_id::text), 1, 8) || '-' ||
        substr(md5(scope_kind || ':' || scope_id::text || ':' || data_model_id::text), 9, 4) || '-' ||
        substr(md5(scope_kind || ':' || scope_id::text || ':' || data_model_id::text), 13, 4) || '-' ||
        substr(md5(scope_kind || ':' || scope_id::text || ':' || data_model_id::text), 17, 4) || '-' ||
        substr(md5(scope_kind || ':' || scope_id::text || ':' || data_model_id::text), 21, 12)
    )::uuid as id, scope_kind, scope_id, data_model_id, permission_profile from model_scope_grants
)
insert into scope_data_model_grants (id, scope_kind, scope_id, data_model_id, enabled, permission_profile, created_by)
select id, scope_kind, scope_id, data_model_id, true, permission_profile, null from hashed
on conflict (scope_kind, scope_id, data_model_id) do nothing;
