-- Previously persisted JSON with mixed currencies, without any source ledger.
update application_run_log_summaries set cost_breakdown = '[{"currency_code":"CNY","total_cost":"0.15"},{"currency_code":"USD","total_cost":"0.020000000000000001"}]'
where flow_run_id='00000000-0000-4000-8000-000000000010';
update application_run_log_summaries set cost_breakdown = '[{"currency_code":"EUR","total_cost":"0.33"}]'
where flow_run_id='00000000-0000-4000-8000-000000000011';
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
update model_fields set title='Custom credit title' where code='cost_breakdown'
and data_model_id in (select id from model_definitions where code='application_run_log_tasks');
-- APPLY MIGRATION
do $$ begin
 assert (select total_cost=0.170000000000000001 from application_run_log_summaries where flow_run_id='00000000-0000-4000-8000-000000000010'), 'sum existing JSON amounts regardless of currency';
 assert (select total_cost=0.500000000000000001 from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'sum existing task member snapshots';
 assert (select total_cost is null from application_run_log_summaries where flow_run_id='00000000-0000-4000-8000-000000000013'), 'preserve unknown history';
 assert (select count(*)=1 from model_fields where code='total_cost' and field_kind='number' and title='Custom credit title'), 'preserve custom presentation metadata';
 assert (select sum(total_cost)=0.500000000000000001 from application_run_log_tasks), 'migrated snapshots support numeric SUM';
end $$;
