alter table mcp_extension_bundle_imports
    drop constraint mcp_extension_bundle_imports_result_status_check;

alter table mcp_extension_bundle_imports
    add constraint mcp_extension_bundle_imports_result_status_check check (
        result_status in ('completed', 'completed_with_warnings', 'already_applied')
    );
