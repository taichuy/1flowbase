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
    md5('permission_definition:settings_feature.access.system.auth-center')::uuid,
    '00000000-0000-0000-0000-000000000000'::uuid,
    'settings_feature',
    'access',
    'system.auth-center',
    'settings_feature.access.system.auth-center',
    'settings_feature:access:system.auth-center',
    ''
  ),
  (
    md5('permission_definition:settings_feature.access.system.host-infrastructure')::uuid,
    '00000000-0000-0000-0000-000000000000'::uuid,
    'settings_feature',
    'access',
    'system.host-infrastructure',
    'settings_feature.access.system.host-infrastructure',
    'settings_feature:access:system.host-infrastructure',
    ''
  ),
  (
    md5('permission_definition:settings_feature.access.system.memory-observation')::uuid,
    '00000000-0000-0000-0000-000000000000'::uuid,
    'settings_feature',
    'access',
    'system.memory-observation',
    'settings_feature.access.system.memory-observation',
    'settings_feature:access:system.memory-observation',
    ''
  ),
  (
    md5('permission_definition:settings_feature.access.system.applications')::uuid,
    '00000000-0000-0000-0000-000000000000'::uuid,
    'settings_feature',
    'access',
    'system.applications',
    'settings_feature.access.system.applications',
    'settings_feature:access:system.applications',
    ''
  )
on conflict (code) do update set
  resource = excluded.resource,
  action = excluded.action,
  scope = excluded.scope,
  name = excluded.name,
  updated_at = now();

do $$
declare
  missing_target text;
begin
  select mapping.target_code
  into missing_target
  from (
    values
      ('settings_route.visible.settings.auth-center', 'settings_feature.access.system.auth-center'),
      ('settings_route.visible.settings.auth-center', 'user.view.all'),
      ('settings_route.visible.settings.auth-center', 'user.manage.all'),
      ('settings_route.visible.settings.host-infrastructure', 'settings_feature.access.system.host-infrastructure'),
      ('settings_route.visible.settings.host-infrastructure', 'plugin_config.view.all'),
      ('settings_route.visible.settings.host-infrastructure', 'plugin_config.configure.all'),
      ('settings_route.visible.settings.memory-observation', 'settings_feature.access.system.memory-observation'),
      ('settings_route.visible.settings.memory-observation', 'plugin_config.view.all'),
      ('settings_route.visible.settings.memory-observation', 'plugin_config.configure.all'),
      ('settings_route.visible.settings.applications', 'settings_feature.access.system.applications'),
      ('settings_route.visible.settings.applications', 'application.view.all')
  ) as mapping(old_code, target_code)
  where exists (
    select 1
    from role_permissions grants
    join permission_definitions legacy_definition
      on legacy_definition.id = grants.permission_id
    where legacy_definition.code = mapping.old_code
  )
    and not exists (
      select 1
      from permission_definitions target_definition
      where target_definition.code = mapping.target_code
    )
  limit 1;

  if missing_target is not null then
    raise exception 'explicit SettingsFeature grant migration target is missing: %', missing_target;
  end if;
end;
$$;

with grant_mapping(old_code, target_code) as (
  values
    ('settings_route.visible.settings.auth-center', 'settings_feature.access.system.auth-center'),
    ('settings_route.visible.settings.auth-center', 'user.view.all'),
    ('settings_route.visible.settings.auth-center', 'user.manage.all'),
    ('settings_route.visible.settings.host-infrastructure', 'settings_feature.access.system.host-infrastructure'),
    ('settings_route.visible.settings.host-infrastructure', 'plugin_config.view.all'),
    ('settings_route.visible.settings.host-infrastructure', 'plugin_config.configure.all'),
    ('settings_route.visible.settings.memory-observation', 'settings_feature.access.system.memory-observation'),
    ('settings_route.visible.settings.memory-observation', 'plugin_config.view.all'),
    ('settings_route.visible.settings.memory-observation', 'plugin_config.configure.all'),
    ('settings_route.visible.settings.applications', 'settings_feature.access.system.applications'),
    ('settings_route.visible.settings.applications', 'application.view.all')
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
        ('settings_route.visible.settings.auth-center', 'settings_feature.access.system.auth-center'),
        ('settings_route.visible.settings.auth-center', 'user.view.all'),
        ('settings_route.visible.settings.auth-center', 'user.manage.all'),
        ('settings_route.visible.settings.host-infrastructure', 'settings_feature.access.system.host-infrastructure'),
        ('settings_route.visible.settings.host-infrastructure', 'plugin_config.view.all'),
        ('settings_route.visible.settings.host-infrastructure', 'plugin_config.configure.all'),
        ('settings_route.visible.settings.memory-observation', 'settings_feature.access.system.memory-observation'),
        ('settings_route.visible.settings.memory-observation', 'plugin_config.view.all'),
        ('settings_route.visible.settings.memory-observation', 'plugin_config.configure.all'),
        ('settings_route.visible.settings.applications', 'settings_feature.access.system.applications'),
        ('settings_route.visible.settings.applications', 'application.view.all')
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
    raise exception 'explicit SettingsFeature grant migration verification failed';
  end if;
end;
$$;

delete from role_permissions grants
using permission_definitions definitions
where grants.permission_id = definitions.id
  and definitions.code in (
    'settings_route.visible.settings.auth-center',
    'settings_route.visible.settings.host-infrastructure',
    'settings_route.visible.settings.memory-observation',
    'settings_route.visible.settings.applications'
  );

delete from permission_definitions
where code in (
  'settings_route.visible.settings.auth-center',
  'settings_route.visible.settings.host-infrastructure',
  'settings_route.visible.settings.memory-observation',
  'settings_route.visible.settings.applications'
);
