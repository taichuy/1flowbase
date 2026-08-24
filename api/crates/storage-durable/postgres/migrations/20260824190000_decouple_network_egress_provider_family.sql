alter table network_egress_providers
    add column extension_category text,
    add column extension_organization text,
    add column extension_artifact_id text;

update network_egress_providers provider
set extension_category = installation.category,
    extension_organization = installation.organization,
    extension_artifact_id = installation.artifact_id
from extension_installations installation
where provider.installation_id = installation.id;

-- Earlier network-egress activation grouped only by artifact_id. If two publishers used the
-- same artifact name, only one family could remain current. Preserve every existing selection
-- and fill only the full catalog families that currently have none.
with ranked_missing_current as (
    select
        artifact.node_id,
        artifact.installation_id,
        row_number() over (
            partition by artifact.node_id, installation.category,
                         installation.organization, installation.artifact_id
            order by
                string_to_array(
                    trim(both '.' from regexp_replace(
                        installation.artifact_version,
                        '[^0-9]+',
                        '.',
                        'g'
                    )),
                    '.'
                )::integer[] desc,
                (installation.artifact_version !~ '-') desc,
                installation.updated_at desc,
                installation.id desc
        ) as family_rank
    from extension_artifact_instances artifact
    join extension_installations installation
      on installation.id = artifact.installation_id
    where installation.metadata_json ->> 'plugin_type' = 'network_egress_provider'
      and artifact.artifact_status = 'ready'
      and not exists (
          select 1
          from extension_artifact_instances selected_artifact
          join extension_installations selected_installation
            on selected_installation.id = selected_artifact.installation_id
          where selected_artifact.node_id = artifact.node_id
            and selected_artifact.is_current
            and selected_installation.category = installation.category
            and selected_installation.organization = installation.organization
            and selected_installation.artifact_id = installation.artifact_id
      )
)
update extension_artifact_instances artifact
set is_current = true,
    checked_at = now()
from ranked_missing_current ranked
where artifact.node_id = ranked.node_id
  and artifact.installation_id = ranked.installation_id
  and ranked.family_rank = 1;

do $$
begin
    if exists (
        select 1
        from network_egress_providers
        where provider_code <> 'builtin_static_http'
          and (
              extension_category is null
              or extension_organization is null
              or extension_artifact_id is null
          )
    ) then
        raise exception
            'network egress provider family migration could not resolve every extension-backed provider';
    end if;
end
$$;

alter table network_egress_providers
    add constraint network_egress_providers_extension_family_check check (
        (
            provider_code = 'builtin_static_http'
            and extension_category is null
            and extension_organization is null
            and extension_artifact_id is null
        )
        or
        (
            provider_code <> 'builtin_static_http'
            and extension_category = 'runtime-extensions'
            and extension_organization is not null
            and extension_organization <> ''
            and extension_artifact_id is not null
            and extension_artifact_id <> ''
        )
    );

create index network_egress_providers_extension_family_idx
    on network_egress_providers (
        extension_category,
        extension_organization,
        extension_artifact_id,
        id
    )
    where extension_category is not null;

alter table network_egress_providers
    drop constraint network_egress_providers_installation_id_fkey;

comment on column network_egress_providers.installation_id is
    'Migration-only legacy pointer; runtime and new writes use the stable extension family. Remove after deployed-family verification.';
