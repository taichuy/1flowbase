-- Existing fixture: task 10 owns runs 10/11; all assertions stay in its transaction.
update flow_runs set status='waiting_callback',finished_at=null where id='00000000-0000-4000-8000-000000000011';
update application_run_log_summaries set status='waiting_callback',finished_at=null,
    input_tokens=10,output_tokens=2,total_tokens=12
where flow_run_id='00000000-0000-4000-8000-000000000011';
update application_run_log_summaries set input_tokens=20,output_tokens=3,total_tokens=23,total_cost=0.25
where flow_run_id='00000000-0000-4000-8000-000000000010';
insert into runtime_cost_ledger(id,flow_run_id,workspace_id,normalized_cost,cost_source,cost_status)
values (gen_random_uuid(),'00000000-0000-4000-8000-000000000011','00000000-0000-4000-8000-000000000002',0.125,'local_token_pricing','rated');
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
create temp table original_execution as select id,status,finished_at from flow_runs;
create temp table original_deadline as select projection_deadline_at from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010';
do $$ begin
 assert (select projection_deadline_at between clock_timestamp()+interval '299 seconds' and clock_timestamp()+interval '301 seconds' from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'five minute deadline';
 assert not settle_next_application_log_projection(), 'not due before five minutes';
end $$;
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
do $$ begin
 assert (select t.projection_deadline_at=o.projection_deadline_at from application_run_log_tasks t,original_deadline o where t.id='00000000-0000-4000-8000-000000000010'), 'repeat projection cannot extend deadline';
end $$;
-- Capture the production query through a prepared statement into a temp table.
create temp table observed_message(answer text,output_source text);
create function pg_temp.check_message(expected text, source text) returns void language plpgsql as $$
declare item record; seen integer := 0;
begin
 for item in execute 'execute read_task(''00000000-0000-4000-8000-000000000003'',''00000000-0000-4000-8000-000000000010'',5)' loop
  seen := seen+1;
  assert item.answer is not distinct from expected, 'projected answer: '||coalesce(item.answer,'NULL');
  assert item.output_source=source, 'output provenance';
  assert item.status='waiting_callback' and item.finished_at is null, 'read keeps workflow state';
  assert item.can_open_detail and item.detail_run_id='00000000-0000-4000-8000-000000000010'::uuid, 'placeholder retains log entry';
 end loop;
 assert seen=1, 'one existing task';
end $$;
select pg_temp.check_message('waiting_callback','waiting_callback');
update application_run_log_tasks set projection_deadline_at=now()-interval '1 second' where status='waiting_callback';
do $$ begin
 assert settle_next_application_log_projection(), 'due snapshot is settled';
 assert not settle_next_application_log_projection(), 'idempotent settlement';
 assert (select total_cost=0.375 and input_tokens=30 and output_tokens=5 and total_tokens=35
     and projection_settled_at is not null and outcome='in_progress'
     from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'exact cost and usage without workflow completion';
 assert not exists ((select id,status,finished_at from flow_runs) except (select * from original_execution)), 'execution untouched';
 assert (select count(*)=1 from runtime_cost_ledger), 'no duplicate billing facts';
end $$;
select pg_temp.check_message('Timeout','projection_timeout');
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
select pg_temp.check_message('Timeout','projection_timeout');
-- Real protocol facts: only emitted assistant text, in original block order.
create function pg_temp.append_fact(seq bigint, fact jsonb) returns void language sql as $$
 insert into runtime_events(id,flow_run_id,sequence,event_type,layer,source,trust_level,payload,visibility,durability)
 values(gen_random_uuid(),'00000000-0000-4000-8000-000000000011',seq,'client_protocol_trajectory','runtime_item','host','host_fact',
 jsonb_build_object('request_id','00000000-0000-4000-8000-000000000090','fact',fact),'internal','durable')
$$;
select pg_temp.append_fact(1,'{"kind":"integrity","status":"pending","dropped_count":0,"persist_failed_count":0}');
select pg_temp.append_fact(2,'{"kind":"step","step":{"id":"00000000-0000-4000-8000-000000000091","category":"assistant","origin":"emitted"}}');
select pg_temp.append_fact(3,'{"kind":"section","step_id":"00000000-0000-4000-8000-000000000091","section":"result","value":[{"type":"output_text","text":"最后"},{"type":"reasoning_text","text":"SECRET"},{"type":"output_text","text":"回复"}]}');
select pg_temp.append_fact(4,'{"kind":"step","step":{"id":"00000000-0000-4000-8000-000000000092","category":"tool_call","origin":"emitted"}}');
select pg_temp.append_fact(5,'{"kind":"section","step_id":"00000000-0000-4000-8000-000000000092","section":"result","value":"exec NO"}');
select pg_temp.append_fact(6,'{"kind":"step","step":{"id":"00000000-0000-4000-8000-000000000093","category":"assistant","origin":"submitted"}}');
select pg_temp.append_fact(7,'{"kind":"section","step_id":"00000000-0000-4000-8000-000000000093","section":"result","value":"history NO"}');
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
select pg_temp.check_message('最后回复','projection_timeout');
-- Usage corrections update the existing snapshot instead of adding old totals.
update runtime_cost_ledger set normalized_cost=0.5;
update application_run_log_summaries set input_tokens=12,output_tokens=4,total_tokens=16 where flow_run_id='00000000-0000-4000-8000-000000000011';
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
do $$ begin
 assert (select total_cost=0.75 and total_tokens=39 and projection_settled_at is not null from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'late usage replaces snapshot';
end $$;
-- Real callback progress reopens the projection and invalidates the old deadline.
update flow_runs set status='running' where id='00000000-0000-4000-8000-000000000011';
update application_run_log_summaries set status='running' where flow_run_id='00000000-0000-4000-8000-000000000011';
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
do $$ begin
 assert (select projection_settled_at is null and projection_deadline_at is null and projection_output is null from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'late callback clears timeout projection';
 assert not settle_next_application_log_projection(), 'old timer cannot settle resumed task';
end $$;
update application_run_log_summaries set status='waiting_callback',input_tokens=null where flow_run_id='00000000-0000-4000-8000-000000000011';
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
update runtime_cost_ledger set normalized_cost=null;
update application_run_log_tasks set projection_deadline_at=now()-interval '1 second' where status='waiting_callback';
select settle_next_application_log_projection();
do $$ begin
 assert (select total_cost is null and input_tokens is null from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'missing values stay unknown';
end $$;

-- Observed usage with missing fields must not become zero via summary fallback.
insert into runtime_usage_ledger(id,flow_run_id,usage_status)
values(gen_random_uuid(),'00000000-0000-4000-8000-000000000011','unknown');
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
do $$ begin
 assert (select input_tokens is null and output_tokens is null and total_tokens is null from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'unknown observed usage remains unknown';
end $$;
update runtime_usage_ledger set input_tokens=0,output_tokens=0,total_tokens=0;
update runtime_cost_ledger set normalized_cost=0;
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
do $$ begin
 assert (select input_tokens=20 and output_tokens=3 and total_tokens=23 and total_cost=0.25 from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'explicit zero usage and cost retained';
end $$;
delete from runtime_usage_ledger;

-- A late final callback updates the same task; it is not a new conversation row.
update flow_runs set status='succeeded',finished_at=now() where id='00000000-0000-4000-8000-000000000011';
insert into application_run_conversation_message_items(id,scope_id,application_id,flow_run_id,display_sequence,source_kind,
 answer,can_open_detail,is_current,status,started_at,projection_version,output_source)
values(gen_random_uuid(),'00000000-0000-4000-8000-000000000002','00000000-0000-4000-8000-000000000003',
 '00000000-0000-4000-8000-000000000011',0,'current_run','完成后的真实回复',true,true,'succeeded',now(),5,'persisted_answer');
update application_run_log_summaries set status='succeeded',finished_at=now(),total_cost=0.75,input_tokens=15,output_tokens=5,total_tokens=20
where flow_run_id='00000000-0000-4000-8000-000000000011';
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
do $$ begin
 assert (select final_output='完成后的真实回复' and projection_settled_at is null and projection_output is null
     and outcome='final_answer_observed' and total_cost=1 and total_tokens=43
     from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'late final callback replaces timeout and totals';
 assert (select count(*)=1 from application_run_log_tasks where '00000000-0000-4000-8000-000000000011'::uuid=any(member_run_ids)), 'one task throughout';
end $$;
