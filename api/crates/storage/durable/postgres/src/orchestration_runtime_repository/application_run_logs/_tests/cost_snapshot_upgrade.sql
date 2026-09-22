-- Seed existing single-currency snapshots, including a mixed-currency task.
update application_run_log_summaries set total_cost = '0.020000000000000001', currency_code = 'USD'
where flow_run_id = '00000000-0000-4000-8000-000000000010';
update application_run_log_summaries set total_cost = '0.15', currency_code = 'CNY'
where flow_run_id = '00000000-0000-4000-8000-000000000011';
select application_run_log_task_refresh('00000000-0000-4000-8000-000000000010');
update model_fields set title='Custom cost title'
where code='total_cost' and data_model_id in (select id from model_definitions where code='application_run_log_tasks');
-- APPLY MIGRATION
-- The old sources are empty: upgrading must use the saved snapshots only.
do $$ begin
 assert (select count(*)=0 from runtime_cost_ledger), 'upgrade fixture has no billing source';
 assert (select cost_breakdown = '[{"currency_code":"USD","total_cost":"0.020000000000000001"}]'::jsonb
         from application_run_log_summaries where flow_run_id='00000000-0000-4000-8000-000000000010'), 'preserve old single-currency snapshot';
 assert (select cost_breakdown = '[{"currency_code":"CNY","total_cost":"0.15"},{"currency_code":"USD","total_cost":"0.020000000000000001"}]'::jsonb
         from application_run_log_tasks where id='00000000-0000-4000-8000-000000000010'), 'upgrade groups existing task members by currency';
 assert (select cost_breakdown is null from application_run_log_summaries where flow_run_id='00000000-0000-4000-8000-000000000013'), 'unknown history remains unknown';
 assert (select count(*)=1 from model_fields where code='cost_breakdown' and field_kind='json' and title='Custom cost title'), 'preserve user presentation metadata';
end $$;
