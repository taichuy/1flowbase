create unique index if not exists role_console_policy_migration_runs_revision_uidx
  on role_console_policy_migration_runs (id, catalog_fingerprint, mapping_fingerprint);

create table if not exists role_console_policy_migration_run_artifacts (
  run_id uuid primary key references role_console_policy_migration_runs(id) on delete cascade,
  catalog_fingerprint text not null,
  mapping_fingerprint text not null,
  compiled_catalog jsonb not null,
  legacy_mappings jsonb not null,
  actor_role_bindings jsonb not null,
  created_at timestamptz not null default now(),
  constraint role_console_policy_migration_run_artifacts_revision_fk
    foreign key (run_id, catalog_fingerprint, mapping_fingerprint)
    references role_console_policy_migration_runs (id, catalog_fingerprint, mapping_fingerprint)
    on delete cascade,
  constraint role_console_policy_migration_run_artifacts_json_check check (
    jsonb_typeof(compiled_catalog) = 'object'
    and jsonb_typeof(legacy_mappings) = 'array'
    and jsonb_typeof(actor_role_bindings) = 'array'
  )
);

create table if not exists role_console_policy_migration_actor_previews (
  run_id uuid not null references role_console_policy_migration_runs(id) on delete cascade,
  actor_user_id uuid not null,
  role_ids jsonb not null,
  probes jsonb not null,
  effective_before jsonb not null,
  effective_after jsonb not null,
  effective_delta jsonb not null,
  status text not null,
  applied_at timestamptz,
  primary key (run_id, actor_user_id),
  constraint role_console_policy_migration_actor_previews_json_check check (
    jsonb_typeof(role_ids) = 'array'
    and jsonb_typeof(probes) = 'array'
    and jsonb_typeof(effective_before) = 'array'
    and jsonb_typeof(effective_after) = 'array'
    and effective_before = effective_after
    and jsonb_typeof(effective_delta) = 'array'
    and jsonb_array_length(effective_delta) = 0
  ),
  constraint role_console_policy_migration_actor_previews_status_check check (
    (status = 'previewed' and applied_at is null)
    or (status in ('applied', 'rolled_back') and applied_at is not null)
  )
);

create table if not exists role_console_policy_migration_cutover_state (
  singleton boolean primary key default true,
  marker text not null,
  run_id uuid,
  catalog_fingerprint text,
  mapping_fingerprint text,
  updated_at timestamptz not null default now(),
  constraint role_console_policy_migration_cutover_state_singleton_check check (singleton),
  constraint role_console_policy_migration_cutover_state_marker_check
    check (marker in ('legacy', 'fenced', 'console_policy')),
  constraint role_console_policy_migration_cutover_state_revision_fk
    foreign key (run_id, catalog_fingerprint, mapping_fingerprint)
    references role_console_policy_migration_runs (id, catalog_fingerprint, mapping_fingerprint),
  constraint role_console_policy_migration_cutover_state_shape_check check (
    (marker = 'legacy'
      and run_id is null
      and catalog_fingerprint is null
      and mapping_fingerprint is null)
    or (marker in ('fenced', 'console_policy')
      and run_id is not null
      and catalog_fingerprint = btrim(catalog_fingerprint)
      and catalog_fingerprint <> ''
      and mapping_fingerprint = btrim(mapping_fingerprint)
      and mapping_fingerprint <> '')
  )
);

insert into role_console_policy_migration_cutover_state (singleton, marker)
values (true, 'legacy')
on conflict (singleton) do nothing;
