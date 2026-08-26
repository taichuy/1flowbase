create table extension_installations (
    id uuid primary key,
    category text not null,
    organization text not null,
    artifact_id text not null,
    artifact_version text not null,
    node_id text not null,
    source text not null,
    trust text not null,
    local_path text not null,
    checksum text not null,
    signature_status text not null,
    signature_algorithm text,
    signing_key_id text,
    warnings jsonb not null default '[]'::jsonb,
    receipt jsonb not null default '{}'::jsonb,
    status text not null,
    installed_by uuid not null references users(id) on delete restrict,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint extension_installations_category_check check (
        category in (
            'agent-flow', 'capability-plugins', 'host-extensions',
            'i18n', 'mcp', 'runtime-extensions'
        )
    ),
    constraint extension_installations_signature_status_check check (
        signature_status in ('verified', 'missing', 'unknown_key', 'invalid')
    ),
    constraint extension_installations_status_check check (status in ('installed', 'missing')),
    constraint extension_installations_identity_values_check check (
        organization <> '' and artifact_id <> '' and artifact_version <> '' and node_id <> ''
    ),
    constraint extension_installations_local_path_check check (local_path <> ''),
    constraint extension_installations_source_trust_check check (source <> '' and trust <> ''),
    constraint extension_installations_identity_unique unique (
        category, organization, artifact_id, artifact_version, node_id
    )
);

create index extension_installations_node_updated_idx
    on extension_installations (node_id, updated_at desc, id desc);
