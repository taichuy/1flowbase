create table plugin_artifact_cleanup_jobs (
    id uuid primary key,
    node_id text not null,
    provider_code text not null,
    tombstone_path text not null,
    created_at timestamptz not null default now(),
    last_error text,
    last_attempt_at timestamptz,
    constraint plugin_artifact_cleanup_jobs_node_id_check
        check (btrim(node_id) <> ''),
    constraint plugin_artifact_cleanup_jobs_provider_code_check
        check (btrim(provider_code) <> ''),
    constraint plugin_artifact_cleanup_jobs_tombstone_path_check
        check (btrim(tombstone_path) <> '')
);

create index plugin_artifact_cleanup_jobs_node_created_idx
    on plugin_artifact_cleanup_jobs (node_id, created_at, id);
