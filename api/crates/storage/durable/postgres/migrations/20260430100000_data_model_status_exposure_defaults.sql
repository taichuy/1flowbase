alter table data_source_instances
  add column if not exists default_data_model_status text not null default 'published';

alter table data_source_instances
  add column if not exists default_api_exposure_status text not null default 'published_not_exposed';

do $$
begin
  if not exists (
    select 1
    from pg_constraint
    where conname = 'data_source_instances_default_data_model_status_check'
      and conrelid = 'data_source_instances'::regclass
  ) then
    alter table data_source_instances
      add constraint data_source_instances_default_data_model_status_check
      check (default_data_model_status in ('draft', 'published', 'disabled', 'broken'));
  end if;
end $$;

do $$
begin
  if not exists (
    select 1
    from pg_constraint
    where conname = 'data_source_instances_default_api_exposure_status_check'
      and conrelid = 'data_source_instances'::regclass
  ) then
    alter table data_source_instances
      add constraint data_source_instances_default_api_exposure_status_check
      check (default_api_exposure_status in (
        'draft',
        'published_not_exposed',
        'api_exposed_no_permission',
        'api_exposed_ready',
        'unsafe_external_source'
      ));
  end if;
end $$;

alter table model_definitions
  add column if not exists data_source_instance_id uuid null references data_source_instances(id);

alter table model_definitions
  add column if not exists status text not null default 'published';

alter table model_definitions
  add column if not exists api_exposure_status text not null default 'published_not_exposed';

alter table model_definitions
  add column if not exists owner_kind text not null default 'core';

alter table model_definitions
  add column if not exists owner_id text null;

alter table model_definitions
  add column if not exists is_protected boolean not null default false;

do $$
begin
  if not exists (
    select 1
    from pg_constraint
    where conname = 'model_definitions_status_check'
      and conrelid = 'model_definitions'::regclass
  ) then
    alter table model_definitions
      add constraint model_definitions_status_check
      check (status in ('draft', 'published', 'disabled', 'broken'));
  end if;
end $$;

do $$
begin
  if not exists (
    select 1
    from pg_constraint
    where conname = 'model_definitions_api_exposure_status_check'
      and conrelid = 'model_definitions'::regclass
  ) then
    alter table model_definitions
      add constraint model_definitions_api_exposure_status_check
      check (api_exposure_status in (
        'draft',
        'published_not_exposed',
        'api_exposed_no_permission',
        'api_exposed_ready',
        'unsafe_external_source'
      ));
  end if;
end $$;

do $$
begin
  alter table model_definitions
    drop constraint if exists model_definitions_owner_kind_check;

  if not exists (
    select 1
    from pg_constraint
    where conname = 'model_definitions_owner_kind_check'
      and conrelid = 'model_definitions'::regclass
  ) then
    alter table model_definitions
      add constraint model_definitions_owner_kind_check
      check (owner_kind in ('core', 'host_extension', 'runtime_extension'));
  end if;
end $$;

alter table model_definitions
  drop constraint if exists model_definitions_scope_kind_scope_id_code_key;

create unique index if not exists model_definitions_data_source_code_uidx
  on model_definitions (data_source_instance_id, code)
  where data_source_instance_id is not null;

create unique index if not exists model_definitions_main_source_code_uidx
  on model_definitions (code)
  where data_source_instance_id is null;

create table if not exists scope_data_model_grants (
  id uuid primary key,
  scope_kind text not null check (scope_kind in ('system', 'workspace')),
  scope_id uuid not null,
  data_model_id uuid not null references model_definitions(id) on delete cascade,
  enabled boolean not null default true,
  permission_profile text not null check (permission_profile in ('owner', 'scope_all', 'system_all')),
  created_by uuid null,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (scope_kind, scope_id, data_model_id)
);

create index if not exists scope_data_model_grants_scope_model_idx
  on scope_data_model_grants (scope_kind, scope_id, data_model_id);
