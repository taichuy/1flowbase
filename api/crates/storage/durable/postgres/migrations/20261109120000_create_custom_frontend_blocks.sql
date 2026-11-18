create table custom_frontend_blocks (
    id uuid primary key,
    workspace_id uuid not null,
    provider_code text not null,
    contribution_code text not null,
    title text not null check (length(trim(title)) > 0),
    source text not null check (length(trim(source)) > 0),
    language text not null check (language in ('jsx', 'tsx')),
    revision integer not null default 1 check (revision > 0),
    created_by uuid not null,
    updated_by uuid not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);
create index custom_frontend_blocks_workspace on custom_frontend_blocks(workspace_id, created_at, id);
