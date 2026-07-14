drop table if exists applications_console_policy_projection;

create temporary table applications_console_policy_projection on commit drop as
select
  role.id as role_id,
  coalesce(
    bool_or(definition.code = 'settings_feature.access.system.applications')
      filter (where definition.code is not null),
    false
  ) as has_feature,
  coalesce(
    bool_or(definition.code in ('application.create.own', 'application.create.all'))
      filter (where definition.code is not null),
    false
  ) as has_create,
  case
    when coalesce(bool_or(definition.code = 'application.view.all'), false) then 'scope_all'
    when coalesce(bool_or(definition.code = 'application.view.own'), false) then 'own'
    else 'disabled'
  end as view_scope,
  case
    when coalesce(bool_or(definition.code = 'application.edit.all'), false) then 'scope_all'
    when coalesce(bool_or(definition.code = 'application.edit.own'), false) then 'own'
    else 'disabled'
  end as update_scope,
  case
    when coalesce(bool_or(definition.code = 'application.delete.all'), false) then 'scope_all'
    when coalesce(bool_or(definition.code = 'application.delete.own'), false) then 'own'
    else 'disabled'
  end as delete_scope,
  coalesce(
    jsonb_agg(distinct definition.code order by definition.code)
      filter (
        where definition.code in (
          'settings_feature.access.system.applications',
          'application.create.own',
          'application.create.all',
          'application.view.own',
          'application.view.all',
          'application.edit.own',
          'application.edit.all',
          'application.delete.own',
          'application.delete.all'
        )
      ),
    '[]'::jsonb
  ) as source_grants
from roles role
left join role_permissions grant_row on grant_row.role_id = role.id
left join permission_definitions definition on definition.id = grant_row.permission_id
group by role.id;

alter table applications_console_policy_projection add column mode text;

update applications_console_policy_projection
set mode = case
  when has_feature
    and has_create
    and view_scope = 'scope_all'
    and update_scope = 'scope_all'
    and delete_scope = 'scope_all'
    then 'full'
  when jsonb_array_length(source_grants) = 0 then 'disabled'
  else 'custom'
end;

delete from role_console_operation_policies operation_policy
using role_console_group_policies group_policy
where operation_policy.group_policy_id = group_policy.id
  and group_policy.group_kind = 'settings_feature'
  and group_policy.group_id = 'system.applications';

insert into role_console_group_policies (
  id,
  role_id,
  group_kind,
  group_id,
  mode
)
select
  md5('role_console_group_policy:system.applications:' || role_id::text)::uuid,
  role_id,
  'settings_feature',
  'system.applications',
  mode
from applications_console_policy_projection
on conflict (role_id, group_kind, group_id) do update set
  mode = excluded.mode,
  updated_at = now();

with custom_operations as (
  select
    projection.role_id,
    'settings_feature.access.system.applications'::text as operation_id,
    'simple'::text as policy_kind,
    true as simple_enabled,
    null::text as row_scope
  from applications_console_policy_projection projection
  where projection.mode = 'custom' and projection.has_feature
  union all
  select
    projection.role_id,
    'applications.create',
    'simple',
    true,
    null::text
  from applications_console_policy_projection projection
  where projection.mode = 'custom' and projection.has_create
  union all
  select projection.role_id, 'applications.view', 'row', null, projection.view_scope
  from applications_console_policy_projection projection
  where projection.mode = 'custom' and projection.view_scope <> 'disabled'
  union all
  select projection.role_id, 'applications.update', 'row', null, projection.update_scope
  from applications_console_policy_projection projection
  where projection.mode = 'custom' and projection.update_scope <> 'disabled'
  union all
  select projection.role_id, 'applications.delete', 'row', null, projection.delete_scope
  from applications_console_policy_projection projection
  where projection.mode = 'custom' and projection.delete_scope <> 'disabled'
)
insert into role_console_operation_policies (
  id,
  role_id,
  group_policy_id,
  group_mode,
  operation_id,
  policy_kind,
  simple_enabled,
  row_scope
)
select
  md5('role_console_operation_policy:' || operation.role_id::text || ':' || operation.operation_id)::uuid,
  operation.role_id,
  group_policy.id,
  'custom',
  operation.operation_id,
  operation.policy_kind,
  operation.simple_enabled,
  operation.row_scope
from custom_operations operation
join role_console_group_policies group_policy
  on group_policy.role_id = operation.role_id
 and group_policy.group_kind = 'settings_feature'
 and group_policy.group_id = 'system.applications';

insert into role_console_policy_migration_ledger (
  id,
  role_id,
  source_contract,
  catalog_fingerprint,
  mapping_fingerprint,
  catalog_complete,
  source_grants,
  projected_policy,
  authorization_delta,
  status,
  applied_at
)
select
  md5('applications_console_policy_migration:' || role_id::text)::uuid,
  role_id,
  'applications-legacy/v1',
  'applications-crud+settings-feature/v1',
  'applications-known-grants/v1',
  true,
  source_grants,
  jsonb_build_object(
    'group_kind', 'settings_feature',
    'group_id', 'system.applications',
    'mode', mode
  ),
  '{"added": [], "removed": []}'::jsonb,
  'applied',
  now()
from applications_console_policy_projection
on conflict (role_id, source_contract, catalog_fingerprint, mapping_fingerprint) do update set
  source_grants = excluded.source_grants,
  projected_policy = excluded.projected_policy,
  authorization_delta = excluded.authorization_delta,
  status = excluded.status,
  applied_at = excluded.applied_at;

do $$
begin
  if exists (
    select 1
    from applications_console_policy_projection projection
    join role_console_group_policies group_policy
      on group_policy.role_id = projection.role_id
     and group_policy.group_kind = 'settings_feature'
     and group_policy.group_id = 'system.applications'
    where group_policy.mode <> projection.mode
  ) then
    raise exception 'applications console policy migration mode assertion failed';
  end if;

  if exists (
    select 1
    from role_console_operation_policies operation_policy
    join role_console_group_policies group_policy on group_policy.id = operation_policy.group_policy_id
    where group_policy.group_kind = 'settings_feature'
      and group_policy.group_id = 'system.applications'
      and (
        group_policy.mode <> 'custom'
        or operation_policy.operation_id not in (
          'settings_feature.access.system.applications',
          'applications.create',
          'applications.view',
          'applications.update',
          'applications.delete'
        )
      )
  ) then
    raise exception 'applications console policy migration operation assertion failed';
  end if;

  if exists (
    select 1
    from applications_console_policy_projection projection
    join role_console_group_policies group_policy
      on group_policy.role_id = projection.role_id
     and group_policy.group_kind = 'settings_feature'
     and group_policy.group_id = 'system.applications'
    where group_policy.mode = 'full'
      and not (
        projection.has_feature
        and projection.has_create
        and projection.view_scope = 'scope_all'
        and projection.update_scope = 'scope_all'
        and projection.delete_scope = 'scope_all'
      )
  ) then
    raise exception 'applications console policy migration expanded a non-full role';
  end if;
end;
$$;
