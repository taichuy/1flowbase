delete from frontstage_page_visibility_rules rules
using roles
where rules.page_id is null
  and rules.workspace_id = roles.workspace_id
  and rules.role_id = roles.id
  and rules.id = md5(
    '1flowbase.frontstage_page_visibility_rules.root:'
    || roles.workspace_id::text
    || ':'
    || roles.id::text
  )::uuid
  and not exists (
    select 1
    from role_permissions permissions
    join permission_definitions definitions on definitions.id = permissions.permission_id
    where permissions.role_id = roles.id
      and definitions.code = 'frontstage.page.design'
  );

delete from role_permissions
where permission_id in (
  select id
  from permission_definitions
  where resource = 'route_page'
     or code like 'route_page.%'
);

delete from permission_definitions
where resource = 'route_page'
   or code like 'route_page.%';
