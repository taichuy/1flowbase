create table if not exists frontstage_pages (
  id uuid primary key,
  workspace_id uuid not null references workspaces(id) on delete cascade,
  parent_id uuid references frontstage_pages(id) on delete cascade,
  kind text not null check (kind in ('group', 'page')),
  title text,
  icon text,
  slug text,
  schema_root_uid text,
  rank text not null default '',
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  check (
    (kind = 'group' and schema_root_uid is null) or
    (kind = 'page' and schema_root_uid is not null)
  )
);

create index if not exists frontstage_pages_workspace_parent_rank_idx
  on frontstage_pages (workspace_id, parent_id, rank);

create index if not exists frontstage_pages_workspace_id_idx
  on frontstage_pages (workspace_id);
