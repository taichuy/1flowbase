create table flows (
    id uuid primary key,
    application_id uuid not null unique references applications(id) on delete cascade,
    created_by uuid not null references users(id),
    updated_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table flow_drafts (
    id uuid primary key,
    flow_id uuid not null unique references flows(id) on delete cascade,
    schema_version text not null,
    document jsonb not null,
    updated_by uuid not null references users(id),
    updated_at timestamptz not null default now()
);

create table flow_versions (
    id uuid primary key,
    flow_id uuid not null references flows(id) on delete cascade,
    sequence bigint not null,
    trigger text not null check (trigger in ('autosave', 'restore')),
    change_kind text not null check (change_kind in ('logical')),
    summary text not null,
    document jsonb not null,
    created_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    unique(flow_id, sequence)
);

create index flow_versions_flow_sequence_idx
    on flow_versions (flow_id, sequence desc);
