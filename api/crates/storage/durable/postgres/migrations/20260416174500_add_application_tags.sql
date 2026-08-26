create table application_tags (
    id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    name text not null,
    normalized_name text not null,
    created_by uuid not null references users(id),
    updated_by uuid null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (workspace_id, normalized_name)
);

create index application_tags_workspace_updated_idx
    on application_tags (workspace_id, updated_at desc, id desc);

create table application_tag_bindings (
    application_id uuid not null references applications(id) on delete cascade,
    tag_id uuid not null references application_tags(id) on delete cascade,
    created_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    primary key (application_id, tag_id)
);

create index application_tag_bindings_tag_idx
    on application_tag_bindings (tag_id, application_id);
