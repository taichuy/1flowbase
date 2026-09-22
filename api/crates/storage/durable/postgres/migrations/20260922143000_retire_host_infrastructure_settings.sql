-- Retire the unused provider configuration surface. Preserve historical values for
-- recovery, but remove them from runtime storage and extension deletion decisions.
create table retired_host_infrastructure_settings (
    source_table text not null,
    record jsonb not null
);
comment on table retired_host_infrastructure_settings is
    'Read-only retirement archive: provider configuration and original observation policies. Not consumed by the application.';
insert into retired_host_infrastructure_settings
select 'host_infrastructure_provider_configs', to_jsonb(config)
from host_infrastructure_provider_configs config;
insert into retired_host_infrastructure_settings
select 'role_console_group_policies', to_jsonb(policy)
from role_console_group_policies policy
where group_kind = 'settings_feature'
  and group_id in ('system.host-infrastructure', 'system.memory-observation');
insert into retired_host_infrastructure_settings
select 'role_console_operation_policies', to_jsonb(operation)
from role_console_operation_policies operation
join role_console_group_policies policy on policy.id = operation.group_policy_id
where policy.group_kind = 'settings_feature'
  and policy.group_id in ('system.host-infrastructure', 'system.memory-observation');

-- Freeze the effective rights of each old group BEFORE combining them. A memory
-- full grant must not acquire cache deletion, nor may a cache grant reveal memory.
create temporary table retiring_observation_operations (
    operation_id text primary key,
    source_group text not null
) on commit drop;
insert into retiring_observation_operations values
    ('get_host_infrastructure_cache_overview', 'system.host-infrastructure'),
    ('list_host_infrastructure_cache_entries', 'system.host-infrastructure'),
    ('reveal_host_infrastructure_cache_entry', 'system.host-infrastructure'),
    ('clear_host_infrastructure_cache_entry', 'system.host-infrastructure'),
    ('clear_host_infrastructure_cache_domain', 'system.host-infrastructure'),
    ('get_host_infrastructure_memory_overview', 'system.memory-observation'),
    ('get_host_infrastructure_memory_stats_overview', 'system.memory-observation'),
    ('get_host_infrastructure_memory_stats', 'system.memory-observation'),
    ('list_host_infrastructure_memory_entries', 'system.memory-observation'),
    ('list_host_infrastructure_memory_tree', 'system.memory-observation'),
    ('search_host_infrastructure_memory_entries', 'system.memory-observation'),
    ('reveal_host_infrastructure_memory_entry', 'system.memory-observation');

create temporary table retiring_observation_rights on commit drop as
select roles.role_id, operations.operation_id,
       coalesce(policy.enabled and (
           policy.strategy = 'full' or coalesce(operation.simple_enabled, false)
       ), false) as allowed
from (
    select distinct role_id from role_console_group_policies
    where group_kind = 'settings_feature'
      and group_id in ('system.host-infrastructure', 'system.memory-observation')
) roles
cross join retiring_observation_operations operations
left join role_console_group_policies policy
  on policy.role_id = roles.role_id and policy.group_kind = 'settings_feature'
 and policy.group_id = operations.source_group
left join role_console_operation_policies operation
  on operation.group_policy_id = policy.id and operation.operation_id = operations.operation_id;

insert into role_console_group_policies (
    id, role_id, group_kind, group_id, enabled, strategy,
    created_by, created_at, updated_by, updated_at
)
select gen_random_uuid(), role_id, 'settings_feature', 'system.memory-observation',
       bool_or(enabled), 'custom', null, min(created_at), null, now()
from role_console_group_policies
where group_kind = 'settings_feature'
  and group_id in ('system.host-infrastructure', 'system.memory-observation')
group by role_id
on conflict (role_id, group_kind, group_id) do update
set enabled = excluded.enabled, strategy = 'custom', updated_at = now();

insert into role_console_operation_policies (
    id, role_id, group_policy_id, operation_id, policy_kind, simple_enabled, row_scope
)
select gen_random_uuid(), rights.role_id, policy.id, rights.operation_id,
       'simple', rights.allowed, null
from retiring_observation_rights rights
join role_console_group_policies policy
  on policy.role_id = rights.role_id and policy.group_kind = 'settings_feature'
 and policy.group_id = 'system.memory-observation'
on conflict (role_id, operation_id) do update
set group_policy_id = excluded.group_policy_id, policy_kind = 'simple',
    simple_enabled = excluded.simple_enabled, row_scope = null, updated_at = now();

delete from role_console_group_policies
where group_kind = 'settings_feature' and group_id = 'system.host-infrastructure';
delete from role_console_operation_policies
where operation_id in ('host_infrastructure.providers.view', 'list_host_infrastructure_providers',
    'save_host_infrastructure_provider_config', 'host_infrastructure.providers.configure');
delete from workspace_console_settings_order_items where group_id = 'system.host-infrastructure';
drop table host_infrastructure_provider_configs;
