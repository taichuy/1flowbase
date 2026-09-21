-- psql regression: run with search_path set to an isolated schema initialized
-- by the forward migrations, with ../cost_snapshot.sql prepared as save_cost(uuid).
-- The fixture is rolled back and never uses public data.
begin;
insert into users(id,account,email,password_hash,name,nickname,status)
values ('00000000-0000-4000-8000-000000000001','cost-test','cost@test.invalid','unused','Cost','Cost','active');
insert into workspaces(id,tenant_id,name)
values ('00000000-0000-4000-8000-000000000002','00000000-0000-0000-0000-000000000001','Cost test');
insert into applications(id,workspace_id,application_type,name,created_by)
values ('00000000-0000-4000-8000-000000000003','00000000-0000-4000-8000-000000000002','agent_flow','Cost test','00000000-0000-4000-8000-000000000001');
insert into flows(id,application_id,scope_id,created_by,updated_by)
values ('00000000-0000-4000-8000-000000000004','00000000-0000-4000-8000-000000000003','00000000-0000-4000-8000-000000000002','00000000-0000-4000-8000-000000000001','00000000-0000-4000-8000-000000000001');
insert into flow_drafts(id,flow_id,scope_id,schema_version,document,created_by,updated_by)
values ('00000000-0000-4000-8000-000000000005','00000000-0000-4000-8000-000000000004','00000000-0000-4000-8000-000000000002','1flowbase.flow/v2','{}','00000000-0000-4000-8000-000000000001','00000000-0000-4000-8000-000000000001');
insert into flow_runs(id,application_id,flow_id,flow_draft_id,scope_id,run_mode,status,created_by,finished_at)
select ('00000000-0000-4000-8000-' || lpad(n::text,12,'0'))::uuid,
 '00000000-0000-4000-8000-000000000003','00000000-0000-4000-8000-000000000004','00000000-0000-4000-8000-000000000005','00000000-0000-4000-8000-000000000002',
 'published_api_run','succeeded','00000000-0000-4000-8000-000000000001',now()
from generate_series(10,14) n;
insert into application_run_log_summaries(flow_run_id,application_id,scope_id,run_mode,status,title,started_at,finished_at,created_at,updated_at,log_task_run_id)
select id,application_id,scope_id,run_mode,status,'Cost test',started_at,finished_at,created_at,updated_at,
 case when id in ('00000000-0000-4000-8000-000000000010','00000000-0000-4000-8000-000000000011') then '00000000-0000-4000-8000-000000000010'::uuid else id end
from flow_runs;
-- A failed provider attempt and its successful retry both count; another call
-- in the same task adds its own cost. Also cover explicit zero and mixed currencies.
insert into runtime_cost_ledger(id,flow_run_id,workspace_id,normalized_cost,settlement_currency,cost_source,cost_status)
select gen_random_uuid(),('00000000-0000-4000-8000-' || lpad(run::text,12,'0'))::uuid,
 '00000000-0000-4000-8000-000000000002',amount,currency,'local_token_pricing','rated'
from (values (10,0.000001250000000001::numeric,'USD'),(10,0.000002::numeric,'USD'),
 (11,0.000003::numeric,'USD'),(12,0::numeric,'USD'),(14,1::numeric,'USD'),(14,2::numeric,'EUR')) v(run,amount,currency);
execute save_cost('00000000-0000-4000-8000-000000000010');
execute save_cost('00000000-0000-4000-8000-000000000011');
execute save_cost('00000000-0000-4000-8000-000000000012');
execute save_cost('00000000-0000-4000-8000-000000000013');
execute save_cost('00000000-0000-4000-8000-000000000014');
select application_run_log_task_refresh(log_task_run_id) from (select distinct log_task_run_id from application_run_log_summaries) anchors;
do $$ begin
 assert (select total_cost::numeric = 0.000006250000000001 and currency_code='USD' from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'sum all retries and task rounds exactly';
 assert (select total_cost='0' and currency_code='USD' from application_run_log_tasks where id='00000000-0000-4000-8000-000000000012'), 'known zero stays zero';
 assert (select total_cost is null and currency_code is null from application_run_log_tasks where id='00000000-0000-4000-8000-000000000013'), 'missing cost is unknown';
 assert (select total_cost is null and currency_code is null from application_run_log_tasks where id='00000000-0000-4000-8000-000000000014'), 'never add different currencies';
end $$;
-- No request-log delivery is needed. Removing the synchronous source after
-- finalization cannot change a recorded task even if its projection refreshes.
delete from model_provider_request_logs;
delete from runtime_cost_ledger;
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
do $$ begin
 assert (select total_cost::numeric = 0.000006250000000001 from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'snapshot survives source cleanup';
end $$;
-- Reopened tasks and tasks with an unknown member must not present a partial
-- amount as a completed total.
update application_run_log_summaries set finished_at=null,status='running' where flow_run_id='00000000-0000-4000-8000-000000000011';
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
do $$ begin
 assert (select total_cost is null from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'active task has no final total';
end $$;
update application_run_log_summaries set finished_at=now(),status='succeeded',total_cost=null,currency_code=null where flow_run_id='00000000-0000-4000-8000-000000000011';
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
do $$ begin
 assert (select total_cost is null from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'unknown member must not be silently omitted';
end $$;
deallocate save_cost;
rollback;
