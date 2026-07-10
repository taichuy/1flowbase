create table mcp_instance_discovery_policies (
    id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    scope_id uuid generated always as (workspace_id) stored not null,
    instance_record_id uuid not null references mcp_instances(id) on delete cascade,
    list_default_limit integer not null default 50,
    list_max_depth integer not null default 3,
    list_regex_enabled boolean not null default false,
    list_regex_max_length integer not null default 128,
    list_return_fields jsonb not null default '["id","type","path","name","description_short","children_count","risk_level"]'::jsonb,
    created_by uuid not null references users(id),
    updated_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

insert into mcp_instance_discovery_policies (
    id,
    workspace_id,
    instance_record_id,
    list_default_limit,
    list_max_depth,
    list_regex_enabled,
    list_regex_max_length,
    list_return_fields,
    created_by,
    updated_by,
    created_at,
    updated_at
)
select
    gen_random_uuid(),
    instances.workspace_id,
    instances.id,
    configs.list_default_limit,
    configs.list_max_depth,
    configs.list_regex_enabled,
    configs.list_regex_max_length,
    configs.list_return_fields,
    configs.created_by,
    configs.updated_by,
    configs.created_at,
    configs.updated_at
from mcp_instances instances
join mcp_meta_tool_configs configs
  on configs.workspace_id = instances.workspace_id;

insert into mcp_instance_discovery_policies (
    id,
    workspace_id,
    instance_record_id,
    created_by,
    updated_by,
    created_at,
    updated_at
)
select
    gen_random_uuid(),
    instances.workspace_id,
    instances.id,
    instances.created_by,
    instances.updated_by,
    instances.created_at,
    instances.updated_at
from mcp_instances instances
where not exists (
    select 1
    from mcp_instance_discovery_policies policies
    where policies.instance_record_id = instances.id
);

create unique index mcp_instance_discovery_policies_instance_idx
    on mcp_instance_discovery_policies (instance_record_id);

create index mcp_instance_discovery_policies_scope_created_id_idx
    on mcp_instance_discovery_policies (scope_id, created_at, id);

drop table mcp_meta_tool_configs;
