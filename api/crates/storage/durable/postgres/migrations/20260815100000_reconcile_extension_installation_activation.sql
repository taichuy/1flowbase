-- Keep applied migration checksums immutable. This follow-up reconciles the
-- executable-plugin activation policy introduced after the unified lifecycle
-- migration had already shipped.
with executable_installations as (
    select
        installation.id,
        installation.category,
        exists (
            select 1
            from plugin_assignments assignment
            where assignment.installation_id = installation.id
        ) as is_assigned,
        exists (
            select 1
            from plugin_assignments assignment
            join extension_installations assigned
              on assigned.id = assignment.installation_id
            where assigned.category = installation.category
              and assigned.organization = installation.organization
              and assigned.artifact_id = installation.artifact_id
        ) as family_has_assignment,
        row_number() over (
            partition by
                installation.category,
                installation.organization,
                installation.artifact_id
            order by
                coalesce(artifact.checked_at, installation.updated_at) desc,
                installation.updated_at desc,
                installation.id desc
        ) as family_position
    from extension_installations installation
    left join lateral (
        select max(instance.checked_at) as checked_at
        from extension_artifact_instances instance
        where instance.installation_id = installation.id
    ) artifact on true
    where installation.plugin_id is not null
), desired_states as (
    select
        id,
        case
            when is_assigned or (not family_has_assignment and family_position = 1)
                then case category
                    when 'host-extensions' then 'pending_restart'
                    else 'active_requested'
                end
            else 'disabled'
        end as desired_state
    from executable_installations
)
update extension_installations installation
set desired_state = desired.desired_state,
    updated_at = now()
from desired_states desired
where installation.id = desired.id
  and installation.desired_state is distinct from desired.desired_state;

update extension_artifact_instances artifact
set availability_status = case installation.desired_state
        when 'disabled' then 'disabled'
        when 'pending_restart' then 'pending_restart'
        when 'active_requested' then case artifact.runtime_status
            when 'active' then 'available'
            when 'load_failed' then 'load_failed'
            else 'install_incomplete'
        end
    end
from extension_installations installation
where installation.id = artifact.installation_id
  and installation.plugin_id is not null;
