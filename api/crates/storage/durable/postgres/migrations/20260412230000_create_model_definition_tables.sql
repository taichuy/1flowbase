create table model_definitions (
  id uuid primary key,
  code text not null unique,
  name text not null,
  status text not null,
  published_version bigint,
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now()
);

create table model_definition_versions (
  id uuid primary key,
  model_id uuid not null references model_definitions(id) on delete cascade,
  version bigint not null,
  payload jsonb not null,
  created_by uuid,
  created_at timestamptz not null default now(),
  unique(model_id, version)
);
