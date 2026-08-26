create table plugin_package_catalog_projection (
    installation_id uuid primary key references plugin_installations(id) on delete cascade,
    package_code text not null,
    package_version text not null,
    catalog_snapshot_json jsonb not null default '{}'::jsonb,
    projection_status text not null check (projection_status in ('ok', 'missing', 'failed')),
    last_error_message text,
    refreshed_at timestamptz,
    updated_at timestamptz not null default now()
);

create index plugin_package_catalog_projection_package_idx
    on plugin_package_catalog_projection (package_code, package_version);

create index plugin_package_catalog_projection_status_idx
    on plugin_package_catalog_projection (projection_status, updated_at desc);
