create unique index if not exists flow_runs_workflow_schedule_idempotency_unique_idx
    on flow_runs (application_id, idempotency_key)
    where run_mode = 'workflow_schedule_run'
      and idempotency_key is not null;
