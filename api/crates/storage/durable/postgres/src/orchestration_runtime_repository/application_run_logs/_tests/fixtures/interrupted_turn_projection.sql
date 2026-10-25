insert into application_conversations(id,scope_id,application_id,external_conversation_id)
values ('00000000-0000-4000-8000-000000000900','00000000-0000-4000-8000-000000000002',
    '00000000-0000-4000-8000-000000000003','fixture-thread');
-- Sanitized shape of the observed Codex turn: 12+1 and 3+1 callback rounds,
-- two old pending waits, then one successful generation in the same turn.
update flow_runs set status=case when right(id::text,2) in ('10','11') then 'waiting_callback' else 'succeeded' end,
    started_at='2026-10-25T03:00:00Z'::timestamptz+right(id::text,2)::integer*interval '1 minute',
    finished_at=case when right(id::text,2)='12' then '2026-10-25T03:13:00Z'::timestamptz end,
    log_context=jsonb_build_object('identity_status','identified','thread_id','fixture-thread',
        'turn_id','fixture-turn','protocol','openai_responses','log_task_run_id','00000000-0000-4000-8000-000000000010',
        'prompt',jsonb_build_object('type','message','content_omitted',true)),
    input_payload=jsonb_build_object('node-start',jsonb_build_object('query',E'  Current image question\nsecond line  '),
        'history',jsonb_build_array(jsonb_build_object('role','user','content','OLD HISTORY IS NOT INPUT')))
where right(id::text,2) in ('10','11','12');
update application_run_log_summaries s set status=f.status,started_at=f.started_at,finished_at=f.finished_at,
    log_conversation_id='00000000-0000-4000-8000-000000000900',call_kind='generate',
    log_task_run_id='00000000-0000-4000-8000-000000000010',total_cost=null
from flow_runs f where s.flow_run_id=f.id and right(f.id::text,2) in ('10','11','12');
insert into node_runs(id,flow_run_id,node_id,node_type,node_alias,status,scope_id)
select ('00000000-0000-4000-8000-'||lpad((n+1000)::text,12,'0'))::uuid,
    ('00000000-0000-4000-8000-'||lpad(n::text,12,'0'))::uuid,
    'fixture-llm','llm','Fixture LLM','waiting_callback','00000000-0000-4000-8000-000000000002'
from generate_series(10,11) n;
insert into flow_run_callback_tasks(id,flow_run_id,node_run_id,callback_kind,status,created_at,completed_at,scope_id)
select ('00000000-0000-4000-8000-'||lpad((w*100+n)::text,12,'0'))::uuid,
    ('00000000-0000-4000-8000-'||lpad(w::text,12,'0'))::uuid,
    ('00000000-0000-4000-8000-'||lpad((w+1000)::text,12,'0'))::uuid,
    'llm_tool',case when n=rounds then 'pending' else 'completed' end,
    '2026-10-25T03:10:00Z'::timestamptz+(w-10)*interval '1 minute'+n*interval '1 second',
    case when n<rounds then '2026-10-25T03:10:00Z'::timestamptz+(w-10)*interval '1 minute'+n*interval '1 second'+interval '100 milliseconds' end,
    '00000000-0000-4000-8000-000000000002'
from (values (10,13),(11,4)) waits(w,rounds) cross join lateral generate_series(1,rounds) n;
insert into flow_run_tool_callback_inbox(id,scope_id,application_id,flow_run_id,callback_task_id,tool_call_id,
    tool_ordinal,status,result_payload,result_fingerprint,created_at,received_at,updated_at)
select gen_random_uuid(),c.scope_id,'00000000-0000-4000-8000-000000000003',c.flow_run_id,c.id,c.id::text,0,
    case when c.status='completed' then 'received' else 'pending' end,
    case when c.status='completed' then '{}'::jsonb end,
    case when c.status='completed' then 'fixture-digest' end,c.created_at,c.completed_at,
    coalesce(c.completed_at,c.created_at)
from flow_run_callback_tasks c;
insert into flow_run_callback_resume_attempts(id,scope_id,flow_run_id,callback_task_id,source,status,
    idempotency_key,created_at,updated_at,completed_at)
