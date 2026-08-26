create table if not exists role_console_policy_migration_runs (
  id uuid primary key,
  source_contract text not null,
  catalog_fingerprint text not null,
  mapping_fingerprint text not null,
  source_filter jsonb not null,
  source_snapshot jsonb not null,
  status text not null,
  cutover_marker text not null default 'legacy',
  write_fenced boolean not null default false,
  created_at timestamptz not null default now(),
  applied_at timestamptz,
  finalized_by uuid,
  finalized_at timestamptz,
  rollback_verified_at timestamptz,
  constraint role_console_policy_migration_runs_identity_check check (
    source_contract = btrim(source_contract) and source_contract <> ''
    and catalog_fingerprint = btrim(catalog_fingerprint) and catalog_fingerprint <> ''
    and mapping_fingerprint = btrim(mapping_fingerprint) and mapping_fingerprint <> ''
  ),
  constraint role_console_policy_migration_runs_json_check check (
    jsonb_typeof(source_filter) = 'object'
    and jsonb_typeof(source_snapshot) = 'object'
  ),
  constraint role_console_policy_migration_runs_status_check
    check (status in ('previewed', 'applied_fenced', 'applied', 'rolled_back')),
  constraint role_console_policy_migration_runs_cutover_marker_check
    check (cutover_marker in ('legacy', 'console_policy')),
  constraint role_console_policy_migration_runs_state_check check (
    (status = 'previewed' and cutover_marker = 'legacy' and not write_fenced
      and applied_at is null and finalized_by is null and finalized_at is null
      and rollback_verified_at is null)
    or (status = 'applied_fenced' and cutover_marker = 'console_policy' and write_fenced
      and applied_at is not null and finalized_by is null and finalized_at is null
      and rollback_verified_at is null)
    or (status = 'applied' and cutover_marker = 'console_policy' and not write_fenced
      and applied_at is not null and finalized_by is not null and finalized_at is not null
      and rollback_verified_at is null)
    or (status = 'rolled_back' and cutover_marker = 'legacy' and not write_fenced
      and applied_at is not null and finalized_by is null and finalized_at is null
      and rollback_verified_at is not null)
  )
);

create unique index if not exists role_console_policy_single_write_fence_uidx
  on role_console_policy_migration_runs (write_fenced)
  where write_fenced;

create table if not exists role_console_policy_migration_role_previews (
  run_id uuid not null references role_console_policy_migration_runs(id) on delete cascade,
  role_id uuid not null references roles(id) on delete cascade,
  source_grants jsonb not null,
  projected_policy jsonb not null,
  authorization_delta jsonb not null,
  effective_before jsonb not null,
  effective_after jsonb not null,
  effective_delta jsonb not null,
  status text not null,
  applied_at timestamptz,
  primary key (run_id, role_id),
  constraint role_console_policy_migration_role_previews_json_check check (
    jsonb_typeof(source_grants) = 'array'
    and jsonb_typeof(projected_policy) = 'object'
    and jsonb_typeof(authorization_delta) = 'object'
    and jsonb_typeof(authorization_delta -> 'added') = 'array'
    and jsonb_typeof(authorization_delta -> 'removed') = 'array'
    and jsonb_array_length(authorization_delta -> 'added') = 0
    and jsonb_array_length(authorization_delta -> 'removed') = 0
    and jsonb_typeof(effective_before) = 'array'
    and jsonb_typeof(effective_after) = 'array'
    and effective_before = effective_after
    and jsonb_typeof(effective_delta) = 'array'
    and jsonb_array_length(effective_delta) = 0
  ),
  constraint role_console_policy_migration_role_previews_status_check check (
    (status = 'previewed' and applied_at is null)
    or (status in ('applied', 'rolled_back') and applied_at is not null)
  )
);

create table if not exists role_console_group_policy_snapshots (
  run_id uuid not null references role_console_policy_migration_runs(id) on delete cascade,
  group_policy_id uuid not null,
  role_id uuid not null references roles(id) on delete cascade,
  group_kind text not null,
  group_id text not null,
  mode text not null,
  created_by uuid,
  created_at timestamptz not null,
  updated_by uuid,
  updated_at timestamptz not null,
  primary key (run_id, group_policy_id)
);

create table if not exists role_console_operation_policy_snapshots (
  run_id uuid not null references role_console_policy_migration_runs(id) on delete cascade,
  operation_policy_id uuid not null,
  role_id uuid not null references roles(id) on delete cascade,
  group_policy_id uuid not null,
  group_mode text not null,
  operation_id text not null,
  policy_kind text not null,
  simple_enabled boolean,
  row_scope text,
  created_by uuid,
  created_at timestamptz not null,
  updated_by uuid,
  updated_at timestamptz not null,
  primary key (run_id, operation_policy_id)
);

create or replace function enforce_role_console_policy_migration_write_fence()
returns trigger
language plpgsql
as $$
declare
  fenced_run_id uuid;
  caller_run_id text;
begin
  select id into fenced_run_id
  from role_console_policy_migration_runs
  where write_fenced
  limit 1;

  if fenced_run_id is null then
    if tg_op = 'DELETE' then
      return old;
    end if;
    return new;
  end if;

  caller_run_id := current_setting('oneflow.role_console_policy_migration_run_id', true);
  if caller_run_id is null or caller_run_id = '' or caller_run_id <> fenced_run_id::text then
    raise exception 'console policy migration write fence is active';
  end if;
  if tg_op = 'DELETE' then
    return old;
  end if;
  return new;
end;
$$;

drop trigger if exists role_console_group_policy_migration_write_fence
  on role_console_group_policies;
create trigger role_console_group_policy_migration_write_fence
before insert or update or delete on role_console_group_policies
for each row execute function enforce_role_console_policy_migration_write_fence();

drop trigger if exists role_console_operation_policy_migration_write_fence
  on role_console_operation_policies;
create trigger role_console_operation_policy_migration_write_fence
before insert or update or delete on role_console_operation_policies
for each row execute function enforce_role_console_policy_migration_write_fence();

drop trigger if exists legacy_role_permission_migration_write_fence
  on role_permissions;
create trigger legacy_role_permission_migration_write_fence
before insert or update or delete on role_permissions
for each row execute function enforce_role_console_policy_migration_write_fence();

drop trigger if exists legacy_permission_definition_migration_write_fence
  on permission_definitions;
create trigger legacy_permission_definition_migration_write_fence
before insert or update or delete on permission_definitions
for each row execute function enforce_role_console_policy_migration_write_fence();

drop trigger if exists legacy_role_binding_migration_write_fence
  on user_role_bindings;
create trigger legacy_role_binding_migration_write_fence
before insert or update or delete on user_role_bindings
for each row execute function enforce_role_console_policy_migration_write_fence();
