create table frontstage_executable_upgrade_markers (
    marker text primary key,
    target_identity jsonb not null,
    status text not null,
    current_run_id uuid,
    completed_at timestamptz,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint frontstage_executable_upgrade_marker_not_empty check (btrim(marker) <> ''),
    constraint frontstage_executable_upgrade_marker_status_check
        check (status in ('running', 'failed', 'completed')),
    constraint frontstage_executable_upgrade_marker_completion_check check (
        (status = 'completed' and completed_at is not null)
        or (status <> 'completed' and completed_at is null)
    )
);

create table frontstage_executable_upgrade_runs (
    run_id uuid primary key,
    marker text not null references frontstage_executable_upgrade_markers(marker) on delete restrict,
    attempt integer not null,
    target_identity jsonb not null,
    status text not null,
    source_snapshot jsonb,
    source_snapshot_sha256 text,
    error_code text,
    failure_target_identity jsonb,
    compiler_identity jsonb not null,
    started_at timestamptz not null default now(),
    failed_at timestamptz,
    completed_at timestamptz,
    updated_at timestamptz not null default now(),
    constraint frontstage_executable_upgrade_run_attempt_unique unique (marker, attempt),
    constraint frontstage_executable_upgrade_run_attempt_positive check (attempt > 0),
    constraint frontstage_executable_upgrade_run_status_check
        check (status in ('running', 'failed', 'completed')),
    constraint frontstage_executable_upgrade_run_snapshot_check check (
        (source_snapshot is null and source_snapshot_sha256 is null)
        or (jsonb_typeof(source_snapshot) = 'array'
            and source_snapshot_sha256 ~ '^[0-9a-f]{64}$')
    ),
    constraint frontstage_executable_upgrade_run_failure_check check (
        (status = 'failed'
            and nullif(btrim(error_code), '') is not null
            and jsonb_typeof(failure_target_identity) = 'object'
            and failed_at is not null
            and completed_at is null)
        or (status = 'running'
            and error_code is null
            and failure_target_identity is null
            and failed_at is null
            and completed_at is null)
        or (status = 'completed'
            and error_code is null
            and failure_target_identity is null
            and failed_at is null
            and completed_at is not null)
    )
);

alter table frontstage_executable_upgrade_markers
    add constraint frontstage_executable_upgrade_marker_current_run_fkey
    foreign key (current_run_id)
    references frontstage_executable_upgrade_runs(run_id)
    deferrable initially deferred;

create index frontstage_executable_upgrade_runs_marker_started_idx
    on frontstage_executable_upgrade_runs (marker, started_at desc, run_id desc);
