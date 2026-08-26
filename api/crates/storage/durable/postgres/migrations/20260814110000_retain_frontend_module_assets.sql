create table retained_frontend_module_assets (
    installation_id uuid not null
        references extension_installations(id) on delete cascade,
    module_source text not null,
    sha256 text not null,
    media_type text not null,
    bytes bytea not null,
    created_at timestamptz not null default now(),
    primary key (installation_id, module_source, sha256),
    constraint retained_frontend_module_assets_module_source_check
        check (btrim(module_source) <> ''),
    constraint retained_frontend_module_assets_sha256_check
        check (sha256 ~ '^[0-9a-f]{64}$'),
    constraint retained_frontend_module_assets_media_type_check
        check (btrim(media_type) <> '')
);

create index retained_frontend_module_assets_sha256_idx
    on retained_frontend_module_assets (sha256);
