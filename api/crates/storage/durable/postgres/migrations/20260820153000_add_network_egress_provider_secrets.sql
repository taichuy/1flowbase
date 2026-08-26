create table network_egress_provider_secrets (
    provider_id uuid primary key references network_egress_providers(id) on delete cascade,
    secret_ref text not null unique check (secret_ref like 'secret://%'),
    encrypted_secret_json jsonb not null,
    secret_version integer not null check (secret_version > 0),
    updated_at timestamptz not null default now()
);
