alter table application_publication_versions
    drop column operation_bindings;

alter table application_api_mappings
    drop column operation_bindings;
