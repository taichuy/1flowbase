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
-- Integrity remains an event; current semantic occurrences are stored directly.
insert into runtime_events(id,flow_run_id,sequence,event_type,layer,source,trust_level,payload,visibility,durability)
values(gen_random_uuid(),'00000000-0000-4000-8000-000000000011',1,'client_protocol_trajectory','runtime_item','host','host_fact',
 jsonb_build_object('request_id','00000000-0000-4000-8000-000000000090',
 'fact',jsonb_build_object('kind','integrity','status','pending','dropped_count',0,'persist_failed_count',0)),
 'internal','durable');
create function pg_temp.append_client_result(seq bigint, step uuid, category text, origin text, canonical_body text)
returns void language plpgsql as $$
declare
 content uuid := gen_random_uuid();
 body_bytes bytea := convert_to(canonical_body,'UTF8');
 body_digest bytea := sha256(body_bytes);
 observed text := clock_timestamp()::text;
begin
 insert into client_trajectory_steps(id,request_id,flow_run_id,event_sequence,metadata,observed_at,
     semantic_metadata_restored,metadata_compact,metadata_layout)
 values(step,'00000000-0000-4000-8000-000000000090','00000000-0000-4000-8000-000000000011',seq,
     jsonb_build_object('category',category,'origin',origin,'available_sections',jsonb_build_array('result')),
     observed,true,true,1);
 insert into runtime_canonical_contents(id,scope_id,application_id,content_hash,content,byte_size)
 values(content,'00000000-0000-4000-8000-000000000002','00000000-0000-4000-8000-000000000003',
     'sha256:'||encode(body_digest,'hex'),canonical_body::jsonb,octet_length(body_bytes));
 insert into runtime_observation_body_ownership(content_id) values(content);
 insert into client_trajectory_sections(id,request_id,step_id,flow_run_id,section,event_sequence,
     content_id,content_path,body_kind,observed_at,value_digest,value_byte_size)
 values(gen_random_uuid(),'00000000-0000-4000-8000-000000000090',step,'00000000-0000-4000-8000-000000000011',
     'result',seq+1,content,'{}','content',observed,body_digest,octet_length(body_bytes));
end $$;
-- Canonical JSON uses sorted object keys and no insignificant whitespace.
select pg_temp.append_client_result(2,'00000000-0000-4000-8000-000000000091','assistant','emitted',
 '[{"text":"最后","type":"output_text"},{"text":"SECRET","type":"reasoning_text"},{"text":"回复","type":"output_text"}]');
select pg_temp.append_client_result(4,'00000000-0000-4000-8000-000000000092','tool_call','emitted','"exec NO"');
select pg_temp.append_client_result(6,'00000000-0000-4000-8000-000000000093','assistant','submitted','"history NO"');
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
select pg_temp.check_message('最后回复','provider_output_item');
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
update flow_runs set status='waiting_callback' where id='00000000-0000-4000-8000-000000000011';
update application_run_log_summaries set status='waiting_callback',input_tokens=null where flow_run_id='00000000-0000-4000-8000-000000000011';
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
update runtime_cost_ledger set normalized_cost=null;
update application_run_log_tasks set projection_deadline_at=now()-interval '1 second' where status='waiting_callback';
select settle_next_application_log_projection();
do $$ begin
 assert (select total_cost=0.25 and input_tokens=20 from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'settlement retains known member amounts without manufacturing missing values';
end $$;

-- Observed usage with missing fields must not become zero via summary fallback.
insert into runtime_usage_ledger(id,flow_run_id,usage_status)
values(gen_random_uuid(),'00000000-0000-4000-8000-000000000011','unknown');
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
do $$ begin
 assert (select input_tokens=20 and output_tokens=3 and total_tokens=23 from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'unknown usage does not erase known observations from another member';
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
