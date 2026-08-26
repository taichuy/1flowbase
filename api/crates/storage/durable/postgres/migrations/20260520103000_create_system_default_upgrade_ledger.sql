create table if not exists system_default_upgrade_runs (
    id uuid primary key,
    system_version text not null,
    status text not null,
    requested_by uuid references users(id) on delete set null,
    summary_json jsonb not null default '{}'::jsonb,
    error_message text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint system_default_upgrade_runs_system_version_check
        check (system_version <> ''),
    constraint system_default_upgrade_runs_status_check
        check (status in ('pending', 'running', 'succeeded', 'failed', 'partially_applied')),
    constraint system_default_upgrade_runs_summary_json_check
        check (jsonb_typeof(summary_json) = 'object'),
    constraint system_default_upgrade_runs_updated_at_check
        check (updated_at >= created_at)
);

create table if not exists system_default_upgrade_items (
    id uuid primary key,
    run_id uuid not null references system_default_upgrade_runs(id) on delete cascade,
    default_key text not null,
    target_kind text not null,
    target_id uuid not null,
    status text not null,
    skip_reason text,
    before_hash text,
    after_hash text,
    patch_json jsonb not null default '{}'::jsonb,
    error_message text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint system_default_upgrade_items_default_key_check
        check (default_key <> ''),
    constraint system_default_upgrade_items_target_kind_check
        check (target_kind <> ''),
    constraint system_default_upgrade_items_status_check
        check (status in ('pending', 'applied', 'skipped', 'failed')),
    constraint system_default_upgrade_items_before_hash_check
        check (before_hash is null or before_hash <> ''),
    constraint system_default_upgrade_items_after_hash_check
        check (after_hash is null or after_hash <> ''),
    constraint system_default_upgrade_items_patch_json_check
        check (jsonb_typeof(patch_json) = 'object'),
    constraint system_default_upgrade_items_updated_at_check
        check (updated_at >= created_at),
    constraint system_default_upgrade_items_run_target_unique
        unique (run_id, default_key, target_kind, target_id)
);

create index if not exists system_default_upgrade_runs_version_created_idx
    on system_default_upgrade_runs (system_version, created_at desc, id desc);

create index if not exists system_default_upgrade_runs_status_updated_idx
    on system_default_upgrade_runs (status, updated_at desc, id desc);

create index if not exists system_default_upgrade_items_run_status_idx
    on system_default_upgrade_items (run_id, status, updated_at desc, id desc);

create index if not exists system_default_upgrade_items_target_idx
    on system_default_upgrade_items (target_kind, target_id, created_at desc, id desc);
