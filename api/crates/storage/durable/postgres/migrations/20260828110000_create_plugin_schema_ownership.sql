create table if not exists plugin_schema_ownership (
    ownership_key text primary key,
    owner_id text not null,
    owner_version text not null,
    object_kind text not null check (object_kind in ('owned_collection', 'owned_field', 'extension_field')),
    logical_name text not null,
    physical_table text not null,
    physical_column text,
    field_type text,
    nullable boolean,
    active boolean not null default true,
    plan_fingerprint text not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    check (
        (object_kind = 'owned_collection' and physical_column is null and field_type is null and nullable is null)
        or (object_kind in ('owned_field', 'extension_field') and physical_column is not null and field_type is not null and nullable is not null)
    )
);

create unique index if not exists plugin_schema_ownership_physical_object_idx
    on plugin_schema_ownership (physical_table, coalesce(physical_column, ''));

create index if not exists plugin_schema_ownership_owner_idx
    on plugin_schema_ownership (owner_id, active, ownership_key);

create table if not exists plugin_schema_reconcile_receipts (
    receipt_id uuid primary key,
    owner_id text not null,
    owner_version text not null,
    plan_fingerprint text not null,
    created_objects integer not null check (created_objects >= 0),
    existing_objects integer not null check (existing_objects >= 0),
    retained_objects integer not null check (retained_objects >= 0),
    applied_at timestamptz not null default now(),
    unique (owner_id, plan_fingerprint)
);
