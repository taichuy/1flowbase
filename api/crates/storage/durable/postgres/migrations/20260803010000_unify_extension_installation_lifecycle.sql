-- Issue #1566: replace the two writable installation roots with one logical root.
-- Runtime/plugin installation ids are preserved because they already own the broad FK graph.

do $$
begin
    if exists (
        select 1
        from plugin_installations plugin
        left join lateral (
            select count(distinct (category, organization)) as family_count
            from extension_installations inventory
            where inventory.artifact_id = plugin.provider_code
        ) family on true
        where coalesce(family.family_count, 0) <> 1
          and nullif(plugin.metadata_json ->> 'vendor', '') is null
          and plugin.source_kind <> 'builtin'
    ) then
        raise exception 'cannot uniquely map every plugin installation to an extension family';
    end if;
end
$$;

alter table extension_installations
    rename to extension_installation_node_inventory;
alter index extension_installations_pkey
    rename to extension_installation_node_inventory_pkey;
alter index extension_installations_identity_unique
    rename to extension_installation_node_inventory_identity_unique;
create table extension_installations (
    id uuid primary key,
    scope_id uuid not null default '00000000-0000-0000-0000-000000000000'::uuid,
    category text not null,
    organization text not null,
    artifact_id text not null,
    artifact_version text not null,
    plugin_id text,
    contract_version text,
    protocol text,
    display_name text not null,
    source_kind text not null,
    trust_level text not null,
    verification_status text,
    desired_state text,
    expected_checksum text,
    signature_status text not null,
    signature_algorithm text,
    signing_key_id text,
    warnings jsonb not null default '[]'::jsonb,
    receipt jsonb not null default '{}'::jsonb,
    application_action text not null default 'none',
    metadata_json jsonb not null default '{}'::jsonb,
    is_system_reserved boolean not null default false,
    created_by uuid not null references users(id) on delete restrict,
    updated_by uuid references users(id) on delete restrict,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint extension_installations_system_scope_check check (
        scope_id = '00000000-0000-0000-0000-000000000000'::uuid
    ),
    constraint extension_installations_category_check check (
        category in (
            'agent-flow', 'capability-plugins', 'host-extensions',
            'i18n', 'mcp', 'runtime-extensions'
        )
    ),
    constraint extension_installations_identity_values_check check (
        organization <> '' and artifact_id <> '' and artifact_version <> ''
    ),
    constraint extension_installations_source_kind_check check (
        source_kind in (
            'builtin', 'official_registry', 'mirror_registry', 'uploaded',
            'official_repository', 'configured_mirror', 'configured_proxy'
        )
    ),
    constraint extension_installations_trust_level_check check (
        trust_level in ('verified_official', 'checksum_only', 'unverified')
    ),
    constraint extension_installations_verification_status_check check (
        verification_status is null or verification_status in ('pending', 'valid', 'invalid')
    ),
    constraint extension_installations_desired_state_check check (
        desired_state is null or desired_state in ('disabled', 'pending_restart', 'active_requested')
    ),
    constraint extension_installations_signature_status_check check (
        signature_status in ('verified', 'missing', 'unknown_key', 'invalid')
    ),
    constraint extension_installations_application_action_check check (
        application_action in (
            'none', 'import_agent_flow', 'import_mcp',
            'activate_i18n', 'configure_model_provider'
        )
    ),
    constraint extension_installations_plugin_contract_check check (
        (
            category in ('capability-plugins', 'host-extensions', 'runtime-extensions')
            and plugin_id is not null
            and contract_version is not null
            and protocol is not null
            and verification_status is not null
            and desired_state is not null
        ) or (
            category not in ('capability-plugins', 'host-extensions', 'runtime-extensions')
            and plugin_id is null
            and contract_version is null
            and protocol is null
            and desired_state is null
        )
    ),
    constraint extension_installations_identity_unique unique (
        category, organization, artifact_id, artifact_version
    )
);

create unique index extension_installations_plugin_id_unique
    on extension_installations (plugin_id)
    where plugin_id is not null;
create index extension_installations_scope_created_id_idx
    on extension_installations (scope_id, created_at, id);
create index extension_installations_family_updated_idx
    on extension_installations (
        category, organization, artifact_id, updated_at desc, id desc
    );

create temporary table extension_installation_id_map (
    legacy_extension_installation_id uuid primary key,
    unified_extension_installation_id uuid not null
) on commit drop;

