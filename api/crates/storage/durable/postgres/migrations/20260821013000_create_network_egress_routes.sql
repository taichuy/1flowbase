alter table model_provider_instances
    add constraint model_provider_instances_workspace_id_id_key unique (workspace_id, id);

create table network_egress_routes (
    id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    consumer_kind text not null check (consumer_kind in ('github', 'model_provider', 'http_node')),
    consumer_reference uuid,
    pool_id uuid not null references network_egress_pools(id) on delete restrict,
    enabled boolean not null default true,
    failure_policy text not null check (failure_policy = 'block'),
    created_by uuid not null references users(id),
    updated_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    check (
        (consumer_kind in ('github', 'http_node') and consumer_reference is null)
        or consumer_kind = 'model_provider'
    ),
    foreign key (workspace_id, consumer_reference)
        references model_provider_instances (workspace_id, id)
        on delete cascade
);

create unique index network_egress_routes_workspace_default_selector_idx
    on network_egress_routes (workspace_id, consumer_kind)
    where consumer_reference is null;

create unique index network_egress_routes_workspace_exact_selector_idx
    on network_egress_routes (workspace_id, consumer_kind, consumer_reference)
    where consumer_reference is not null;
