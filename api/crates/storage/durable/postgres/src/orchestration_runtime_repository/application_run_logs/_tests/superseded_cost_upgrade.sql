-- Historical cancelled predecessor with complete ledger but no snapshot.
update flow_runs set status='cancelled' where id='00000000-0000-4000-8000-000000000010';
update application_run_log_summaries set status='cancelled' where flow_run_id='00000000-0000-4000-8000-000000000010';
update flow_runs set status='waiting_callback',finished_at=null where id='00000000-0000-4000-8000-000000000011';
update application_run_log_summaries set status='waiting_callback',finished_at=null where flow_run_id='00000000-0000-4000-8000-000000000011';
insert into runtime_cost_ledger(id,flow_run_id,workspace_id,normalized_cost,cost_source,cost_status)
select gen_random_uuid(),('00000000-0000-4000-8000-'||lpad(run::text,12,'0'))::uuid,
 '00000000-0000-4000-8000-000000000002',cost,'local_token_pricing','rated'
from (values (10,0.22::numeric),(10,0.03::numeric),(11,0.5::numeric),(12,0::numeric),(14,1::numeric),(14,null::numeric)) v(run,cost);
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
update application_run_log_tasks set projection_deadline_at=now()-interval '1 second' where status='waiting_callback';
select settle_next_application_log_projection();
do $$ begin
 assert (select total_cost is null and projection_settled_at is not null from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'reproduce settled task with missing terminal snapshot';
end $$;
create temp table execution_before as select id,status,finished_at from flow_runs;
create temp table ledger_before as select * from runtime_cost_ledger;
create temp table deadline_before as select projection_deadline_at,projection_settled_at from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010';
-- APPLY MIGRATION
do $$ begin
 assert (select total_cost=0.25 from application_run_log_summaries where flow_run_id='00000000-0000-4000-8000-000000000010'), 'repair cancelled snapshot from complete ledger';
 assert (select total_cost=0.75 from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'refresh already settled task after repair';
 assert (select t.projection_settled_at=b.projection_settled_at and t.projection_deadline_at=b.projection_deadline_at from application_run_log_tasks t,deadline_before b where t.id='00000000-0000-4000-8000-000000000010'), 'repair does not reopen waiting episode';
 assert (select total_cost=0 from application_run_log_summaries where flow_run_id='00000000-0000-4000-8000-000000000012'), 'explicit zero snapshot repaired';
 assert (select total_cost is null from application_run_log_summaries where flow_run_id='00000000-0000-4000-8000-000000000013'), 'absent ledger remains unknown';
 assert (select total_cost is null from application_run_log_summaries where flow_run_id='00000000-0000-4000-8000-000000000014'), 'partial ledger remains unknown';
 assert not exists((select id,status,finished_at from flow_runs) except (table execution_before)), 'workflow unchanged';
 assert not exists((table runtime_cost_ledger) except (table ledger_before)), 'no billing writes';
end $$;
-- Future missing snapshot also falls back at the normal projector boundary.
update application_run_log_summaries set total_cost=null where flow_run_id='00000000-0000-4000-8000-000000000010';
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
do $$ begin
 assert (select total_cost=0.75 from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'future missing snapshot uses ledger';
end $$;
update application_run_log_summaries set total_cost=0.25 where flow_run_id='00000000-0000-4000-8000-000000000010';
delete from runtime_cost_ledger where flow_run_id='00000000-0000-4000-8000-000000000010';
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
do $$ begin
 assert (select total_cost=0.75 from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'known snapshot survives retention and repeated projection';
end $$;
