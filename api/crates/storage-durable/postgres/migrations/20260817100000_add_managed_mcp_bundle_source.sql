alter table mcp_instances
    add column managed_bundle_organization text null,
    add column managed_bundle_id text null,
    add column managed_bundle_version text null;

alter table mcp_instances
    add constraint mcp_instances_managed_bundle_source_complete_ck
    check (
        (managed_bundle_organization is null
            and managed_bundle_id is null
            and managed_bundle_version is null)
        or
        (managed_bundle_organization is not null
            and managed_bundle_id is not null
            and managed_bundle_version is not null)
    );

alter table mcp_tools
    add column managed_bundle_organization text null,
    add column managed_bundle_id text null,
    add column managed_bundle_version text null;

alter table mcp_tools
    add constraint mcp_tools_managed_bundle_source_complete_ck
    check (
        (managed_bundle_organization is null
            and managed_bundle_id is null
            and managed_bundle_version is null)
        or
        (managed_bundle_organization is not null
            and managed_bundle_id is not null
            and managed_bundle_version is not null)
    );

create index mcp_instances_managed_bundle_idx
    on mcp_instances (
        workspace_id,
        managed_bundle_organization,
        managed_bundle_id
    )
    where managed_bundle_id is not null;

create index mcp_tools_managed_bundle_idx
    on mcp_tools (
        workspace_id,
        managed_bundle_organization,
        managed_bundle_id
    )
    where managed_bundle_id is not null;
