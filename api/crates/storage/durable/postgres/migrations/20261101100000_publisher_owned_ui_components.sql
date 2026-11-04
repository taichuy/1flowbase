-- Backfill from the authoritative installation, not a publisher-free legacy owner string.
-- Preserve record UUIDs while correcting stable keys before the next install/upgrade upsert.
with owners as (
    select component.id,
           installation.organization || '/' || installation.artifact_id as plugin_owner,
           component.owner_contribution_code as contribution_code
    from ui_component_records component
    join extension_installations installation on installation.id = component.owner_installation_id
    where component.origin = 'plugin'
), identities as (
    select *,
           '[' || to_json(plugin_owner)::text || ',' || to_json(contribution_code)::text || ']' as identity_json
    from owners
)
update ui_component_records component
set owner_plugin_id = identities.plugin_owner,
    component_code = 'plugin.' || encode(sha256(convert_to(identities.identity_json, 'UTF8')), 'hex'),
    import_code = '// Plugin component: ' || identities.identity_json,
    upstream_identity = identities.plugin_owner || ':' || identities.contribution_code,
    keywords = array[identities.plugin_owner, identities.contribution_code]
from identities
where component.id = identities.id;

comment on column ui_component_records.owner_plugin_id is
    'Canonical unversioned publisher_namespace/artifact_id owner from extension_installations';
