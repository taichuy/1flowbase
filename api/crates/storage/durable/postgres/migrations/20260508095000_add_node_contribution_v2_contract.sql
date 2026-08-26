alter table node_contribution_registry
    add column if not exists plugin_unique_identifier text,
    add column if not exists package_id text,
    add column if not exists contribution_checksum text,
    add column if not exists compiled_contribution_hash text,
    add column if not exists output_schema_snapshot jsonb,
    add column if not exists side_effect_policy text,
    add column if not exists infra_contracts jsonb not null default '[]'::jsonb;

delete from node_contribution_registry
where schema_version <> '1flowbase.node-contribution/v2'
    or nullif(trim(plugin_unique_identifier), '') is null
    or nullif(trim(package_id), '') is null
    or nullif(trim(contribution_checksum), '') is null
    or contribution_checksum = 'sha256:legacy'
    or nullif(trim(compiled_contribution_hash), '') is null
    or compiled_contribution_hash = 'sha256:legacy'
    or output_schema_snapshot is null
    or coalesce(jsonb_typeof(output_schema_snapshot), '') <> 'object'
    or coalesce(jsonb_typeof(output_schema_snapshot -> 'outputs'), '') <> 'array'
    or nullif(trim(side_effect_policy), '') is null
    or side_effect_policy not in ('none', 'external_read', 'external_write', 'durable_write')
    or coalesce(jsonb_typeof(infra_contracts), '') <> 'array';

alter table node_contribution_registry
    alter column plugin_unique_identifier set not null,
    alter column package_id set not null,
    alter column contribution_checksum set not null,
    alter column compiled_contribution_hash set not null,
    alter column output_schema_snapshot set not null,
    alter column side_effect_policy set not null;

alter table node_contribution_registry
    add constraint node_contribution_registry_schema_version_v2_check
        check (schema_version = '1flowbase.node-contribution/v2'),
    add constraint node_contribution_registry_side_effect_policy_check
        check (side_effect_policy in ('none', 'external_read', 'external_write', 'durable_write')),
    add constraint node_contribution_registry_hash_not_legacy_check
        check (
            contribution_checksum <> 'sha256:legacy'
            and compiled_contribution_hash <> 'sha256:legacy'
        ),
    add constraint node_contribution_registry_output_schema_snapshot_shape_check
        check (
            jsonb_typeof(output_schema_snapshot) = 'object'
            and jsonb_typeof(output_schema_snapshot -> 'outputs') = 'array'
        ),
    add constraint node_contribution_registry_infra_contracts_array_check
        check (jsonb_typeof(infra_contracts) = 'array');

create index if not exists node_contribution_registry_v2_identity_idx
    on node_contribution_registry (
        plugin_unique_identifier,
        package_id,
        plugin_version,
        contribution_code,
        contribution_checksum
    );
