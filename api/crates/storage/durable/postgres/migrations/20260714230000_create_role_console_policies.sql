create table if not exists role_console_group_policies (
  id uuid primary key,
  role_id uuid not null references roles(id) on delete cascade,
  group_kind text not null,
  group_id text not null,
  mode text not null,
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now(),
  constraint role_console_group_policies_group_kind_check
    check (group_kind in ('settings_feature', 'other')),
  constraint role_console_group_policies_group_id_check
    check (group_id ~ '^[A-Za-z0-9][A-Za-z0-9._-]*$'),
  constraint role_console_group_policies_mode_check
    check (mode in ('disabled', 'full', 'custom')),
  constraint role_console_group_policies_role_group_key
    unique (role_id, group_kind, group_id),
  constraint role_console_group_policies_id_role_key
    unique (id, role_id),
  constraint role_console_group_policies_id_role_mode_key
    unique (id, role_id, mode)
);

create index if not exists role_console_group_policies_role_id_idx
  on role_console_group_policies (role_id, group_kind, group_id);

create table if not exists role_console_operation_policies (
  id uuid primary key,
  role_id uuid not null references roles(id) on delete cascade,
  group_policy_id uuid not null,
  group_mode text not null,
  operation_id text not null,
  policy_kind text not null,
  simple_enabled boolean,
  row_scope text,
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now(),
  constraint role_console_operation_policies_group_fk
    foreign key (group_policy_id, role_id, group_mode)
    references role_console_group_policies (id, role_id, mode)
    on delete cascade,
  constraint role_console_operation_policies_group_mode_check
    check (group_mode = 'custom'),
  constraint role_console_operation_policies_operation_id_check
    check (operation_id ~ '^[A-Za-z0-9][A-Za-z0-9._-]*$'),
  constraint role_console_operation_policies_value_check
    check (
      (policy_kind = 'simple' and simple_enabled is not null and row_scope is null)
      or
      (policy_kind = 'row' and simple_enabled is null and row_scope in ('disabled', 'own', 'scope_all'))
    ),
  constraint role_console_operation_policies_role_operation_key
    unique (role_id, operation_id)
);

create index if not exists role_console_operation_policies_group_idx
  on role_console_operation_policies (group_policy_id, operation_id);

create table if not exists role_console_policy_migration_ledger (
  id uuid primary key,
  role_id uuid not null references roles(id) on delete cascade,
  source_contract text not null,
  catalog_fingerprint text not null,
  mapping_fingerprint text not null,
  catalog_complete boolean not null,
  source_grants jsonb not null,
  projected_policy jsonb not null,
  authorization_delta jsonb not null,
  status text not null,
  created_at timestamptz not null default now(),
  applied_at timestamptz,
  constraint role_console_policy_migration_source_contract_check
    check (source_contract = btrim(source_contract) and source_contract <> ''),
  constraint role_console_policy_migration_json_shape_check
    check (
      jsonb_typeof(source_grants) = 'array'
      and jsonb_typeof(projected_policy) = 'object'
      and jsonb_typeof(authorization_delta) = 'object'
      and jsonb_typeof(authorization_delta -> 'added') = 'array'
      and jsonb_typeof(authorization_delta -> 'removed') = 'array'
    ),
  constraint role_console_policy_migration_status_check
    check (status in ('previewed', 'applied', 'rolled_back')),
  constraint role_console_policy_migration_apply_check
    check (
      status <> 'applied'
      or (
        catalog_complete
        and jsonb_array_length(authorization_delta -> 'added') = 0
        and jsonb_array_length(authorization_delta -> 'removed') = 0
        and applied_at is not null
      )
    ),
  constraint role_console_policy_migration_ledger_revision_key
    unique (role_id, source_contract, catalog_fingerprint, mapping_fingerprint)
);
