create table applications (
    id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    application_type text not null check (application_type in ('agent_flow', 'workflow')),
    name text not null,
    description text not null default '',
    icon_type text null,
    icon text null,
    icon_background text null,
    created_by uuid not null references users(id),
    updated_by uuid null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index applications_workspace_updated_idx
    on applications (workspace_id, updated_at desc, id desc);

create index applications_workspace_creator_idx
    on applications (workspace_id, created_by, updated_at desc, id desc);
