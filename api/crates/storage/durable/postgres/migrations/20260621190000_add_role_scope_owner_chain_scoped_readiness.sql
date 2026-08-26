alter table roles add column if not exists scope_id uuid;
update roles
   set scope_id = case
       when scope_kind = 'system' then '00000000-0000-0000-0000-000000000000'::uuid
       when scope_kind = 'workspace' then workspace_id
       else null
   end
 where scope_id is null;

do $$
begin
    if exists (select 1 from roles where scope_id is null) then
        raise exception 'roles contains rows without a resolvable scope_id';
    end if;
end $$;

alter table roles alter column scope_id set not null;
create index if not exists roles_scope_created_id_idx on roles (scope_id, created_at, id);

alter table role_permissions add column if not exists scope_id uuid;
update role_permissions permissions
   set scope_id = roles.scope_id
  from roles
 where permissions.role_id = roles.id
   and permissions.scope_id is null;
alter table role_permissions alter column scope_id set not null;
create index if not exists role_permissions_scope_created_id_idx
    on role_permissions (scope_id, created_at, id);

alter table user_role_bindings add column if not exists scope_id uuid;
update user_role_bindings bindings
   set scope_id = roles.scope_id
  from roles
 where bindings.role_id = roles.id
   and bindings.scope_id is null;
alter table user_role_bindings alter column scope_id set not null;
create index if not exists user_role_bindings_scope_created_id_idx
    on user_role_bindings (scope_id, created_at, id);
