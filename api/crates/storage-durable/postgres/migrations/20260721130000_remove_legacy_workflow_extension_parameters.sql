-- Workflow extension request fields are derived from the published Workflow Start contract.
-- The legacy extension.parameters mapping is no longer part of ApplicationApiMappingConfig.
update application_api_mappings
set mapping_config = jsonb_set(
    mapping_config,
    '{extension}',
    (mapping_config -> 'extension') - 'parameters'
)
where mapping_config #> '{extension,parameters}' is not null;

update application_publication_versions
set mapping_snapshot = jsonb_set(
    mapping_snapshot,
    '{extension}',
    (mapping_snapshot -> 'extension') - 'parameters'
)
where mapping_snapshot #> '{extension,parameters}' is not null;
