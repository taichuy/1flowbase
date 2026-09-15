create index application_run_log_tasks_scope_created_id
    on application_run_log_tasks (scope_id, created_at desc, id desc);
