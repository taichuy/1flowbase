create table provider_protocol_capsules (
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    capsule_kind text not null check (capsule_kind in ('protocol_context', 'continuation')),
    slot_key text not null,
    encrypted_payload jsonb not null,
    plaintext_digest text not null,
    plaintext_size_bytes bigint not null check (plaintext_size_bytes > 0),
    key_version text not null,
    hard_expires_at timestamptz not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    primary key (flow_run_id, capsule_kind, slot_key)
);

create index provider_protocol_capsules_expiry_idx
    on provider_protocol_capsules (hard_expires_at, flow_run_id);
