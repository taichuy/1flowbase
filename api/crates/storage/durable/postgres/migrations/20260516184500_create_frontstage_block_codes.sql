create table if not exists frontstage_page_schemas (
  page_id uuid primary key references frontstage_pages(id) on delete cascade,
  workspace_id uuid not null references workspaces(id) on delete cascade,
  root_uid text not null,
  schema_payload jsonb not null,
  root_payload jsonb not null,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (workspace_id, page_id),
  unique (workspace_id, root_uid)
);

create index if not exists frontstage_page_schemas_workspace_idx
  on frontstage_page_schemas (workspace_id);

create table if not exists frontstage_block_codes (
  id uuid primary key,
  workspace_id uuid not null references workspaces(id) on delete cascade,
  page_id uuid not null references frontstage_pages(id) on delete cascade,
  code_ref text not null,
  code text not null,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (workspace_id, page_id, code_ref)
);

create index if not exists frontstage_block_codes_workspace_page_idx
  on frontstage_block_codes (workspace_id, page_id);
