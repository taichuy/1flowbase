create table application_extension_sources (
    application_id uuid primary key references applications(id) on delete cascade,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    extension_installation_id uuid not null references extension_installations(id) on delete restrict,
    imported_by uuid not null references users(id) on delete restrict,
    imported_at timestamptz not null default now()
);

create index application_extension_sources_workspace_installation_idx
    on application_extension_sources (workspace_id, extension_installation_id);
