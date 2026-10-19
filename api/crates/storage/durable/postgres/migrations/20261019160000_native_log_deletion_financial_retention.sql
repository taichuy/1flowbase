-- Financial history keeps the original run identity even after its logs expire.
-- Deliberately do not SET NULL: billing audit and idempotency retain attribution.
alter table runtime_cost_ledger drop constraint runtime_cost_ledger_flow_run_id_fkey;
alter table runtime_credit_ledger drop constraint runtime_credit_ledger_flow_run_id_fkey;
alter table billing_sessions drop constraint billing_sessions_flow_run_id_fkey;
alter table runtime_usage_ledger drop constraint runtime_usage_ledger_flow_run_id_fkey;
alter table runtime_usage_ledger drop constraint runtime_usage_ledger_node_run_id_fkey;
alter table runtime_usage_ledger add constraint runtime_usage_ledger_node_run_id_fkey
    foreign key(node_run_id) references node_runs(id) on delete set null;

-- Imported records have no runs. Native jobs freeze the complete execution set,
-- so a later call joining an existing task cannot silently expand a deletion.
alter table application_log_deletion_records
    add column native_run_ids uuid[] not null default '{}';

create index flow_runs_log_task_members on flow_runs
    (application_id, (log_context->>'log_task_run_id'))
    where log_context->>'log_task_run_id' is not null;
