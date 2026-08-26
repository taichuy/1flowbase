alter table applications
    add column if not exists api_enabled boolean not null default false;

alter table api_keys
    add column if not exists key_kind text not null default 'data_model_api_key',
    add column if not exists application_id uuid null references applications(id) on delete cascade;

do $$
begin
    if not exists (
        select 1
        from pg_constraint
        where conrelid = 'api_keys'::regclass
          and conname = 'api_keys_key_kind_check'
    ) then
        alter table api_keys
            add constraint api_keys_key_kind_check
            check (key_kind in ('data_model_api_key', 'application_api_key'));
    end if;

    if not exists (
        select 1
        from pg_constraint
        where conrelid = 'api_keys'::regclass
          and conname = 'api_keys_application_key_application_required_check'
    ) then
        alter table api_keys
            add constraint api_keys_application_key_application_required_check
            check (key_kind <> 'application_api_key' or application_id is not null);
    end if;
end $$;

create index if not exists api_keys_application_creator_created_idx
    on api_keys (application_id, creator_user_id, created_at desc, id desc);

create table if not exists application_api_mappings (
    application_id uuid primary key references applications(id) on delete cascade,
    mapping_config jsonb not null,
    updated_by uuid not null references users(id),
    updated_at timestamptz not null default now()
);

create table if not exists application_publication_versions (
    id uuid primary key,
    application_id uuid not null references applications(id) on delete cascade,
    flow_id uuid not null references flows(id) on delete cascade,
    flow_version_id uuid not null references flow_versions(id) on delete restrict,
    compiled_plan_id uuid not null references flow_compiled_plans(id) on delete restrict,
    version_sequence bigint not null,
    active boolean not null default false,
    api_enabled boolean not null default true,
    flow_schema_version text not null,
    document_hash text not null,
    document_snapshot jsonb not null,
    mapping_snapshot jsonb not null,
    runtime_profile_snapshot jsonb not null default '{}'::jsonb,
    output_selector jsonb not null default '{}'::jsonb,
    created_by uuid not null references users(id),
    created_at timestamptz not null default now()
);

create unique index if not exists application_publication_versions_active_idx
    on application_publication_versions (application_id)
    where active;

create unique index if not exists application_publication_versions_application_sequence_idx
    on application_publication_versions (application_id, version_sequence);