with plugin_family as (
    select
        plugin.id,
        plugin.provider_code,
        plugin.plugin_version,
        coalesce(
            (
                select min(inventory.category)
                from extension_installation_node_inventory inventory
                where inventory.artifact_id = plugin.provider_code
                having count(distinct (inventory.category, inventory.organization)) = 1
            ),
            case plugin.metadata_json ->> 'plugin_type'
                when 'capability_plugin' then 'capability-plugins'
                when 'host_extension' then 'host-extensions'
                else 'runtime-extensions'
            end
        ) as category,
        coalesce(
            (
                select min(inventory.organization)
                from extension_installation_node_inventory inventory
                where inventory.artifact_id = plugin.provider_code
                having count(distinct (inventory.category, inventory.organization)) = 1
            ),
            nullif(plugin.metadata_json ->> 'vendor', ''),
            case when plugin.source_kind = 'builtin' then '1flowbase' end
        ) as organization
    from plugin_installations plugin
)
insert into extension_installations (
    id, scope_id, category, organization, artifact_id, artifact_version,
    plugin_id, contract_version, protocol, display_name,
    source_kind, trust_level, verification_status, desired_state,
    expected_checksum, signature_status, signature_algorithm, signing_key_id,
    warnings, receipt, application_action, metadata_json, is_system_reserved,
    created_by, updated_by, created_at, updated_at
)
select
    plugin.id,
    plugin.scope_id,
    family.category,
    family.organization,
    plugin.provider_code,
    plugin.plugin_version,
    plugin.plugin_id,
    plugin.contract_version,
    plugin.protocol,
    plugin.display_name,
    plugin.source_kind,
    plugin.trust_level,
    plugin.verification_status,
    plugin.desired_state,
    plugin.checksum,
    case plugin.signature_status
        when 'verified' then 'verified'
        when 'builtin' then 'verified'
        when 'invalid' then 'invalid'
        when 'unknown_key' then 'unknown_key'
        else 'missing'
    end,
    plugin.signature_algorithm,
    plugin.signing_key_id,
    coalesce(projected.warnings, '[]'::jsonb),
    coalesce(projected.receipt, '{}'::jsonb) || jsonb_build_object(
        'legacy_plugin_installation_id', plugin.id,
        'migration', 'unified_extension_installation_lifecycle'
    ),
    coalesce(
        projected.application_action,
        case when family.category = 'runtime-extensions'
            and coalesce(plugin.metadata_json ->> 'plugin_type', 'model_provider') = 'model_provider'
            then 'configure_model_provider'
            else 'none'
        end
    ),
    plugin.metadata_json,
    plugin.source_kind = 'builtin',
    plugin.created_by,
    coalesce(plugin.updated_by, plugin.created_by),
    plugin.created_at,
    plugin.updated_at
from plugin_installations plugin
join plugin_family family on family.id = plugin.id
left join lateral (
    select inventory.warnings, inventory.receipt, inventory.application_action
    from extension_installation_node_inventory inventory
    where inventory.category = family.category
      and inventory.organization = family.organization
      and inventory.artifact_id = plugin.provider_code
      and inventory.artifact_version = plugin.plugin_version
    order by inventory.updated_at desc, inventory.id desc
    limit 1
) projected on true;

insert into extension_installation_id_map (
    legacy_extension_installation_id,
    unified_extension_installation_id
)
select inventory.id, unified.id
from extension_installation_node_inventory inventory
join extension_installations unified
  on unified.category = inventory.category
 and unified.organization = inventory.organization
 and unified.artifact_id = inventory.artifact_id
 and unified.artifact_version = inventory.artifact_version;

with canonical_non_plugin as (
    select distinct on (category, organization, artifact_id, artifact_version)
        inventory.*
    from extension_installation_node_inventory inventory
    where not exists (
        select 1
        from extension_installation_id_map mapped
        where mapped.legacy_extension_installation_id = inventory.id
    )
    order by
        category, organization, artifact_id, artifact_version,
        created_at, id
)
insert into extension_installations (
    id, category, organization, artifact_id, artifact_version,
    display_name, source_kind, trust_level, verification_status,
    expected_checksum, signature_status, signature_algorithm, signing_key_id,
    warnings, receipt, application_action, metadata_json, is_system_reserved,
    created_by, updated_by, created_at, updated_at
)
select
    inventory.id,
    inventory.category,
    inventory.organization,
    inventory.artifact_id,
    inventory.artifact_version,
    inventory.artifact_id,
    case inventory.source
        when 'official' then 'official_registry'
        when 'mirror' then 'mirror_registry'
        when 'upload' then 'uploaded'
        else inventory.source
    end,
    case inventory.trust
        when 'official' then 'verified_official'
        when 'trusted' then 'checksum_only'
        else 'unverified'
    end,
    case when inventory.signature_status = 'verified' then 'valid' else 'pending' end,
    inventory.checksum,
    inventory.signature_status,
    inventory.signature_algorithm,
    inventory.signing_key_id,
    inventory.warnings,
    inventory.receipt || jsonb_build_object(
        'migration', 'unified_extension_installation_lifecycle'
    ),
    inventory.application_action,
    '{}'::jsonb,
    inventory.source = 'builtin',
    inventory.installed_by,
    inventory.installed_by,
    inventory.created_at,
    inventory.updated_at
