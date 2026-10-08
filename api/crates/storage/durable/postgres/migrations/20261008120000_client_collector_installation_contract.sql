-- Client distributions reuse the installation root without becoming server runtime plugins.
-- Preserve the existing server/non-plugin branches and all historical records.
alter table extension_installations
    drop constraint extension_installations_plugin_contract_check,
    add constraint extension_installations_plugin_contract_check check (
        case when receipt ->> 'distribution_kind' = 'client_collector' then
            coalesce(
                category = 'runtime-extensions'
                and receipt #>> '{client_collector,distribution_kind}' = 'client_collector'
                and receipt #>> '{client_collector,schema_version}' = '1flowbase.client-collector/v1'
                and receipt #>> '{client_collector,execution_target}' = 'client'
                and receipt #>> '{client_collector,protocol_version}' = '1flowbase.agent-logs/v1'
                and receipt #>> '{client_collector,organization}' = organization
                and receipt #>> '{client_collector,artifact_id}' = artifact_id
                and receipt #>> '{client_collector,version}' = artifact_version
                and plugin_id is null
                and contract_version is null
                and protocol is null
                and desired_state is null
                and application_action = 'none',
                false
            )
        else (
            (
                category in ('capability-plugins', 'host-extensions', 'runtime-extensions')
                and plugin_id is not null
                and contract_version is not null
                and protocol is not null
                and verification_status is not null
                and desired_state is not null
            ) or (
                category not in ('capability-plugins', 'host-extensions', 'runtime-extensions')
                and plugin_id is null
                and contract_version is null
                and protocol is null
                and desired_state is null
            )
        ) end
    );
