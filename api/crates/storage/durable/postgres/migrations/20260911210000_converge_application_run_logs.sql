-- Forward-only convergence. Public conversation/history and execution payloads
-- are not modified. All guards run in the migration transaction before retirement.
alter table flow_runs add column log_context jsonb;
alter table application_conversations add column client_protocol text, add column client_thread_id text;
alter table application_run_log_summaries
    add column log_conversation_id uuid references application_conversations(id) on delete set null,
    add column log_task_run_id uuid references flow_runs(id) on delete set null,
    add column invocation_count bigint not null default 1 check (invocation_count = 1),
    add column parent_run_id uuid references flow_runs(id) on delete set null,
    add column caused_by_run_id uuid references flow_runs(id) on delete set null;
alter table application_run_conversation_message_items
    add column log_conversation_id uuid references application_conversations(id) on delete set null,
    add column source_item_key text;

do $$ begin
 if exists(select 1 from gateway_log_invocations g join flow_runs f on f.id=g.flow_run_id
    where f.scope_id<>g.scope_id or f.application_id<>g.application_id or f.api_key_id is distinct from g.api_key_id) then
  raise exception 'log convergence: invocation authorization domain mismatch';
 end if;
 if exists(select 1 from gateway_log_invocations g where not exists(
    select 1 from application_run_log_summaries s where s.flow_run_id=g.flow_run_id)) then
  raise exception 'log convergence: original summary missing';
 end if;
end $$;

insert into application_conversations(id,scope_id,application_id,api_key_id,external_user,external_conversation_id,created_at,updated_at,client_protocol,client_thread_id)
select id,scope_id,application_id,api_key_id,nullif(external_user,''),'log:responses:'||id,created_at,updated_at,protocol,thread_id
from gateway_log_conversations;

with tasks as (
 select distinct on(turn_id) turn_id,flow_run_id from gateway_log_invocations
 where turn_id is not null order by turn_id,flow_run_id
)
update flow_runs f set log_context=g.context||jsonb_build_object(
 'log_conversation_id',g.conversation_id,'log_task_run_id',tasks.flow_run_id,
 'caused_by_run_id',g.caused_by_run_id,'identity_status',g.identity_status,
 'legacy_turn_id',t.id,'parent_task_id',t.parent_task_id,
 'parent_conversation_id',t.parent_conversation_id,'relation_status',t.relation_status,
 'protocol','openai_responses')
from gateway_log_invocations g left join gateway_log_turns t on t.id=g.turn_id
left join tasks on tasks.turn_id=g.turn_id where f.id=g.flow_run_id;

-- Resolve retired task references to an existing original run as well as
-- retaining the original declaration IDs for audit.
update flow_runs f set log_context=f.log_context||jsonb_build_object('parent_run_id',parent.flow_run_id)
from gateway_log_invocations g join gateway_log_turns t on t.id=g.turn_id
join lateral (select flow_run_id from gateway_log_invocations p where p.turn_id=t.parent_task_id order by flow_run_id limit 1) parent on true
where f.id=g.flow_run_id;

-- Preserve even orphaned task declarations in the existing conversation's
-- retained run context; the current database has no such orphans. Abort rather
-- than discard a declaration that cannot be attached to an original run.
do $$ begin
 if exists(select 1 from gateway_log_turns t where not exists(select 1 from gateway_log_invocations g where g.turn_id=t.id)) then
  raise exception 'log convergence: task without original run';
 end if;
 if exists(select 1 from gateway_log_invocations g join flow_runs f on f.id=g.flow_run_id where f.log_context is null or not f.log_context @> g.context) then
  raise exception 'log convergence: identity preservation failed';
 end if;
end $$;

update application_run_log_summaries s set
 log_conversation_id=(f.log_context->>'log_conversation_id')::uuid,
 log_task_run_id=(f.log_context->>'log_task_run_id')::uuid,
 parent_run_id=(f.log_context->>'parent_run_id')::uuid,
 caused_by_run_id=(f.log_context->>'caused_by_run_id')::uuid
from flow_runs f where f.id=s.flow_run_id and f.log_context is not null;

insert into application_run_conversation_message_items(
 id,scope_id,application_id,flow_run_id,display_sequence,source_kind,role,content,
 detail_run_id,can_open_detail,is_current,status,started_at,finished_at,
 projection_version,native_message,log_conversation_id,source_item_key)
select md5(o.owner_id::text||':'||o.item_key)::uuid,g.scope_id,g.application_id,o.flow_run_id,
 row_number() over(partition by o.flow_run_id order by o.sequence,o.item_key),
 'current_run','assistant',null,o.flow_run_id,true,true,f.status,o.observed_at,f.finished_at,
 3,jsonb_build_object('role','assistant','content','','_source_item',o.item,'_log_conflicting',o.conflicting),o.conversation_id,'output:'||o.item_key
from gateway_log_output_items o join gateway_log_invocations g using(flow_run_id) join flow_runs f on f.id=o.flow_run_id;

-- Result input is a retained original fact even when the provider call was not
-- captured. Keep its type/call ID and conflict marker; do not claim execution.
insert into application_run_conversation_message_items(
 id,scope_id,application_id,flow_run_id,display_sequence,source_kind,role,content,
 detail_run_id,can_open_detail,is_current,status,started_at,finished_at,
 projection_version,native_message,log_conversation_id,source_item_key)
select md5(r.conversation_id::text||':result:'||r.call_id)::uuid,g.scope_id,g.application_id,r.flow_run_id,
 1000000+row_number() over(partition by r.flow_run_id order by r.call_id),
 'current_run','tool',null,r.flow_run_id,true,true,f.status,r.received_at,f.finished_at,
 3,jsonb_build_object('role','tool','content','','tool_call_id',r.call_id,'_source_item',r.result,'_log_conflicting',r.conflicting),r.conversation_id,'result:'||r.call_id
