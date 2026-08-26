create table if not exists mcp_instances (
    id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    instance_id text not null,
    name text not null,
    description_short text null,
    status text not null check (
        status in ('draft', 'enabled', 'disabled', 'archived')
    ),
    default_entry_path text not null default '/',
    created_by uuid not null references users(id),
    updated_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create unique index if not exists mcp_instances_workspace_instance_id_idx
    on mcp_instances (workspace_id, instance_id);

create index if not exists mcp_instances_workspace_status_idx
    on mcp_instances (workspace_id, status, updated_at desc, id desc);

create table if not exists mcp_groups (
    id uuid primary key,
    instance_record_id uuid not null references mcp_instances(id) on delete cascade,
    path text not null,
    display_name text not null,
    description_short text null,
    enabled boolean not null default true,
    sort_order integer not null default 0,
    created_by uuid not null references users(id),
    updated_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create unique index if not exists mcp_groups_instance_path_idx
    on mcp_groups (instance_record_id, path);

create index if not exists mcp_groups_instance_sort_idx
    on mcp_groups (instance_record_id, sort_order asc, path asc);

create table if not exists mcp_tools (
    id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    tool_id text not null,
    name text not null,
    short_description text not null,
    usage_description text null,
    full_description text not null,
    interface_id text not null,
    parameter_schema jsonb not null default '{}'::jsonb,
    result_schema jsonb not null default '{}'::jsonb,
    input_mapping jsonb not null default '{}'::jsonb,
    output_mapping jsonb not null default '{}'::jsonb,
    permission_code text null,
    risk_level text not null check (
        risk_level in ('low', 'medium', 'high', 'critical')
    ),
    audit_policy jsonb not null default '{}'::jsonb,
    des_id text not null,
    des_id_required boolean not null default true,
    status text not null check (
        status in ('draft', 'enabled', 'disabled', 'archived')
    ),
    revision integer not null default 1,
    created_by uuid not null references users(id),
    updated_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create unique index if not exists mcp_tools_workspace_tool_id_idx
    on mcp_tools (workspace_id, tool_id);

create index if not exists mcp_tools_workspace_status_idx
    on mcp_tools (workspace_id, status, updated_at desc, id desc);

create table if not exists mcp_tool_bindings (
    id uuid primary key,
    instance_record_id uuid not null references mcp_instances(id) on delete cascade,
    tool_record_id uuid not null references mcp_tools(id) on delete cascade,
    group_path text not null,
    display_alias text null,
    visible boolean not null default true,
    sort_order integer not null default 0,
    created_by uuid not null references users(id),
    updated_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create unique index if not exists mcp_tool_bindings_instance_path_tool_idx
    on mcp_tool_bindings (instance_record_id, group_path, tool_record_id);

create index if not exists mcp_tool_bindings_instance_sort_idx
    on mcp_tool_bindings (instance_record_id, group_path, sort_order asc, id asc);

create table if not exists mcp_meta_tool_configs (
    id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    list_default_limit integer not null default 50,
    list_max_depth integer not null default 3,
    list_regex_enabled boolean not null default false,
    list_regex_max_length integer not null default 128,
    list_return_fields jsonb not null default '["id","type","path","name","description_short","children_count","risk_level"]'::jsonb,
    get_include_mapping_summary boolean not null default true,
    get_include_interface_summary boolean not null default true,
    call_default_des_id_policy text not null default 'tool_config',
    call_high_risk_requires_des_id boolean not null default true,
    call_validation_error_format text not null default 'structured',
    created_by uuid not null references users(id),
    updated_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create unique index if not exists mcp_meta_tool_configs_workspace_idx
    on mcp_meta_tool_configs (workspace_id);
