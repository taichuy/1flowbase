alter table mcp_instances
    add column webmcp_exposure text not null default 'disabled';

alter table mcp_instances
    add constraint mcp_instances_webmcp_exposure_check
    check (webmcp_exposure in ('disabled', 'authenticated_session'));
