create table plugin_artifact_instances (
    node_id text not null,
    installation_id uuid not null references plugin_installations(id) on delete cascade,
    local_version text,
    local_checksum text,
    installed_path text,
    artifact_status text not null,
    runtime_status text not null default 'inactive',
    checked_at timestamptz not null default now(),
    last_error text,
    primary key (node_id, installation_id),
    constraint plugin_artifact_instances_artifact_status_check
        check (artifact_status in ('missing', 'ready', 'outdated', 'mismatched', 'corrupted', 'load_failed')),
    constraint plugin_artifact_instances_runtime_status_check
        check (runtime_status in ('inactive', 'active', 'load_failed')),
    constraint plugin_artifact_instances_node_id_check
        check (length(trim(node_id)) > 0)
);

create index plugin_artifact_instances_installation_id_idx
    on plugin_artifact_instances (installation_id);
