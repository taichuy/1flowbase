create table if not exists mcp_upstream_connections (
    id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    name text not null,
    endpoint text not null,
    transport text not null check (transport = 'streamable_http'),
    auth_type text not null check (auth_type in ('none', 'bearer', 'custom_header')),
    custom_header_name text null,
    status text not null check (status in ('enabled', 'disabled')),
    last_connected_at timestamptz null,
    last_discovered_at timestamptz null,
    last_error text null,
    created_by uuid not null references users(id),
    updated_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint mcp_upstream_connections_custom_header_ck check (
        (auth_type = 'custom_header' and custom_header_name is not null)
        or (auth_type <> 'custom_header' and custom_header_name is null)
    )
);

create unique index if not exists mcp_upstream_connections_workspace_name_idx
    on mcp_upstream_connections (workspace_id, name);
create index if not exists mcp_upstream_connections_scope_updated_idx
    on mcp_upstream_connections (workspace_id, updated_at desc, id desc);

create table if not exists mcp_upstream_connection_secrets (
    upstream_connection_id uuid primary key references mcp_upstream_connections(id) on delete cascade,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    encrypted_secret_json jsonb not null,
    updated_by uuid not null references users(id),
    updated_at timestamptz not null default now()
);

create table if not exists mcp_upstream_tool_sources (
    id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    upstream_connection_id uuid not null references mcp_upstream_connections(id) on delete cascade,
    remote_tool_name text not null,
    description text null,
    input_schema jsonb not null default '{}'::jsonb,
    output_schema jsonb not null default '{}'::jsonb,
    schema_hash text not null,
    source_status text not null check (
        source_status in ('not_imported', 'imported', 'definition_changed', 'remote_missing')
    ),
    imported_tool_record_id uuid null references mcp_tools(id) on delete set null,
    discovered_at timestamptz not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create unique index if not exists mcp_upstream_tool_sources_identity_idx
    on mcp_upstream_tool_sources (upstream_connection_id, remote_tool_name);
create index if not exists mcp_upstream_tool_sources_scope_status_idx
    on mcp_upstream_tool_sources (workspace_id, source_status, remote_tool_name);

alter table mcp_tools add column if not exists execution_kind text;
alter table mcp_tools add column if not exists upstream_connection_id uuid null
    references mcp_upstream_connections(id) on delete restrict;
alter table mcp_tools add column if not exists remote_tool_name text null;
alter table mcp_tools add column if not exists source_schema_hash text null;
update mcp_tools set execution_kind = 'interface_wrapper' where execution_kind is null;
alter table mcp_tools alter column execution_kind set not null;
alter table mcp_tools alter column interface_id drop not null;
alter table mcp_tools add constraint mcp_tools_execution_target_ck check (
    (
        execution_kind = 'interface_wrapper'
        and interface_id is not null
        and upstream_connection_id is null
        and remote_tool_name is null
        and source_schema_hash is null
    )
    or (
        execution_kind = 'mcp_proxy'
        and interface_id is null
        and upstream_connection_id is not null
        and remote_tool_name is not null
        and source_schema_hash is not null
    )
);
create unique index if not exists mcp_tools_upstream_source_idx
    on mcp_tools (workspace_id, upstream_connection_id, remote_tool_name)
    where execution_kind = 'mcp_proxy';
