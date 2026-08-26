create table debug_variable_cache_entries (
    id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    application_id uuid not null references applications(id) on delete cascade,
    flow_draft_id uuid not null references flow_drafts(id) on delete cascade,
    actor_user_id uuid not null references users(id) on delete cascade,
    node_id text not null,
    variable_key text not null,
    value jsonb not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (
        application_id,
        flow_draft_id,
        actor_user_id,
        node_id,
        variable_key
    )
);

create index debug_variable_cache_entries_lookup_idx
    on debug_variable_cache_entries (
        application_id,
        flow_draft_id,
        actor_user_id,
        updated_at desc,
        id desc
    );
