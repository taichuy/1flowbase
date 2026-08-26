create table if not exists tenants (
  id uuid primary key,
  code text not null unique,
  name text not null,
  is_root boolean not null default false,
  is_hidden boolean not null default false,
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now()
);

create table if not exists users (
  id uuid primary key,
  account text not null unique,
  email text not null unique,
  phone text unique,
  password_hash text not null,
  name text not null,
  nickname text not null,
  avatar_url text,
  introduction text not null default '',
  default_display_role text,
  email_login_enabled boolean not null default true,
  phone_login_enabled boolean not null default false,
  status text not null,
  session_version bigint not null default 1,
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now(),
  check (status in ('active', 'disabled'))
);

create table if not exists workspaces (
  id uuid primary key,
  tenant_id uuid not null references tenants(id) on delete cascade,
  name text not null,
  logo_url text,
  introduction text not null default '',
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now()
);

create unique index if not exists workspaces_tenant_name_uidx
  on workspaces (tenant_id, lower(name));

create table if not exists workspace_memberships (
  id uuid primary key,
  workspace_id uuid not null references workspaces(id) on delete cascade,
  user_id uuid not null references users(id) on delete cascade,
  introduction text not null default '',
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now(),
  unique (workspace_id, user_id)
);

create table if not exists roles (
  id uuid primary key,
  scope_kind text not null,
  workspace_id uuid references workspaces(id) on delete cascade,
  code text not null,
  name text not null,
  introduction text not null default '',
  is_builtin boolean not null default false,
  is_editable boolean not null default true,
  system_kind text,
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now(),
  check (scope_kind in ('system', 'workspace'))
);

create unique index if not exists roles_system_code_uidx
  on roles (code)
  where scope_kind = 'system';

create unique index if not exists roles_workspace_code_uidx
  on roles (workspace_id, code)
  where scope_kind = 'workspace';

create table if not exists permission_definitions (
  id uuid primary key,
  resource text not null,
  action text not null,
  scope text not null,
  code text not null unique,
  name text not null,
  introduction text not null default '',
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now()
);

create table if not exists role_permissions (
  id uuid primary key,
  role_id uuid not null references roles(id) on delete cascade,
  permission_id uuid not null references permission_definitions(id) on delete cascade,
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now(),
  unique (role_id, permission_id)
);

create table if not exists user_role_bindings (
  id uuid primary key,
  user_id uuid not null references users(id) on delete cascade,
  role_id uuid not null references roles(id) on delete cascade,
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now(),
  unique (user_id, role_id)
);

create table if not exists authenticators (
  id uuid primary key,
  name text not null unique,
  auth_type text not null,
  title text not null,
  enabled boolean not null default true,
  is_builtin boolean not null default false,
  sort_order integer not null default 0,
  options jsonb not null default '{}'::jsonb,
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now()
);

create table if not exists user_auth_identities (
  id uuid primary key,
  user_id uuid not null references users(id) on delete cascade,
  authenticator_name text not null references authenticators(name) on delete cascade,
  subject_type text not null,
  subject_value text not null,
  metadata jsonb not null default '{}'::jsonb,
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now()
);

create unique index if not exists user_auth_identities_subject_uidx
  on user_auth_identities (authenticator_name, subject_type, lower(subject_value));

create table if not exists audit_logs (
  id uuid primary key,
  workspace_id uuid references workspaces(id) on delete set null,
  actor_user_id uuid references users(id) on delete set null,
  target_type text not null,
  target_id uuid,
  event_code text not null,
  payload jsonb not null default '{}'::jsonb,
  created_at timestamptz not null default now()
);
