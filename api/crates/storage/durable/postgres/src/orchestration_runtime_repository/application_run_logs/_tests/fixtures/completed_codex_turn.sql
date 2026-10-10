-- One known observation plus unavailable provider usage reproduces the real
-- task's aggregate shape. No source execution or billing record may be closed.
update flow_runs set status='waiting_callback',finished_at=null
where id='00000000-0000-4000-8000-000000000010';
update application_run_log_summaries set status='waiting_callback',finished_at=null,
    input_tokens=0,output_tokens=0,total_tokens=0,total_cost=null
where flow_run_id='00000000-0000-4000-8000-000000000010';
update application_run_log_summaries set total_cost=null,input_tokens=0,output_tokens=0,total_tokens=0
where flow_run_id='00000000-0000-4000-8000-000000000011';
insert into runtime_usage_ledger(id,flow_run_id,usage_status,input_tokens,output_tokens,total_tokens)
select gen_random_uuid(),'00000000-0000-4000-8000-000000000010','recorded',
    (data->>'input_tokens')::bigint,(data->>'output_tokens')::bigint,(data->>'total_tokens')::bigint from completed_turn_fixture;
insert into runtime_usage_ledger(id,flow_run_id,usage_status)
values(gen_random_uuid(),'00000000-0000-4000-8000-000000000010','unavailable_error'),
(gen_random_uuid(),'00000000-0000-4000-8000-000000000011','unavailable_error');
insert into runtime_cost_ledger(id,flow_run_id,workspace_id,normalized_cost,cost_source,cost_status)
select gen_random_uuid(),'00000000-0000-4000-8000-000000000010',
'00000000-0000-4000-8000-000000000002',(data->>'total_cost')::numeric,'local_token_pricing','rated' from completed_turn_fixture;
insert into application_run_conversation_message_items(id,scope_id,application_id,flow_run_id,display_sequence,
source_kind,role,content,native_message,is_current,can_open_detail,status,started_at,projection_version,output_source)
select gen_random_uuid(),'00000000-0000-4000-8000-000000000002'::uuid,'00000000-0000-4000-8000-000000000003'::uuid,
'00000000-0000-4000-8000-000000000010'::uuid,0,'current_run','user',data->>'user_input','{}'::jsonb,true,true,'waiting_callback',now(),7,null from completed_turn_fixture
union all
select gen_random_uuid(),'00000000-0000-4000-8000-000000000002'::uuid,'00000000-0000-4000-8000-000000000003'::uuid,
'00000000-0000-4000-8000-000000000011'::uuid,1,'current_run','assistant',data->>'final_output',
'{"_source_item":{"type":"message"}}',true,true,'succeeded',now(),7,'provider_output_item' from completed_turn_fixture;
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
create temp table execution_before as select id,status,finished_at from flow_runs;
create temp table usage_before as select * from runtime_usage_ledger;
create temp table costs_before as select * from runtime_cost_ledger;
update application_run_log_tasks set projection_deadline_at=now()-interval '1 second' where status='waiting_callback';
select settle_next_application_log_projection();
do $$ declare actual record; expected jsonb; message record; begin
 select data into expected from completed_turn_fixture;
 select * into actual from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010';
 assert actual.total_tokens=(expected->>'total_tokens')::bigint, 'completed turn retains observed token total';
 assert actual.input_tokens=(expected->>'input_tokens')::bigint and actual.output_tokens=(expected->>'output_tokens')::bigint, 'observed input and output';
 assert actual.total_cost=(expected->>'total_cost')::numeric, 'completed turn retains exact observed cost';
 assert actual.projection_output=expected->>'final_output' and actual.projection_output_source='observed_output', 'real completed report with provenance';
 assert actual.user_input=expected->>'user_input', 'real current user input retained';
 assert actual.status='waiting_callback' and actual.finished_at is null, 'projection does not terminate execution';
 for message in execute 'execute read_task(''00000000-0000-4000-8000-000000000003'',''00000000-0000-4000-8000-000000000010'',7)' loop
  assert message.query=expected->>'user_input' and message.answer=expected->>'final_output', 'business reader retains real user and final';
  assert message.output_source='provider_output_item', 'observed output is not a timeout placeholder';
 end loop;
 assert not settle_next_application_log_projection(), 'same task settles only once';
 assert not exists((select * from execution_before) except (select id,status,finished_at from flow_runs)), 'execution unchanged';
 assert not exists((select * from usage_before) except (select * from runtime_usage_ledger)), 'usage unchanged';
 assert not exists((select * from costs_before) except (select * from runtime_cost_ledger)), 'cost facts unchanged';
end $$;
-- Reapply the migration's repair operation to an already-settled old row.
-- This must repair old NULL snapshots as well as newly due projections.
update application_run_log_tasks set projection_settled_at=projection_settled_at where projection_settled_at is not null;
do $$ begin
 assert (select total_tokens=93825831 and total_cost=32.779366 from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'existing settlement repair is idempotent';
end $$;
-- A literal model reply "Timeout" is still an observed reply, not a placeholder.
update application_run_conversation_message_items set content='Timeout' where role='assistant';
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
do $$ begin
 assert (select projection_output='Timeout' and projection_output_source='observed_output' from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'text value does not determine provenance';
end $$;
-- No observations at all remain unknown, never zero.
delete from runtime_usage_ledger;
delete from runtime_cost_ledger;
update application_run_log_summaries set input_tokens=null,output_tokens=null,total_tokens=null,total_cost=null;
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
do $$ begin
 assert (select total_tokens is null and total_cost is null from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'all missing remains unknown';
end $$;
