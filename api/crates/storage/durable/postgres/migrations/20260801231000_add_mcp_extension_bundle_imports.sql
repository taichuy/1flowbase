create table mcp_extension_bundle_imports (
    workspace_id uuid not null references workspaces(id) on delete cascade,
    extension_installation_id uuid not null references extension_installations(id) on delete restrict,
    imported_by uuid not null references users(id) on delete restrict,
    result_status text not null,
    imported_at timestamptz not null default now(),
    primary key (workspace_id, extension_installation_id),
    constraint mcp_extension_bundle_imports_result_status_check check (
        result_status in ('completed', 'completed_with_warnings')
    )
);
