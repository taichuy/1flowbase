do $$
declare
  conflicting_workspace_id uuid;
begin
  select legacy.workspace_id
  into conflicting_workspace_id
  from roles legacy
  join roles existing
    on existing.workspace_id = legacy.workspace_id
   and existing.scope_kind = 'workspace'
   and existing.code = 'member'
   and existing.id <> legacy.id
  where legacy.scope_kind = 'workspace'
    and legacy.code = 'manager'
    and legacy.is_builtin = true
  order by legacy.workspace_id
  limit 1;

  if conflicting_workspace_id is not null then
    raise exception using
      errcode = '23505',
      message = format(
        'manager/member role code collision in workspace %s; rename the existing custom member role before retrying',
        conflicting_workspace_id
      );
  end if;
end $$;

update users users_to_migrate
set default_display_role = 'member',
    updated_at = now()
where users_to_migrate.default_display_role = 'manager'
  and exists (
    select 1
    from user_role_bindings bindings
    join roles legacy on legacy.id = bindings.role_id
    where bindings.user_id = users_to_migrate.id
      and legacy.scope_kind = 'workspace'
      and legacy.code = 'manager'
      and legacy.is_builtin = true
  );

update api_keys keys_to_migrate
set role_code = 'member',
    updated_at = now()
where keys_to_migrate.role_code = 'manager'
  and keys_to_migrate.scope_kind = 'workspace'
  and exists (
    select 1
    from roles legacy
    where legacy.workspace_id = keys_to_migrate.scope_id
      and legacy.scope_kind = 'workspace'
      and legacy.code = 'manager'
      and legacy.is_builtin = true
  );

update roles
set code = 'member',
    system_kind = case when system_kind = 'manager' then 'member' else system_kind end,
    updated_at = now()
where scope_kind = 'workspace'
  and code = 'manager'
  and is_builtin = true;
