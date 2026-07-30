alter table application_api_mappings
    add column if not exists operation_bindings jsonb not null default '{}'::jsonb;

alter table application_publication_versions
    add column if not exists operation_bindings jsonb not null default '{}'::jsonb;
