-- Package identity is stable across versions; the installation owns the current projection.
alter table ui_component_records
    add column owner_plugin_id text,
    add column owner_contribution_code text,
    add column owner_installation_id uuid references extension_installations(id) on delete cascade,
    drop constraint ui_component_records_origin_check,
    drop constraint ui_component_records_required_text_check,
    drop constraint ui_component_records_version_check;

alter table ui_component_records
    add constraint ui_component_records_origin_check check (origin in ('official', 'custom', 'plugin')),
    add constraint ui_component_records_required_text_check check (
        length(trim(name)) > 0 and length(trim(description)) > 0
        and (origin = 'plugin' or length(trim(import_code)) > 0)
        and length(trim(source_code)) > 0 and length(trim(upstream_identity)) > 0
        and length(trim(upstream_version)) > 0
    ),
    add constraint ui_component_records_version_check check (
        version ~ '^[0-9]+\.[0-9]+\.[0-9]+$' or (origin = 'plugin' and length(trim(version)) > 0)
    ),
    add constraint ui_component_records_plugin_owner_check check (
        (origin = 'plugin' and owner_plugin_id is not null and length(trim(owner_plugin_id)) > 0
            and owner_contribution_code is not null and length(trim(owner_contribution_code)) > 0
            and owner_installation_id is not null)
        or (origin <> 'plugin' and owner_plugin_id is null
            and owner_contribution_code is null and owner_installation_id is null)
    );
create unique index ui_component_records_plugin_owner_identity_idx
    on ui_component_records(scope_id, owner_plugin_id, owner_contribution_code)
    where origin = 'plugin';
create index ui_component_records_owner_installation_idx
    on ui_component_records(owner_installation_id) where origin = 'plugin';
