with managed_sources as (
    select distinct on (source.workspace_id, source.organization, source.bundle_id)
        source.workspace_id,
        source.organization,
        source.bundle_id,
        source.bundle_version,
        source.imported_by
    from (
        select
            workspace_id,
            managed_bundle_organization as organization,
            managed_bundle_id as bundle_id,
            managed_bundle_version as bundle_version,
            updated_by as imported_by,
            updated_at
        from mcp_instances
        where managed_bundle_id is not null

        union all

        select
            workspace_id,
            managed_bundle_organization as organization,
            managed_bundle_id as bundle_id,
            managed_bundle_version as bundle_version,
            updated_by as imported_by,
            updated_at
        from mcp_tools
        where managed_bundle_id is not null
    ) source
    order by source.workspace_id, source.organization, source.bundle_id, source.updated_at desc
)
insert into mcp_extension_bundle_imports (
    workspace_id,
    extension_installation_id,
    imported_by,
    result_status
)
select
    source.workspace_id,
    installation.id,
    source.imported_by,
    'completed'
from managed_sources source
join lateral (
    select candidate.id
    from extension_installations candidate
    where candidate.category = 'mcp'
      and candidate.organization = source.organization
      and candidate.artifact_id = source.bundle_id
    order by
        (candidate.artifact_version = source.bundle_version) desc,
        candidate.updated_at desc
    limit 1
) installation on true
on conflict (workspace_id, extension_installation_id) do nothing;
