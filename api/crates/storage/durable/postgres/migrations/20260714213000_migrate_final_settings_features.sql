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
    md5('permission_definition:settings_feature.access.system.docs')::uuid,
    '00000000-0000-0000-0000-000000000000'::uuid,
    'settings_feature',
    'access',
    'system.docs',
    'settings_feature.access.system.docs',
    'settings_feature:access:system.docs',
    ''
  ),
  (
    md5('permission_definition:settings_feature.access.system.api-key-authentication')::uuid,
    '00000000-0000-0000-0000-000000000000'::uuid,
    'settings_feature',
    'access',
    'system.api-key-authentication',
    'settings_feature.access.system.api-key-authentication',
    'settings_feature:access:system.api-key-authentication',
    ''
  ),
  (
    md5('permission_definition:settings_feature.access.system.system-runtime')::uuid,
    '00000000-0000-0000-0000-000000000000'::uuid,
    'settings_feature',
    'access',
    'system.system-runtime',
    'settings_feature.access.system.system-runtime',
    'settings_feature:access:system.system-runtime',
    ''
  ),
  (
    md5('permission_definition:settings_feature.access.system.mcp-management')::uuid,
    '00000000-0000-0000-0000-000000000000'::uuid,
    'settings_feature',
    'access',
    'system.mcp-management',
    'settings_feature.access.system.mcp-management',
    'settings_feature:access:system.mcp-management',
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
      ('settings_route.visible.settings.docs', 'settings_feature.access.system.docs'),
      ('settings_route.visible.settings.docs', 'api_reference.view.all'),
      ('settings_route.visible.settings.api-key-authentication', 'settings_feature.access.system.api-key-authentication'),
      ('settings_route.visible.settings.system-runtime', 'settings_feature.access.system.system-runtime'),
      ('settings_route.visible.settings.system-runtime', 'system_runtime.view.all'),
      ('settings_route.visible.settings.mcp-management', 'settings_feature.access.system.mcp-management'),
      ('settings_route.visible.settings.mcp-management', 'mcp_management.view.all'),
      ('settings_route.visible.settings.mcp-management', 'mcp_management.manage.all')
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
    raise exception 'final SettingsFeature grant migration target is missing: %', missing_target;
  end if;
end;
$$;

with grant_mapping(old_code, target_code) as (
  values
    ('settings_route.visible.settings.docs', 'settings_feature.access.system.docs'),
    ('settings_route.visible.settings.docs', 'api_reference.view.all'),
    ('settings_route.visible.settings.api-key-authentication', 'settings_feature.access.system.api-key-authentication'),
    ('settings_route.visible.settings.system-runtime', 'settings_feature.access.system.system-runtime'),
    ('settings_route.visible.settings.system-runtime', 'system_runtime.view.all'),
    ('settings_route.visible.settings.mcp-management', 'settings_feature.access.system.mcp-management'),
    ('settings_route.visible.settings.mcp-management', 'mcp_management.view.all'),
    ('settings_route.visible.settings.mcp-management', 'mcp_management.manage.all')
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
        ('settings_route.visible.settings.docs', 'settings_feature.access.system.docs'),
        ('settings_route.visible.settings.docs', 'api_reference.view.all'),
        ('settings_route.visible.settings.api-key-authentication', 'settings_feature.access.system.api-key-authentication'),
        ('settings_route.visible.settings.system-runtime', 'settings_feature.access.system.system-runtime'),
        ('settings_route.visible.settings.system-runtime', 'system_runtime.view.all'),
        ('settings_route.visible.settings.mcp-management', 'settings_feature.access.system.mcp-management'),
        ('settings_route.visible.settings.mcp-management', 'mcp_management.view.all'),
        ('settings_route.visible.settings.mcp-management', 'mcp_management.manage.all')
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
    raise exception 'final SettingsFeature grant migration verification failed';
  end if;
end;
$$;

delete from role_permissions grants
using permission_definitions definitions
where grants.permission_id = definitions.id
  and definitions.code in (
    'settings_route.visible.settings.docs',
    'settings_route.visible.settings.api-key-authentication',
    'settings_route.visible.settings.system-runtime',
    'settings_route.visible.settings.mcp-management'
  );

delete from permission_definitions
where code in (
  'settings_route.visible.settings.docs',
  'settings_route.visible.settings.api-key-authentication',
  'settings_route.visible.settings.system-runtime',
  'settings_route.visible.settings.mcp-management'
);
