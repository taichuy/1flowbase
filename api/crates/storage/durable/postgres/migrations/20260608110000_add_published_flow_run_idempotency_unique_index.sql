create unique index if not exists flow_runs_published_idempotency_unique_idx
    on flow_runs (application_id, api_key_id, idempotency_key)
    where run_mode = 'published_api_run'
      and api_key_id is not null
      and idempotency_key is not null;
