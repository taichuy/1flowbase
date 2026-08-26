create table workspace_console_settings_orders (
    workspace_id uuid primary key references workspaces(id) on delete cascade,
    revision bigint not null default 0 check (revision >= 0),
    updated_by uuid null references users(id) on delete set null,
    updated_at timestamptz not null default now()
);

create table workspace_console_settings_order_items (
    workspace_id uuid not null references workspace_console_settings_orders(workspace_id) on delete cascade,
    group_id text not null check (btrim(group_id) <> ''),
    position integer not null check (position >= 0),
    primary key (workspace_id, group_id),
    unique (workspace_id, position)
);
