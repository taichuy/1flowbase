create table plugin_installations (
    id uuid primary key,
    provider_code text not null,
    plugin_id text not null unique,
    plugin_version text not null,
    contract_version text not null,
    protocol text not null,
    display_name text not null,
    source_kind text not null,
    verification_status text not null check (
        verification_status in ('pending', 'valid', 'invalid')
    ),
    enabled boolean not null default false,
    install_path text not null,
    checksum text,
    signature_status text,
    metadata_json jsonb not null default '{}'::jsonb,
    created_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index plugin_installations_provider_code_idx
    on plugin_installations (provider_code, updated_at desc, id desc);

create table plugin_assignments (
    id uuid primary key,
    installation_id uuid not null references plugin_installations(id) on delete cascade,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    assigned_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    unique (installation_id, workspace_id)
);

create index plugin_assignments_workspace_idx
    on plugin_assignments (workspace_id, created_at desc, id desc);

create table plugin_tasks (
    id uuid primary key,
    installation_id uuid references plugin_installations(id) on delete set null,
    workspace_id uuid references workspaces(id) on delete set null,
    provider_code text not null,
    task_kind text not null check (
        task_kind in ('install', 'upgrade', 'uninstall', 'enable', 'disable', 'assign', 'unassign')
    ),
    status text not null check (
        status in ('pending', 'running', 'success', 'failed', 'canceled', 'timed_out')
    ),
    status_message text,
    detail_json jsonb not null default '{}'::jsonb,
    created_by uuid references users(id) on delete set null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    finished_at timestamptz
);

create index plugin_tasks_provider_status_idx
    on plugin_tasks (provider_code, status, created_at desc, id desc);