select gen_random_uuid(),scope_id,flow_run_id,id,'fixture','succeeded',id::text,created_at,completed_at,completed_at
from flow_run_callback_tasks where status='completed';
insert into flow_run_checkpoints(id,flow_run_id,node_run_id,status,reason,locator_payload,variable_snapshot,scope_id)
select gen_random_uuid(),flow_run_id,id,'waiting','callback','{}','{}',scope_id from node_runs;
insert into flow_run_resume_claims(id,scope_id,application_id,flow_run_id,checkpoint_id,callback_task_id,resume_kind,
    status,request_payload,claim_token,lease_expires_at,created_at,updated_at,completed_at)
select gen_random_uuid(),c.scope_id,'00000000-0000-4000-8000-000000000003',c.flow_run_id,p.id,c.id,'callback',
    'succeeded','{}',gen_random_uuid(),c.completed_at,c.created_at,c.completed_at,c.completed_at
from flow_run_callback_tasks c join flow_run_checkpoints p on p.flow_run_id=c.flow_run_id where c.status='completed';
insert into application_run_conversation_message_items(id,scope_id,application_id,flow_run_id,display_sequence,
    source_kind,role,content,is_current,can_open_detail,status,started_at,projection_version,output_source,native_message)
values
(gen_random_uuid(),'00000000-0000-4000-8000-000000000002','00000000-0000-4000-8000-000000000003',
 '00000000-0000-4000-8000-000000000010',0,'current_run','user','',true,true,'waiting_callback',now(),7,'none','{"role":"user","content":""}'),
(gen_random_uuid(),'00000000-0000-4000-8000-000000000002','00000000-0000-4000-8000-000000000003',
 '00000000-0000-4000-8000-000000000012',0,'current_run','assistant','Observed final answer',true,true,'succeeded',now(),7,'persisted_answer','{"role":"assistant","content":"Observed final answer"}');
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
create temp table interrupted_source_before as select jsonb_build_object(
    'runs',(select jsonb_agg(to_jsonb(f) order by id) from flow_runs f),
    'callbacks',(select jsonb_agg(to_jsonb(c) order by id) from flow_run_callback_tasks c),
    'inbox',(select jsonb_agg(to_jsonb(i) order by id) from flow_run_tool_callback_inbox i),
    'resumes',(select jsonb_agg(to_jsonb(r) order by id) from flow_run_callback_resume_attempts r),
    'claims',(select jsonb_agg(to_jsonb(r) order by id) from flow_run_resume_claims r),
    'usage',(select jsonb_agg(to_jsonb(l) order by id) from runtime_usage_ledger l),
    'cost',(select jsonb_agg(to_jsonb(l) order by id) from runtime_cost_ledger l)) as state;
-- EXPECT OLD
-- The actual pre-upgrade function must exhibit both reported defects.
do $$ begin
 assert (select status='waiting_callback' and outcome='in_progress' and user_input=''
     and final_output='Observed final answer' from application_run_log_tasks
     where id='00000000-0000-4000-8000-000000000010'), 'old interrupted turn reproduces wait and missing input';
