create table run_archive_upload_sessions (
    id uuid primary key,
    scope_id uuid not null references workspaces(id) on delete cascade,
    application_id uuid not null references applications(id) on delete cascade,
    actor_user_id uuid not null references users(id),
    original_filename text,
    total_size_bytes bigint not null check (total_size_bytes > 0 and total_size_bytes <= 104857600),
    received_bytes bigint not null default 0 check (received_bytes >= 0),
    expected_sha256 text not null,
    chunk_size_bytes bigint not null check (chunk_size_bytes > 0 and chunk_size_bytes <= 8388608),
    status text not null check (status in ('uploading', 'completed', 'failed')),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    completed_at timestamptz
);

create index run_archive_upload_sessions_scope_application_created_idx
    on run_archive_upload_sessions (scope_id, application_id, created_at desc, id desc);

create table run_archive_upload_chunks (
    session_id uuid not null references run_archive_upload_sessions(id) on delete cascade,
    chunk_index integer not null check (chunk_index >= 0),
    chunk_size_bytes bigint not null check (chunk_size_bytes > 0),
    chunk_sha256 text not null,
    content bytea not null,
    created_at timestamptz not null default now(),
    primary key (session_id, chunk_index)
);

create table run_archive_import_jobs (
    id uuid primary key,
    scope_id uuid not null references workspaces(id) on delete cascade,
    application_id uuid not null references applications(id) on delete cascade,
    actor_user_id uuid not null references users(id),
    upload_session_id uuid not null references run_archive_upload_sessions(id) on delete restrict,
    status text not null check (status in ('queued', 'processing', 'succeeded', 'failed')),
    archive_version integer,
    archive_sha256 text,
    run_count integer not null default 0 check (run_count >= 0),
    imported_run_count integer not null default 0 check (imported_run_count >= 0),
    error_payload jsonb,
    result_payload jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    started_at timestamptz,
    finished_at timestamptz
);

create index run_archive_import_jobs_scope_application_created_idx
    on run_archive_import_jobs (scope_id, application_id, created_at desc, id desc);

create table run_archive_import_mappings (
    job_id uuid not null references run_archive_import_jobs(id) on delete cascade,
    entity_kind text not null,
    source_id text not null,
    target_id uuid not null,
    source_locator text,
    created_at timestamptz not null default now(),
    primary key (job_id, entity_kind, source_id)
);

create index run_archive_import_mappings_target_idx
    on run_archive_import_mappings (entity_kind, target_id);

alter table flow_runs
    add column if not exists import_job_id uuid references run_archive_import_jobs(id) on delete set null,
    add column if not exists import_source_run_id text;

create index if not exists flow_runs_import_job_idx
    on flow_runs (import_job_id)
    where import_job_id is not null;
