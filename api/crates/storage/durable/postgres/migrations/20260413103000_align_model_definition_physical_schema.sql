drop table if exists model_change_logs;
drop table if exists model_fields;
drop table if exists model_definition_versions;
drop table if exists model_definitions;

create table model_definitions (
  id uuid primary key,
  scope_kind text not null check (scope_kind in ('workspace', 'system')),
  scope_id uuid not null,
  code text not null,
  title text not null,
  physical_table_name text not null unique,
  acl_namespace text not null,
  audit_namespace text not null,
  created_by uuid references users(id) on delete set null,
  created_at timestamptz not null default now(),
  updated_by uuid references users(id) on delete set null,
  updated_at timestamptz not null default now(),
  unique (scope_kind, scope_id, code)
);

create table model_fields (
  id uuid primary key,
  data_model_id uuid not null references model_definitions(id) on delete cascade,
  code text not null,
  title text not null,
  physical_column_name text not null,
  field_kind text not null,
  is_required boolean not null default false,
  is_unique boolean not null default false,
  default_value jsonb,
  display_interface text,
  display_options jsonb not null default '{}'::jsonb,
  relation_target_model_id uuid references model_definitions(id) on delete restrict,
  relation_options jsonb not null default '{}'::jsonb,
  sort_order integer not null default 0,
  created_by uuid references users(id) on delete set null,
  created_at timestamptz not null default now(),
  updated_by uuid references users(id) on delete set null,
  updated_at timestamptz not null default now(),
  unique (data_model_id, code),
  unique (data_model_id, physical_column_name)
);

create table model_change_logs (
  id uuid primary key,
  data_model_id uuid references model_definitions(id) on delete cascade,
  action text not null,
  target_type text not null,
  target_id uuid,
  actor_user_id uuid references users(id) on delete set null,
  before_snapshot jsonb not null default '{}'::jsonb,
  after_snapshot jsonb not null default '{}'::jsonb,
  execution_status text not null check (execution_status in ('success', 'failed')),
  error_message text,
  created_at timestamptz not null default now()
);