from canonical_non_plugin inventory;

insert into extension_installation_id_map (
    legacy_extension_installation_id,
    unified_extension_installation_id
)
select inventory.id, unified.id
from extension_installation_node_inventory inventory
join extension_installations unified
  on unified.category = inventory.category
 and unified.organization = inventory.organization
 and unified.artifact_id = inventory.artifact_id
 and unified.artifact_version = inventory.artifact_version
on conflict (legacy_extension_installation_id) do nothing;

alter table plugin_artifact_instances
    drop constraint plugin_artifact_instances_installation_id_fkey;
alter table plugin_artifact_instances
    rename to extension_artifact_instances;
alter table extension_artifact_instances
    rename column installed_path to local_path;
alter table extension_artifact_instances
    add column package_path text,
    add column manifest_fingerprint text,
    add column availability_status text not null default 'disabled',
    add column is_current boolean not null default false;

update extension_artifact_instances artifact
set package_path = plugin.package_path,
    manifest_fingerprint = plugin.manifest_fingerprint,
    availability_status = plugin.availability_status,
    is_current = exists (
        select 1
        from extension_installation_node_inventory inventory
        join extension_installations unified
          on unified.id = artifact.installation_id
         and unified.category = inventory.category
         and unified.organization = inventory.organization
         and unified.artifact_id = inventory.artifact_id
         and unified.artifact_version = inventory.artifact_version
        where inventory.node_id = artifact.node_id
          and inventory.is_current
    )
from plugin_installations plugin
where plugin.id = artifact.installation_id;

insert into extension_artifact_instances (
    node_id, installation_id, local_version, local_checksum, local_path,
    artifact_status, runtime_status, checked_at, last_error,
    package_path, manifest_fingerprint, availability_status, is_current
)
select
    inventory.node_id,
    mapped.unified_extension_installation_id,
    inventory.artifact_version,
    inventory.checksum,
    inventory.local_path,
    case inventory.status when 'installed' then 'ready' else 'missing' end,
    'inactive',
    inventory.updated_at,
    null,
    null,
    null,
    case inventory.status when 'installed' then 'available' else 'artifact_missing' end,
    inventory.is_current
from extension_installation_node_inventory inventory
join extension_installation_id_map mapped
  on mapped.legacy_extension_installation_id = inventory.id
on conflict (node_id, installation_id) do update
set local_version = coalesce(extension_artifact_instances.local_version, excluded.local_version),
    local_checksum = coalesce(extension_artifact_instances.local_checksum, excluded.local_checksum),
    local_path = coalesce(extension_artifact_instances.local_path, excluded.local_path),
    is_current = extension_artifact_instances.is_current or excluded.is_current;

-- Runtime/capability activation is workspace-scoped in plugin_assignments, not node-global.
update extension_artifact_instances artifact
set is_current = false
from extension_installations installation
where installation.id = artifact.installation_id
  and installation.plugin_id is not null;

alter table extension_artifact_instances
    add constraint extension_artifact_instances_installation_id_fkey
    foreign key (installation_id) references extension_installations(id) on delete cascade;
alter table extension_artifact_instances
    add constraint extension_artifact_instances_availability_status_check check (
        availability_status in (
            'disabled', 'pending_restart', 'artifact_missing',
            'install_incomplete', 'load_failed', 'available'
        )
    );

alter table extension_artifact_instances
    drop constraint plugin_artifact_instances_pkey,
    add constraint extension_artifact_instances_pkey primary key (node_id, installation_id);
alter table extension_artifact_instances
    drop constraint plugin_artifact_instances_artifact_status_check,
    add constraint extension_artifact_instances_artifact_status_check check (
        artifact_status in (
            'missing', 'ready', 'outdated', 'mismatched', 'corrupted', 'load_failed'
        )
    );
alter table extension_artifact_instances
    drop constraint plugin_artifact_instances_node_id_check,
    add constraint extension_artifact_instances_node_id_check check (btrim(node_id) <> '');
