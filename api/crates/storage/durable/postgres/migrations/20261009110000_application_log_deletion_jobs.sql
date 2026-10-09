-- Durable fixed worksets: no conversation/event bodies are copied into task state.
create table application_log_deletion_jobs (
    id uuid primary key,
    application_id uuid not null references applications(id) on delete cascade,
    scope_id uuid not null,
    requested_scope jsonb not null,
    ingested_at_before timestamptz not null,
    status text not null check (status in ('queued','running','succeeded','stopped','failed')),
    total_records bigint not null default 0 check (total_records >= 0),
    deleted_records bigint not null default 0 check (deleted_records >= 0 and deleted_records <= total_records),
    error_code text,
    created_at timestamptz not null default clock_timestamp(),
    updated_at timestamptz not null default clock_timestamp(),
    check (status <> 'succeeded' or deleted_records = total_records)
);
create unique index application_log_deletion_one_active on application_log_deletion_jobs(application_id)
    where status in ('queued','running');
create index application_log_deletion_pending on application_log_deletion_jobs(created_at,id)
    where status in ('queued','running');
create index application_log_deletion_latest on application_log_deletion_jobs(application_id,scope_id,created_at desc,id desc);
create table application_log_deletion_records (
    job_id uuid not null references application_log_deletion_jobs(id) on delete cascade,
    record_id uuid not null references application_run_log_tasks(id) on delete cascade,
    primary key(job_id,record_id)
);
create index application_log_deletion_record_ref on application_log_deletion_records(record_id);
-- Separate cancellation fact permits a stop request during a long batch transaction.
create table application_log_deletion_stop_requests (
    job_id uuid primary key references application_log_deletion_jobs(id) on delete cascade
);
