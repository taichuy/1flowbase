-- Canonical plugin identity is artifact_id. Include every retained installation version,
-- including stopped/inactive versions; runtime publication alone cannot recover revokes.
-- Fully uninstalled plugins without retained metadata cannot be inferred safely.
alter table role_console_group_policies
    drop constraint role_console_group_policies_group_kind_check;
alter table role_console_group_policies add constraint role_console_group_policies_group_kind_check
    check (group_kind in ('settings_feature', 'other', 'plugin'));
alter table console_permission_catalog_groups
    drop constraint console_permission_catalog_groups_group_kind_check;
alter table console_permission_catalog_groups add constraint console_permission_catalog_groups_group_kind_check
    check (group_kind in ('settings_feature', 'other', 'plugin'));

create temporary table plugin_policy_group_mapping on commit drop as
select distinct metadata_json #>> '{managed_service,feature,feature_id}' as feature_id,
    artifact_id as plugin_code
from extension_installations
where metadata_json #>> '{managed_service,feature,feature_id}' is not null;

do $$
begin
    if exists (select 1 from plugin_policy_group_mapping
        where feature_id !~ '^[A-Za-z0-9][A-Za-z0-9._-]*$'
            or plugin_code !~ '^[A-Za-z0-9][A-Za-z0-9._-]*$')
        or exists (select feature_id from plugin_policy_group_mapping
            group by feature_id having count(distinct plugin_code) > 1)
        or exists (select plugin_code from plugin_policy_group_mapping
            group by plugin_code having count(distinct feature_id) > 1) then
        raise exception 'ambiguous or invalid historical plugin policy identity';
    end if;
    -- Do not merge grants: either side can contain explicit denials or retained custom state.
    if exists (select 1 from role_console_group_policies source
        join plugin_policy_group_mapping mapping on source.group_kind = 'settings_feature'
            and source.group_id = mapping.feature_id
        join role_console_group_policies target on target.role_id = source.role_id
            and target.group_kind = 'plugin' and target.group_id = mapping.plugin_code)
        or exists (select 1 from console_permission_catalog_groups source
            join plugin_policy_group_mapping mapping on source.group_kind = 'settings_feature'
                and source.group_id = mapping.feature_id
            join console_permission_catalog_groups target on target.group_kind = 'plugin'
                and target.group_id = mapping.plugin_code) then
        raise exception 'conflicting historical and canonical plugin policy groups';
    end if;
end $$;

-- Move first-seen identities instead of publishing a new group; operation IDs stay unchanged.
update console_permission_catalog_groups catalog
set group_kind = 'plugin', group_id = mapping.plugin_code
from plugin_policy_group_mapping mapping
where catalog.group_kind = 'settings_feature' and catalog.group_id = mapping.feature_id;
-- Old databases can have durable role policy without a catalog marker. Preserve that omission
-- as historical state so the next publication cannot interpret the move as a new permission.
insert into console_permission_catalog_groups (group_kind, group_id, first_seen_at)
select 'plugin', mapping.plugin_code, min(policy.created_at)
from role_console_group_policies policy
join plugin_policy_group_mapping mapping on policy.group_kind = 'settings_feature'
    and policy.group_id = mapping.feature_id
group by mapping.plugin_code
on conflict do nothing;

-- Preserve UUIDs, enabled, strategy and all operation foreign keys in place.
update role_console_group_policies policy
set group_kind = 'plugin', group_id = mapping.plugin_code
from plugin_policy_group_mapping mapping
where policy.group_kind = 'settings_feature' and policy.group_id = mapping.feature_id;
