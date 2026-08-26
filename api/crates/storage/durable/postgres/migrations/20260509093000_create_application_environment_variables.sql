create table if not exists application_environment_variables (
  application_id uuid not null references applications(id) on delete cascade,
  name text not null,
  value_type text not null,
  value_json jsonb not null,
  description text not null default '',
  created_by uuid not null,
  updated_by uuid not null,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  primary key (application_id, name),
  check (name ~ '^[A-Za-z][A-Za-z0-9]*$'),
  check (
    value_type in (
      'string',
      'number',
      'boolean',
      'object',
      'array[string]',
      'array[number]',
      'array[boolean]',
      'array[object]'
    )
  )
);
