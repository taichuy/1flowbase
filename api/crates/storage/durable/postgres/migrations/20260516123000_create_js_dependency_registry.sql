create table if not exists js_dependency_registry (
    id uuid primary key,
    installation_id uuid not null references plugin_installations(id) on delete cascade,
    provider_code text not null,
    plugin_id text not null,
    plugin_version text not null,
    alias text not null,
    package text not null,
    version text not null,
    target text not null,
    artifact_path text not null,
    integrity text not null,
    permission_network text not null default 'none',
    permission_filesystem text not null default 'none',
    permission_env text not null default 'none',
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint js_dependency_registry_unique_target unique (installation_id, alias, target),
    constraint js_dependency_registry_target_check check (target in ('backend_code')),
    constraint js_dependency_registry_integrity_check check (integrity like 'sha256-%'),
    constraint js_dependency_registry_permission_network_check
        check (permission_network in ('none', 'deny', 'outbound_only')),
    constraint js_dependency_registry_permission_filesystem_check
        check (permission_filesystem in ('none', 'deny')),
    constraint js_dependency_registry_permission_env_check
        check (permission_env in ('none', 'deny'))
);

create index if not exists idx_js_dependency_registry_installation
    on js_dependency_registry (installation_id);

create index if not exists idx_js_dependency_registry_alias
    on js_dependency_registry (alias, target);