end $$;
-- APPLY MIGRATION
do $$ begin
 assert (select status='succeeded' and outcome='final_answer_observed'
     and user_input=E'  Current image question\nsecond line  ' and final_output='Observed final answer'
     and finished_at='2026-10-25T03:13:00Z' and projection_settled_at is null
     and cardinality(member_run_ids)=3
     from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'completed explicit turn bypasses compensation without losing old members';
 assert (select state=jsonb_build_object(
    'runs',(select jsonb_agg(to_jsonb(f) order by id) from flow_runs f),
    'callbacks',(select jsonb_agg(to_jsonb(c) order by id) from flow_run_callback_tasks c),
    'inbox',(select jsonb_agg(to_jsonb(i) order by id) from flow_run_tool_callback_inbox i),
    'resumes',(select jsonb_agg(to_jsonb(r) order by id) from flow_run_callback_resume_attempts r),
    'claims',(select jsonb_agg(to_jsonb(r) order by id) from flow_run_resume_claims r),
    'usage',(select jsonb_agg(to_jsonb(l) order by id) from runtime_usage_ledger l),
    'cost',(select jsonb_agg(to_jsonb(l) order by id) from runtime_cost_ledger l)) from interrupted_source_before), 'source execution callbacks and billing are byte-for-byte unchanged';
end $$;
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
-- Projection/usage refresh timestamps are not new execution progress.
update application_run_log_summaries set updated_at=now(),total_cost=0.125
where flow_run_id='00000000-0000-4000-8000-000000000010';
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
do $$ begin
 assert (select status='succeeded' and total_cost=0.125 from application_run_log_tasks
     where id='00000000-0000-4000-8000-000000000010'), 'projection clocks do not reopen or double-count';
end $$;
-- Unknown or unrelated identities cannot borrow this turn's final answer.
do $$ declare changed jsonb; begin
 foreach changed in array array[
     '{"identity_status":"missing_identity"}'::jsonb,
     '{"identity_status":"conflicting_identity"}'::jsonb,
     '{"thread_id":"another-thread"}'::jsonb,
     '{"turn_id":"another-turn"}'::jsonb,
     '{"protocol":"another-protocol"}'::jsonb] loop
  update flow_runs set log_context=log_context||changed where id='00000000-0000-4000-8000-000000000010';
  perform application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
  assert (select status='waiting_callback' and outcome='in_progress' from application_run_log_tasks
      where id='00000000-0000-4000-8000-000000000010'), 'uncertain identity remains live';
  update flow_runs set log_context=log_context||'{"identity_status":"identified","thread_id":"fixture-thread","turn_id":"fixture-turn","protocol":"openai_responses"}'::jsonb where id='00000000-0000-4000-8000-000000000010';
 end loop;
end $$;
-- Other active work always wins, even after the final was observed.
do $$ declare active text; begin
 foreach active in array array['queued','running','waiting_human','paused'] loop
  update application_run_log_summaries set status=active where flow_run_id='00000000-0000-4000-8000-000000000010';
  perform application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
  assert (select status=active and outcome='in_progress' and finished_at is null from application_run_log_tasks
      where id='00000000-0000-4000-8000-000000000010'), 'active work is not hidden';
 end loop;
end $$;
update application_run_log_summaries set status='waiting_callback' where flow_run_id='00000000-0000-4000-8000-000000000010';
-- A successful count/compact call does not close a generation wait.
update application_run_log_summaries set call_kind='compact' where flow_run_id='00000000-0000-4000-8000-000000000012';
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
do $$ begin
 assert (select status='waiting_callback' from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'auxiliary final cannot dominate generation';
end $$;
update application_run_log_summaries set call_kind='generate' where flow_run_id='00000000-0000-4000-8000-000000000012';
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
-- A late tool result reopens via the production trigger, without manual refresh.
update flow_run_tool_callback_inbox set status='received',result_payload='{}',result_fingerprint='late',
    received_at='2026-10-25T03:14:00Z',updated_at='2026-10-25T03:14:00Z'
where callback_task_id='00000000-0000-4000-8000-000000001013';
do $$ begin
 assert (select status='waiting_callback' and outcome='in_progress' and finished_at is null
     from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'late receipt trigger reopens logical task';
 assert (select status='waiting_callback' from flow_runs where id='00000000-0000-4000-8000-000000000010'), 'log trigger never rewrites execution';
end $$;
update flow_run_tool_callback_inbox set status='pending',result_payload=null,result_fingerprint=null,
    received_at=null,updated_at=created_at where callback_task_id='00000000-0000-4000-8000-000000001013';
-- Even unchanged pending status with a new callback time is real progress.
update flow_run_callback_tasks set created_at='2026-10-25T03:14:00Z' where id='00000000-0000-4000-8000-000000001013';
do $$ begin
 assert (select status='waiting_callback' from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'new callback wait reopens without a manual refresh';
end $$;
update flow_run_callback_tasks set created_at='2026-10-25T03:10:13Z' where id='00000000-0000-4000-8000-000000001013';
-- Late resume/claim activity in an earlier round is also not a projection clock.
update flow_run_callback_resume_attempts set updated_at='2026-10-25T03:14:00Z' where callback_task_id='00000000-0000-4000-8000-000000001001';
do $$ begin
 assert (select status='waiting_callback' from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'resume update trigger reopens';
end $$;
update flow_run_callback_resume_attempts set updated_at=completed_at where callback_task_id='00000000-0000-4000-8000-000000001001';
update flow_run_resume_claims set updated_at='2026-10-25T03:14:00Z' where callback_task_id='00000000-0000-4000-8000-000000001001';
do $$ begin
 assert (select status='waiting_callback' from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'claim update trigger reopens';
end $$;
update flow_run_resume_claims set updated_at=completed_at where callback_task_id='00000000-0000-4000-8000-000000001001';
do $$ begin
 assert (select status='succeeded' and outcome='final_answer_observed' and total_cost=0.125 from application_run_log_tasks
     where id='00000000-0000-4000-8000-000000000010'), 'all original facts restore the same deterministic projection';
end $$;
