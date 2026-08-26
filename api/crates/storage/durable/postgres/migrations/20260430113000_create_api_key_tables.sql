create table if not exists api_keys (
  id uuid primary key,
  name text not null,
  token_hash text not null unique,
  token_prefix text not null,
  creator_user_id uuid not null references users(id) on delete cascade,
  tenant_id uuid not null references tenants(id) on delete cascade,
  scope_kind text not null check (scope_kind in ('system', 'workspace')),
  scope_id uuid not null,
  enabled boolean not null default true,
  expires_at timestamptz null,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create index if not exists api_keys_scope_idx
  on api_keys (scope_kind, scope_id);

create table if not exists api_key_data_model_permissions (
  api_key_id uuid not null references api_keys(id) on delete cascade,
  data_model_id uuid not null references model_definitions(id) on delete cascade,
  allow_list boolean not null default false,
  allow_get boolean not null default false,
  allow_create boolean not null default false,
  allow_update boolean not null default false,
  allow_delete boolean not null default false,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  primary key (api_key_id, data_model_id)
);

create index if not exists api_key_data_model_permissions_model_idx
  on api_key_data_model_permissions (data_model_id);
