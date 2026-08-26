alter table application_api_mappings
    drop column if exists operation_bindings;

alter table application_publication_versions
    drop column if exists operation_bindings;
