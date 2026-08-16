alter table mcp_tools drop constraint if exists mcp_tools_execution_target_ck;
alter table mcp_tools add constraint mcp_tools_execution_target_ck check (
    (execution_kind = 'interface_wrapper' and interface_id is not null
        and upstream_connection_id is null and remote_tool_name is null
        and source_schema_hash is null and assistant_client_capability_code is null)
    or (execution_kind = 'mcp_proxy' and interface_id is null
        and upstream_connection_id is not null and remote_tool_name is not null
        and source_schema_hash is not null and assistant_client_capability_code is null)
    or (execution_kind = 'assistant_client' and interface_id is null
        and upstream_connection_id is null and remote_tool_name is null
        and source_schema_hash is null and assistant_client_capability_code in (
            'list_page_blocks',
            'inspect_block_render',
            'search_block_render',
            'read_block_render_fragment',
            'click_block_element',
            'recompile_block'
        ))
);