from gateway_log_tool_results r join gateway_log_invocations g using(flow_run_id) join flow_runs f on f.id=r.flow_run_id;

update flow_runs f set log_context=log_context||jsonb_build_object(
 'conflicting_output_keys',coalesce((select jsonb_agg(o.item_key) from gateway_log_output_items o where o.flow_run_id=f.id and o.conflicting),'[]'::jsonb),
 'conflicting_result_call_ids',coalesce((select jsonb_agg(r.call_id) from gateway_log_tool_results r where r.flow_run_id=f.id and r.conflicting),'[]'::jsonb))
where log_context is not null;

-- Every source row must have an exact destination and retained rebuild source.

do $$ begin
 if exists(select 1 from gateway_log_output_items o where not exists(select 1 from runtime_events e where e.flow_run_id=o.flow_run_id and e.event_type='provider_output_item_done' and e.payload->'item'=o.item)) then
  raise exception 'log convergence: retained formal output source missing';
 end if;
 if exists(select 1 from gateway_log_tool_results r join flow_runs f on f.id=r.flow_run_id where not coalesce((f.log_context->'tool_results') @> jsonb_build_array(r.result),false)) then
  raise exception 'log convergence: retained tool input source missing';
 end if;
 if exists(select 1 from gateway_log_conversations g left join application_conversations c on c.id=g.id
 where c.id is null or c.scope_id<>g.scope_id or c.application_id<>g.application_id or c.api_key_id is distinct from g.api_key_id) then
  raise exception 'log convergence: conversation preservation failed';
 end if;
 if exists(select 1 from gateway_log_output_items o where not exists(
 select 1 from application_run_conversation_message_items m where m.flow_run_id=o.flow_run_id
 and m.source_item_key='output:'||o.item_key and m.native_message->'_source_item'=o.item
 and (m.native_message->>'_log_conflicting')::boolean=o.conflicting)) then
  raise exception 'log convergence: output preservation failed';
 end if;
 if exists(select 1 from gateway_log_tool_results r where not exists(
 select 1 from application_run_conversation_message_items m where m.log_conversation_id=r.conversation_id
 and m.source_item_key='result:'||r.call_id and m.native_message->'_source_item'=r.result
 and (m.native_message->>'_log_conflicting')::boolean=r.conflicting)) then
  raise exception 'log convergence: result preservation failed';
 end if;
end $$;

create unique index application_conversations_client_identity on application_conversations(application_id,api_key_id,(coalesce(external_user,'')),client_protocol,client_thread_id) where client_thread_id is not null;
create index application_run_log_task_page on application_run_log_summaries(application_id,log_task_run_id,created_at,flow_run_id);
create index application_run_log_conversation_page on application_run_log_summaries(log_conversation_id,created_at,flow_run_id);
create index flow_runs_log_conversation on flow_runs(application_id,api_key_id,(log_context->>'log_conversation_id')) where log_context is not null;
create index flow_runs_log_task on flow_runs((log_context->>'log_task_run_id')) where log_context is not null;
create unique index application_run_message_source_key on application_run_conversation_message_items(log_conversation_id,source_item_key) where source_item_key is not null and log_conversation_id is not null;

-- Original summary/overview tool metrics count each canonical call once,
-- without counting an existing host callback representation a second time.
update application_run_log_summaries s set tool_callback_count=s.tool_callback_count+(
 select count(*) from application_run_conversation_message_items m
 where m.flow_run_id=s.flow_run_id and m.source_item_key like 'output:tool:%'
 and not exists(select 1 from flow_run_callback_tasks c
   cross join lateral jsonb_array_elements(case when jsonb_typeof(c.request_payload->'tool_calls')='array'
     then c.request_payload->'tool_calls' else '[]'::jsonb end) t(item)
   where c.flow_run_id=s.flow_run_id and coalesce(t.item->>'call_id',t.item->>'id')=m.native_message#>>'{_source_item,call_id}')
) where s.log_conversation_id is not null;

-- No original runs, nodes, events, accounting or audit rows are deleted.
drop table gateway_log_output_items;
drop table gateway_log_tool_results;
drop table gateway_log_invocations;
drop table gateway_log_turns;
drop table gateway_log_conversations;
drop index gateway_log_output_facts;
-- The formal runtime source remains indexed under the original log owner.
create index application_run_message_output_facts on runtime_events(flow_run_id,sequence) where event_type='provider_output_item_done';

alter index gateway_log_attempts rename to application_run_provider_attempts;

-- Add only missing system fields; preserve existing user titles/display metadata.
insert into model_fields (
    id,data_model_id,scope_id,code,title,physical_column_name,field_kind,
    is_system,is_writable,is_required,is_unique,display_options,relation_options,
    sort_order,availability_status
)
select md5(d.id::text || ':' || f.code)::uuid,d.id,d.scope_id,f.code,f.code,f.code,f.kind,
    true,false,f.code='invocation_count',false,'{}'::jsonb,'{}'::jsonb,f.ordinal,'available'
from model_definitions d cross join (values
    ('log_conversation_id','string',40),('log_task_run_id','string',41),
    ('invocation_count','number',42),('parent_run_id','string',43),('caused_by_run_id','string',44)
) f(code,kind,ordinal)
where d.code='application_run_log_summaries' and d.data_source_instance_id is null
    and d.owner_kind='core'
    and not exists(select 1 from model_fields e where e.data_model_id=d.id and e.code=f.code);
