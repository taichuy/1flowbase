create table if not exists role_data_policies (
  id uuid primary key,
  role_id uuid not null references roles(id) on delete cascade,
  can_view boolean not null default false,
  can_create boolean not null default false,
  can_update boolean not null default false,
  can_delete boolean not null default false,
  default_view_scope text not null default 'own',
  default_update_scope text not null default 'own',
  default_delete_scope text not null default 'own',
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now(),
  unique (role_id),
  check (default_view_scope in ('own', 'scope_all', 'system_all')),
  check (default_update_scope in ('own', 'scope_all', 'system_all')),
  check (default_delete_scope in ('own', 'scope_all', 'system_all'))
);

create table if not exists role_data_model_policies (
  id uuid primary key,
  role_id uuid not null references roles(id) on delete cascade,
  data_model_id uuid not null references model_definitions(id) on delete cascade,
  view_scope_override text,
  update_scope_override text,
  delete_scope_override text,
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now(),
  unique (role_id, data_model_id),
  check (view_scope_override is null or view_scope_override in ('own', 'scope_all', 'system_all')),
  check (update_scope_override is null or update_scope_override in ('own', 'scope_all', 'system_all')),
  check (delete_scope_override is null or delete_scope_override in ('own', 'scope_all', 'system_all'))
);

insert into role_data_policies (
  id,
  role_id,
  can_view,
  can_create,
  can_update,
  can_delete,
  default_view_scope,
  default_update_scope,
  default_delete_scope
)
select (
    substr(generated.generated_id, 1, 8)
    || '-'
    || substr(generated.generated_id, 9, 4)
    || '-'
    || substr(generated.generated_id, 13, 4)
    || '-'
    || substr(generated.generated_id, 17, 4)
    || '-'
    || substr(generated.generated_id, 21, 12)
  )::uuid,
  roles.id,
  case when roles.code in ('root', 'admin', 'manager') then true else false end,
  case when roles.code in ('root', 'admin', 'manager') then true else false end,
  case when roles.code in ('root', 'admin', 'manager') then true else false end,
  case when roles.code in ('root', 'admin', 'manager') then true else false end,
  case
    when roles.code = 'root' then 'system_all'
    when roles.code = 'admin' then 'scope_all'
    else 'own'
  end,
  case
    when roles.code = 'root' then 'system_all'
    when roles.code = 'admin' then 'scope_all'
    else 'own'
  end,
  case
    when roles.code = 'root' then 'system_all'
    when roles.code = 'admin' then 'scope_all'
    else 'own'
  end
from roles
cross join lateral (
  select md5('role_data_policy:' || roles.id::text) as generated_id
) generated
on conflict (role_id) do nothing;
