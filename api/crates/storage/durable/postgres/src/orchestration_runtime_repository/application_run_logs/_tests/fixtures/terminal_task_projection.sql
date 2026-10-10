-- Real failure/retry shape from gateway task 01a12143-3af3-79d1-9065-58c1406f438a:
-- failed (unknown cost), failed (0.2499308), succeeded (0.2792348).
-- Existing base fixture provides runs 10..14 and application/scope/actor only.
update flow_runs set status=case when right(id::text,2) in ('10','11') then 'failed' else 'succeeded' end,
    started_at='2026-10-09T15:23:00Z'::timestamptz+right(id::text,2)::integer*interval '1 minute',
    finished_at='2026-10-09T15:36:44Z'::timestamptz,
    error_payload=case when right(id::text,2)='10' then '{"error_code":"provider_transport_unavailable"}'::jsonb
        when right(id::text,2)='11' then '{"error_code":"provider_upstream_error","code":"websocket_connection_limit_reached"}'::jsonb end;
update application_run_log_summaries s set status=f.status,started_at=f.started_at,finished_at=f.finished_at,
    log_task_run_id=case when right(f.id::text,2) in ('10','11','12') then '00000000-0000-4000-8000-000000000010'::uuid else f.id end,
    total_cost=case right(f.id::text,2) when '11' then 0.2499308 when '12' then 0.2792348 end,
    total_tokens=case right(f.id::text,2) when '10' then 0 when '11' then 728395 when '12' then 716443 end
from flow_runs f where f.id=s.flow_run_id;
insert into application_run_conversation_message_items(id,scope_id,application_id,flow_run_id,display_sequence,
source_kind,role,content,is_current,can_open_detail,status,started_at,projection_version,output_source)
values(gen_random_uuid(),'00000000-0000-4000-8000-000000000002','00000000-0000-4000-8000-000000000003',
'00000000-0000-4000-8000-000000000012',0,'current_run','assistant','Recovered final answer',true,true,'succeeded',now(),7,'persisted_answer');
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
create temp table terminal_source_before as select id,status,error_payload,finished_at from flow_runs;
-- EXPECT OLD
-- Prove that the upgrade fixture starts with the actual historical defect.
do $$ begin
 assert (select status='failed' and total_cost is null and outcome='final_answer_observed'
 from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'pre-upgrade retry task reproduces failed/null snapshot';
end $$;
-- APPLY MIGRATION
-- Both the forward repair and subsequent refresh must retain the successful retry.
do $$ begin
 assert (select status='succeeded' and outcome='final_answer_observed' and total_cost=0.5291656
     and total_tokens=1444838 and final_output='Recovered final answer'
     from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'terminal retry task has final attempt outcome and observed cost';
 assert not exists((select * from terminal_source_before) except (select id,status,error_payload,finished_at from flow_runs)), 'failed attempt evidence is unchanged';
end $$;
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
do $$ begin
 assert (select status='succeeded' and total_cost=0.5291656 from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'repeat refresh does not double count';
end $$;
-- The other real sample has a billed failure and a successful retry.
update application_run_log_summaries set log_task_run_id='00000000-0000-4000-8000-000000000013',
    total_cost=case right(flow_run_id::text,2) when '13' then 0.066804 when '14' then 0.0387024 end,
    status=case right(flow_run_id::text,2) when '13' then 'failed' else 'succeeded' end
where right(flow_run_id::text,2) in ('13','14');
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000013');
do $$ begin
 assert (select status='succeeded' and total_cost=0.1055064 from application_run_log_tasks where id='00000000-0000-4000-8000-000000000013'), 'billed failed attempt plus successful retry';
end $$;
-- Success is not sticky: a later failed/cancelled/incomplete business attempt wins.
do $$ declare terminal text; begin
 foreach terminal in array array['failed','cancelled','incomplete'] loop
  update application_run_log_summaries set status=terminal where flow_run_id='00000000-0000-4000-8000-000000000012';
  perform application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
  assert (select status=terminal from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'latest terminal business attempt wins';
 end loop;
end $$;
-- Successful auxiliary calls cannot hide the failed generation; active calls still win.
update application_run_log_summaries set status='succeeded',call_kind='compact' where flow_run_id='00000000-0000-4000-8000-000000000012';
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
do $$ begin
 assert (select status='failed' from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'compaction success does not mask generation failure';
end $$;
do $$ declare active text; begin
 foreach active in array array['queued','running','waiting_callback','waiting_human','paused'] loop
  update application_run_log_summaries set status=active,finished_at=null where flow_run_id='00000000-0000-4000-8000-000000000010';
  perform application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
  assert (select status=active and outcome='in_progress' and finished_at is null and total_cost is null
   from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'active work cannot be hidden by a later terminal attempt';
 end loop;
end $$;
-- A standalone count-tokens/compact call still reports its own terminal status.
update application_run_log_summaries set call_kind='count_tokens',status='failed' where flow_run_id='00000000-0000-4000-8000-000000000014';
update application_run_log_summaries set log_task_run_id=flow_run_id where flow_run_id='00000000-0000-4000-8000-000000000014';
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000014');
do $$ begin
 assert (select status='failed' from application_run_log_tasks where id='00000000-0000-4000-8000-000000000014'), 'standalone auxiliary task preserves its status';
end $$;
-- No zero is invented when every cost is unknown.
update application_run_log_summaries set status='failed',finished_at=now(),total_cost=null;
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
do $$ begin
 assert (select total_cost is null from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'all unknown terminal costs remain NULL';
end $$;
-- Missing snapshots may still have retained observed ledger amounts.
insert into runtime_cost_ledger(id,flow_run_id,workspace_id,normalized_cost,cost_source,cost_status)
values(gen_random_uuid(),'00000000-0000-4000-8000-000000000011','00000000-0000-4000-8000-000000000002',0.125,'local_token_pricing','rated');
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
do $$ begin
 assert (select total_cost=0.125 from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'missing snapshot uses observed ledger without changing billing';
 assert (select count(*)=1 from runtime_cost_ledger), 'refresh does not create billing facts';
end $$;
