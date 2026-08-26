alter table roles
  add column if not exists auto_grant_new_permissions boolean not null default false;

alter table roles
  add column if not exists is_default_member_role boolean not null default false;

create unique index if not exists roles_workspace_default_member_role_uidx
  on roles (workspace_id)
  where scope_kind = 'workspace' and is_default_member_role = true;

update roles
set auto_grant_new_permissions = true
where scope_kind = 'workspace' and code = 'admin';

update roles
set auto_grant_new_permissions = false
where code in ('manager', 'root');

update roles
set is_default_member_role = true
where scope_kind = 'workspace' and code = 'manager';

update roles
set is_default_member_role = false
where code in ('admin', 'root');
