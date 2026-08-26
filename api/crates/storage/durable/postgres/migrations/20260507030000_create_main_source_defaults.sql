create table if not exists main_source_defaults (
    workspace_id uuid primary key references workspaces(id) on delete cascade,
    default_data_model_status text not null default 'published',
    default_api_exposure_status text not null default 'published_not_exposed',
    updated_by uuid null references users(id) on delete set null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

do $$
begin
  if not exists (
    select 1 from pg_constraint
    where conname = 'main_source_defaults_data_model_status_check'
      and conrelid = 'main_source_defaults'::regclass
  ) then
    alter table main_source_defaults
      add constraint main_source_defaults_data_model_status_check
      check (default_data_model_status in ('draft', 'published', 'disabled', 'broken'));
  end if;

  if not exists (
    select 1 from pg_constraint
    where conname = 'main_source_defaults_api_exposure_status_check'
      and conrelid = 'main_source_defaults'::regclass
  ) then
    alter table main_source_defaults
      add constraint main_source_defaults_api_exposure_status_check
      check (default_api_exposure_status in ('draft', 'published_not_exposed', 'api_exposed_no_permission'));
  end if;
end $$;
