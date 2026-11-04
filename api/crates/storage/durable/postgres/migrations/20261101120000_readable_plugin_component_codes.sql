-- One durable naming authority for installation projection and existing records.
-- Package versions are deliberately excluded; UUIDs and ownership remain unchanged.
create or replace function plugin_ui_component_code(
    publisher_namespace text, artifact_id text, contribution_code text
) returns text language sql immutable strict parallel safe as $$
    select lower(publisher_namespace || '.' || artifact_id || '.' ||
        trim(both '-' from regexp_replace(
            regexp_replace(
                regexp_replace(contribution_code, '([A-Z]+)([A-Z][a-z])', '\1-\2', 'g'),
                '([a-z0-9])([A-Z])', '\1-\2', 'g'),
            '[^a-zA-Z0-9]+', '-', 'g')))
$$;

-- Unique constraints reject normalization collisions atomically, including conflicts
-- with official/custom records. Never delete/recreate a component to rename its code.
update ui_component_records component
set component_code = plugin_ui_component_code(
    installation.organization, installation.artifact_id, component.owner_contribution_code)
from extension_installations installation
where component.origin = 'plugin'
    and installation.id = component.owner_installation_id
    and component.component_code is distinct from plugin_ui_component_code(
        installation.organization, installation.artifact_id, component.owner_contribution_code);
