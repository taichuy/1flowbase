-- Plugin-owned outbound credentials; host authentication credentials are a separate domain.
create table plugin_credentials (
    publisher_namespace text not null,
    plugin_code text not null,
    scope_id uuid not null,
    credential_id text not null,
    encrypted_value jsonb not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    primary key (publisher_namespace, plugin_code, scope_id, credential_id),
    check (publisher_namespace <> '' and plugin_code <> '' and credential_id <> ''),
    check ((encrypted_value->>'algorithm' = 'aead_xchacha20poly1305_v1') is true)
);
