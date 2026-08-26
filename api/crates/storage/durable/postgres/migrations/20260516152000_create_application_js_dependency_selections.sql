create table if not exists application_js_dependency_selections (
    id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    application_id uuid not null references applications(id) on delete cascade,
    installation_id uuid not null references plugin_installations(id) on delete restrict,
    provider_code text not null,
    plugin_id text not null,
    plugin_version text not null,
    alias text not null,
    package text not null,
    version text not null,
    target text not null,
    artifact_path text not null,
    artifact_hash text not null,
    integrity text not null,
    permission_network text not null default 'none',
    permission_filesystem text not null default 'none',
    permission_env text not null default 'none',
    created_by uuid not null references users(id),
    updated_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint application_js_dependency_selections_unique_target
        unique (workspace_id, application_id, alias, target),
    constraint application_js_dependency_selections_target_check
        check (target in ('backend_code')),
    constraint application_js_dependency_selections_artifact_hash_check
        check (artifact_hash <> ''),
    constraint application_js_dependency_selections_integrity_check
        check (integrity like 'sha256-%'),
    constraint application_js_dependency_selections_permission_network_check
        check (permission_network in ('none', 'deny', 'outbound_only')),
    constraint application_js_dependency_selections_permission_filesystem_check
        check (permission_filesystem in ('none', 'deny')),
    constraint application_js_dependency_selections_permission_env_check
        check (permission_env in ('none', 'deny'))
);

create index if not exists idx_application_js_dependency_selections_application
    on application_js_dependency_selections (workspace_id, application_id);
