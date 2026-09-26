create table portable_template_identities (
    workspace_id uuid not null references workspaces(id) on delete cascade,
    kind text not null,
    source_id text not null,
    target_id text not null,
    created_at timestamptz not null default now(),
    primary key (workspace_id, source_id)
);
create index portable_template_identities_target_idx
    on portable_template_identities (workspace_id, kind, target_id);