alter table extension_artifact_instances
    drop constraint plugin_artifact_instances_runtime_status_check,
    add constraint extension_artifact_instances_runtime_status_check check (
        runtime_status in ('inactive', 'active', 'load_failed')
    );
alter index plugin_artifact_instances_installation_id_idx
    rename to extension_artifact_instances_installation_id_idx;

alter table application_extension_sources
    drop constraint application_extension_sources_extension_installation_id_fkey;
alter table mcp_extension_bundle_imports
    drop constraint mcp_extension_bundle_imports_extension_installation_id_fkey;

update application_extension_sources source
set extension_installation_id = mapped.unified_extension_installation_id
from extension_installation_id_map mapped
where source.extension_installation_id = mapped.legacy_extension_installation_id;

update mcp_extension_bundle_imports imported
set extension_installation_id = mapped.unified_extension_installation_id
from extension_installation_id_map mapped
where imported.extension_installation_id = mapped.legacy_extension_installation_id;

alter table application_extension_sources
    add constraint application_extension_sources_extension_installation_id_fkey
    foreign key (extension_installation_id) references extension_installations(id) on delete restrict;
alter table mcp_extension_bundle_imports
    add constraint mcp_extension_bundle_imports_extension_installation_id_fkey
    foreign key (extension_installation_id) references extension_installations(id) on delete restrict;

alter table plugin_assignments drop constraint plugin_assignments_installation_id_fkey;
alter table plugin_tasks drop constraint plugin_tasks_installation_id_fkey;
alter table plugin_worker_leases drop constraint plugin_worker_leases_installation_id_fkey;
alter table model_provider_instances drop constraint model_provider_instances_installation_id_fkey;
alter table model_provider_preview_sessions drop constraint model_provider_preview_sessions_installation_id_fkey;
alter table data_source_instances drop constraint data_source_instances_installation_id_fkey;
alter table host_infrastructure_provider_configs drop constraint host_infrastructure_provider_configs_installation_id_fkey;
alter table node_contribution_registry drop constraint node_contribution_registry_installation_id_fkey;
alter table js_dependency_registry drop constraint js_dependency_registry_installation_id_fkey;
alter table application_js_dependency_selections drop constraint application_js_dependency_selections_installation_id_fkey;
alter table frontend_block_catalog drop constraint frontend_block_catalog_installation_id_fkey;
alter table plugin_package_catalog_projection drop constraint plugin_package_catalog_projection_installation_id_fkey;

alter table plugin_assignments
    add constraint plugin_assignments_installation_id_fkey
    foreign key (installation_id) references extension_installations(id) on delete cascade;
alter table plugin_tasks
    add constraint plugin_tasks_installation_id_fkey
    foreign key (installation_id) references extension_installations(id) on delete set null;
alter table plugin_worker_leases
    add constraint plugin_worker_leases_installation_id_fkey
    foreign key (installation_id) references extension_installations(id) on delete cascade;
alter table model_provider_instances
    add constraint model_provider_instances_installation_id_fkey
    foreign key (installation_id) references extension_installations(id) on delete restrict;
alter table model_provider_preview_sessions
    add constraint model_provider_preview_sessions_installation_id_fkey
    foreign key (installation_id) references extension_installations(id) on delete cascade;
alter table data_source_instances
    add constraint data_source_instances_installation_id_fkey
    foreign key (installation_id) references extension_installations(id) on delete cascade;
alter table host_infrastructure_provider_configs
    add constraint host_infrastructure_provider_configs_installation_id_fkey
    foreign key (installation_id) references extension_installations(id) on delete cascade;
alter table node_contribution_registry
    add constraint node_contribution_registry_installation_id_fkey
    foreign key (installation_id) references extension_installations(id) on delete cascade;
alter table js_dependency_registry
    add constraint js_dependency_registry_installation_id_fkey
    foreign key (installation_id) references extension_installations(id) on delete cascade;
alter table application_js_dependency_selections
    add constraint application_js_dependency_selections_installation_id_fkey
    foreign key (installation_id) references extension_installations(id) on delete restrict;
alter table frontend_block_catalog
    add constraint frontend_block_catalog_installation_id_fkey
    foreign key (installation_id) references extension_installations(id) on delete cascade;
alter table plugin_package_catalog_projection
    add constraint plugin_package_catalog_projection_installation_id_fkey
    foreign key (installation_id) references extension_installations(id) on delete cascade;

drop table plugin_installations;
drop table extension_installation_node_inventory;
