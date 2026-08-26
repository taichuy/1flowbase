alter table application_publication_versions
    add column if not exists dependency_snapshot jsonb not null default '[]'::jsonb;
