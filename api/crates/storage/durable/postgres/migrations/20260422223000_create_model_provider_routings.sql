create unique index model_provider_instances_workspace_provider_id_idx
    on model_provider_instances (workspace_id, provider_code, id);

create table model_provider_routings (
    workspace_id uuid not null references workspaces(id) on delete cascade,
    provider_code text not null,
    routing_mode text not null check (
        routing_mode in ('manual_primary')
    ),
    primary_instance_id uuid not null,
    created_by uuid not null references users(id),
    updated_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    primary key (workspace_id, provider_code),
    foreign key (workspace_id, provider_code, primary_instance_id)
        references model_provider_instances (workspace_id, provider_code, id)
        on delete cascade
);
