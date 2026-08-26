-- Network egress providers are system/node scoped.  Keep every installed version so existing
-- provider instances remain pinned, but select exactly one ready artifact per provider family
-- for type projection and new proxy creation.
update extension_artifact_instances artifact
set is_current = false,
    checked_at = now()
from extension_installations installation
where installation.id = artifact.installation_id
  and installation.metadata_json ->> 'plugin_type' = 'network_egress_provider'
  and artifact.is_current;

with ranked_ready_artifacts as (
    select
        artifact.node_id,
        artifact.installation_id,
        row_number() over (
            partition by artifact.node_id, installation.artifact_id
            order by
                string_to_array(
                    trim(both '.' from regexp_replace(installation.artifact_version, '[^0-9]+', '.', 'g')),
                    '.'
                )::integer[] desc,
                (installation.artifact_version !~ '-') desc,
                installation.updated_at desc,
                installation.id desc
        ) as family_rank
    from extension_artifact_instances artifact
    join extension_installations installation on installation.id = artifact.installation_id
    where installation.metadata_json ->> 'plugin_type' = 'network_egress_provider'
      and artifact.artifact_status = 'ready'
)
update extension_artifact_instances artifact
set is_current = true,
    checked_at = now()
from ranked_ready_artifacts ranked
where artifact.node_id = ranked.node_id
  and artifact.installation_id = ranked.installation_id
  and ranked.family_rank = 1;
