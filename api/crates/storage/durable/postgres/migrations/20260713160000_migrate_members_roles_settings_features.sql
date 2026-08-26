insert into permission_definitions (
  id,
  scope_id,
  resource,
  action,
  scope,
  code,
  name,
  introduction
)
values
  (
    md5('permission_definition:settings_feature.access.system.members')::uuid,
    '00000000-0000-0000-0000-000000000000'::uuid,
    'settings_feature',
    'access',
    'system.members',
    'settings_feature.access.system.members',
    'settings_feature:access:system.members',
    ''
  ),
  (
    md5('permission_definition:settings_feature.access.system.roles')::uuid,
    '00000000-0000-0000-0000-000000000000'::uuid,
    'settings_feature',
    'access',
    'system.roles',
    'settings_feature.access.system.roles',
    'settings_feature:access:system.roles',
    ''
  )
on conflict (code) do update set
  resource = excluded.resource,
  action = excluded.action,
  scope = excluded.scope,
  name = excluded.name,
  updated_at = now();

do $$
begin
  if exists (
    select 1
    from role_permissions grants
    join permission_definitions definitions on definitions.id = grants.permission_id
    where definitions.code = 'settings_route.visible.settings.members'
  ) and exists (
    select 1
    from (
      values
        ('settings_feature.access.system.members'),
        ('user.view.all'),
        ('user.manage.all'),
        ('role_permission.view.all'),
        ('role_permission.manage.all')
    ) as required(code)
    where not exists (
      select 1 from permission_definitions definitions where definitions.code = required.code
    )
  ) then
    raise exception 'members SettingsFeature grant migration target is missing';
  end if;

  if exists (
    select 1
    from role_permissions grants
    join permission_definitions definitions on definitions.id = grants.permission_id
    where definitions.code = 'settings_route.visible.settings.roles'
  ) and exists (
    select 1
    from (
      values
        ('settings_feature.access.system.roles'),
        ('role_permission.view.all'),
        ('role_permission.manage.all')
    ) as required(code)
    where not exists (
      select 1 from permission_definitions definitions where definitions.code = required.code
    )
  ) then
    raise exception 'roles SettingsFeature grant migration target is missing';
  end if;
end;
$$;

with grant_mapping(old_code, target_code) as (
  values
    ('settings_route.visible.settings.members', 'settings_feature.access.system.members'),
    ('settings_route.visible.settings.members', 'user.view.all'),
    ('settings_route.visible.settings.members', 'user.manage.all'),
    ('settings_route.visible.settings.members', 'role_permission.view.all'),
    ('settings_route.visible.settings.members', 'role_permission.manage.all'),
    ('settings_route.visible.settings.roles', 'settings_feature.access.system.roles'),
    ('settings_route.visible.settings.roles', 'role_permission.view.all'),
    ('settings_route.visible.settings.roles', 'role_permission.manage.all')
),
legacy_grants as (
  select
    grants.role_id,
    grants.scope_id,
    grants.created_by,
    grants.updated_by,
    mapping.target_code
  from role_permissions grants
  join permission_definitions legacy_definition
    on legacy_definition.id = grants.permission_id
  join grant_mapping mapping on mapping.old_code = legacy_definition.code
)
insert into role_permissions (
  id,
  role_id,
  permission_id,
  scope_id,
  created_by,
  updated_by
)
select
  md5(
    'settings_feature_grant:' || legacy_grants.role_id::text || ':' || target.id::text
  )::uuid,
  legacy_grants.role_id,
  target.id,
  legacy_grants.scope_id,
  legacy_grants.created_by,
  legacy_grants.updated_by
from legacy_grants
join permission_definitions target on target.code = legacy_grants.target_code
on conflict (role_id, permission_id) do nothing;

do $$
begin
  if exists (
    with grant_mapping(old_code, target_code) as (
      values
        ('settings_route.visible.settings.members', 'settings_feature.access.system.members'),
        ('settings_route.visible.settings.members', 'user.view.all'),
        ('settings_route.visible.settings.members', 'user.manage.all'),
        ('settings_route.visible.settings.members', 'role_permission.view.all'),
        ('settings_route.visible.settings.members', 'role_permission.manage.all'),
        ('settings_route.visible.settings.roles', 'settings_feature.access.system.roles'),
        ('settings_route.visible.settings.roles', 'role_permission.view.all'),
        ('settings_route.visible.settings.roles', 'role_permission.manage.all')
    )
    select 1
    from role_permissions legacy_grant
    join permission_definitions legacy_definition
      on legacy_definition.id = legacy_grant.permission_id
    join grant_mapping mapping on mapping.old_code = legacy_definition.code
    join permission_definitions target_definition on target_definition.code = mapping.target_code
    where not exists (
      select 1
      from role_permissions migrated_grant
      where migrated_grant.role_id = legacy_grant.role_id
        and migrated_grant.permission_id = target_definition.id
    )
  ) then
    raise exception 'SettingsFeature grant migration verification failed';
  end if;
end;
$$;

delete from role_permissions grants
using permission_definitions definitions
where grants.permission_id = definitions.id
  and definitions.code in (
    'settings_route.visible.settings.members',
    'settings_route.visible.settings.roles'
  );

delete from permission_definitions
where code in (
  'settings_route.visible.settings.members',
  'settings_route.visible.settings.roles'
);
