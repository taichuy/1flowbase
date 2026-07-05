create unique index if not exists frontstage_pages_workspace_id_id_uidx
  on frontstage_pages (workspace_id, id);

create unique index if not exists roles_workspace_id_id_uidx
  on roles (workspace_id, id);

create table if not exists frontstage_page_visibility_rules (
  id uuid primary key,
  workspace_id uuid not null references workspaces(id) on delete cascade,
  page_id uuid,
  role_id uuid not null,
  visibility text not null,
  created_by uuid,
  updated_by uuid,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  check (visibility in ('visible', 'hidden')),
  foreign key (workspace_id, page_id)
    references frontstage_pages (workspace_id, id)
    on delete cascade,
  foreign key (workspace_id, role_id)
    references roles (workspace_id, id)
    on delete cascade
);

create unique index if not exists frontstage_page_visibility_rules_root_uidx
  on frontstage_page_visibility_rules (workspace_id, role_id)
  where page_id is null;

create unique index if not exists frontstage_page_visibility_rules_page_uidx
  on frontstage_page_visibility_rules (workspace_id, page_id, role_id)
  where page_id is not null;

create index if not exists frontstage_page_visibility_rules_workspace_page_idx
  on frontstage_page_visibility_rules (workspace_id, page_id);

create index if not exists frontstage_page_visibility_rules_workspace_role_idx
  on frontstage_page_visibility_rules (workspace_id, role_id);

-- Preserve the previous workspace-access read behavior for roles that already carried
-- frontstage design or route_page permissions. Root users bypass visibility rules.
insert into frontstage_page_visibility_rules (
  id,
  workspace_id,
  page_id,
  role_id,
  visibility,
  created_by,
  updated_by
)
select
  md5('1flowbase.frontstage_page_visibility_rules.root:' || roles.workspace_id::text || ':' || roles.id::text)::uuid,
  roles.workspace_id,
  null,
  roles.id,
  'visible',
  null,
  null
from roles
where roles.scope_kind = 'workspace'
  and exists (
    select 1
    from role_permissions permissions
    join permission_definitions definitions on definitions.id = permissions.permission_id
    where permissions.role_id = roles.id
      and (
        definitions.code = 'frontstage.page.design'
        or definitions.code like 'route_page.%'
      )
  )
on conflict do nothing;
