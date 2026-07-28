#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CoreConsolePolicyGroupSpec {
    SettingsFeature(&'static str),
    Other(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CoreConsoleAuthorizationSpec {
    Authenticated,
    Simple,
    ResourceAction {
        resource_code: &'static str,
        action_code: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CoreConsoleOperationSpec {
    pub(crate) operation_id: &'static str,
    pub(crate) policy_group: CoreConsolePolicyGroupSpec,
    pub(crate) authorization: CoreConsoleAuthorizationSpec,
}

const fn authenticated(
    operation_id: &'static str,
    other_group_id: &'static str,
) -> CoreConsoleOperationSpec {
    CoreConsoleOperationSpec {
        operation_id,
        policy_group: CoreConsolePolicyGroupSpec::Other(other_group_id),
        authorization: CoreConsoleAuthorizationSpec::Authenticated,
    }
}

const fn settings(
    operation_id: &'static str,
    feature_id: &'static str,
) -> CoreConsoleOperationSpec {
    CoreConsoleOperationSpec {
        operation_id,
        policy_group: CoreConsolePolicyGroupSpec::SettingsFeature(feature_id),
        authorization: CoreConsoleAuthorizationSpec::Simple,
    }
}

const fn other(operation_id: &'static str, group_id: &'static str) -> CoreConsoleOperationSpec {
    CoreConsoleOperationSpec {
        operation_id,
        policy_group: CoreConsolePolicyGroupSpec::Other(group_id),
        authorization: CoreConsoleAuthorizationSpec::Simple,
    }
}

const fn resource_action(
    operation_id: &'static str,
    feature_id: &'static str,
    resource_code: &'static str,
    action_code: &'static str,
) -> CoreConsoleOperationSpec {
    CoreConsoleOperationSpec {
        operation_id,
        policy_group: CoreConsolePolicyGroupSpec::SettingsFeature(feature_id),
        authorization: CoreConsoleAuthorizationSpec::ResourceAction {
            resource_code,
            action_code,
        },
    }
}

/// The Core's closed operation set. New route ownership fails compilation until it is declared
/// here with an explicit policy group and authorization contract.
pub(crate) static CORE_CONSOLE_OPERATION_SPECS: &[CoreConsoleOperationSpec] = &[
    authenticated("core.authenticated", "core.authenticated"),
    settings("applications.api.set_enabled", "system.applications"),
    settings("applications.create", "system.applications"),
    resource_action(
        "applications.delete",
        "system.applications",
        "applications",
        "delete",
    ),
    settings("applications.logs.export", "system.applications"),
    settings("applications.logs.import", "system.applications"),
    settings(
        "applications.orchestration.template.export",
        "system.applications",
    ),
    settings(
        "applications.orchestration.template.import",
        "system.applications",
    ),
    settings(
        "applications.orchestration.version.restore",
        "system.applications",
    ),
    settings("applications.publish", "system.applications"),
    settings("applications.run", "system.applications"),
    resource_action(
        "applications.update",
        "system.applications",
        "applications",
        "update",
    ),
    resource_action(
        "applications.view",
        "system.applications",
        "applications",
        "view",
    ),
    other("agent_flow.data_model_options.list", "other.agent-flow"),
    settings("auth_center.authenticators.copy", "system.auth-center"),
    settings("auth_center.authenticators.create", "system.auth-center"),
    settings("auth_center.authenticators.delete", "system.auth-center"),
    settings("auth_center.authenticators.enable", "system.auth-center"),
    settings("auth_center.authenticators.order", "system.auth-center"),
    settings("auth_center.authenticators.update", "system.auth-center"),
    settings("auth_center.overview.view", "system.auth-center"),
    settings("data_sources.create", "system.data-models"),
    settings("data_sources.defaults.update", "system.data-models"),
    settings("data_sources.discover", "system.data-models"),
    settings("data_sources.list", "system.data-models"),
    settings("data_sources.map_to_model", "system.data-models"),
    settings("data_sources.preview", "system.data-models"),
    other("data_sources.secret.rotate", "other.data-sources"),
    settings("data_sources.validate", "system.data-models"),
    resource_action(
        "data_sources.view",
        "system.data-models",
        "data_source_instances",
        "view",
    ),
    settings("file_storages.create", "system.files"),
    settings("file_storages.delete", "system.files"),
    settings("file_storages.list", "system.files"),
    settings("file_storages.update", "system.files"),
    settings("file_tables.create", "system.files"),
    settings("file_tables.delete", "system.files"),
    settings("file_tables.list", "system.files"),
    settings("file_tables.storage.bind", "system.files"),
    other("frontend_blocks.view", "other.frontend-blocks"),
    settings(
        "host_infrastructure.cache.domain.clear",
        "system.host-infrastructure",
    ),
    settings(
        "host_infrastructure.cache.entry.clear",
        "system.host-infrastructure",
    ),
    settings(
        "host_infrastructure.cache.reveal",
        "system.host-infrastructure",
    ),
    settings(
        "host_infrastructure.cache.view",
        "system.host-infrastructure",
    ),
    settings(
        "host_infrastructure.memory.reveal",
        "system.memory-observation",
    ),
    settings(
        "host_infrastructure.memory.view",
        "system.memory-observation",
    ),
    settings(
        "host_infrastructure.providers.configure",
        "system.host-infrastructure",
    ),
    settings(
        "host_infrastructure.providers.view",
        "system.host-infrastructure",
    ),
    settings("i18n_catalog.bundle.get", "system.i18n-catalog"),
    settings("i18n_catalog.state.get", "system.i18n-catalog"),
    settings("i18n_catalog.update.activate", "system.i18n-catalog"),
    settings("i18n_catalog.update.check", "system.i18n-catalog"),
    other("js_dependencies.view", "other.js-dependencies"),
    settings("mcp.bundles.export", "system.mcp-management"),
    settings("mcp.bundles.import", "system.mcp-management"),
    settings("mcp.bundles.official.list", "system.mcp-management"),
    settings("mcp.bundles.preview", "system.mcp-management"),
    settings("mcp.catalog.export", "system.mcp-management"),
    settings("mcp.catalog.view", "system.mcp-management"),
    settings("mcp.client_credential.delete", "system.mcp-management"),
    settings("mcp.client_credential.reveal", "system.mcp-management"),
    settings("mcp.client_credential.save", "system.mcp-management"),
    settings("mcp.debug.execute", "system.mcp-management"),
    settings("mcp.discovery_policy.update", "system.mcp-management"),
    settings("mcp.discovery_policy.view", "system.mcp-management"),
    settings("mcp.groups.delete", "system.mcp-management"),
    settings("mcp.groups.move", "system.mcp-management"),
    settings("mcp.groups.upsert", "system.mcp-management"),
    settings("mcp.instances.copy", "system.mcp-management"),
    settings("mcp.instances.create", "system.mcp-management"),
    settings("mcp.instances.delete", "system.mcp-management"),
    settings("mcp.instances.export", "system.mcp-management"),
    settings("mcp.instances.update", "system.mcp-management"),
    settings("mcp.instances.view", "system.mcp-management"),
    settings("mcp.tool_bindings.create", "system.mcp-management"),
    settings("mcp.tool_bindings.delete", "system.mcp-management"),
    settings("mcp.tool_bindings.update", "system.mcp-management"),
    settings("mcp.tools.create", "system.mcp-management"),
    settings("mcp.tools.delete", "system.mcp-management"),
    settings("mcp.tools.description.check", "system.mcp-management"),
    settings("mcp.tools.description.refresh", "system.mcp-management"),
    settings("mcp.tools.update", "system.mcp-management"),
    settings("mcp.tools.view", "system.mcp-management"),
    settings("mcp.upstream_connections.create", "system.mcp-management"),
    settings("mcp.upstream_connections.delete", "system.mcp-management"),
    settings("mcp.upstream_connections.discover", "system.mcp-management"),
    settings("mcp.upstream_connections.test", "system.mcp-management"),
    settings("mcp.upstream_connections.update", "system.mcp-management"),
    settings("mcp.upstream_connections.view", "system.mcp-management"),
    settings("mcp.upstream_credentials.delete", "system.mcp-management"),
    settings("mcp.upstream_credentials.update", "system.mcp-management"),
    settings("mcp.upstream_tools.debug", "system.mcp-management"),
    settings("mcp.upstream_tools.import", "system.mcp-management"),
    settings("members.create", "system.members"),
    settings("members.delete", "system.members"),
    settings("members.disable", "system.members"),
    settings("members.enable", "system.members"),
    settings("members.list", "system.members"),
    settings("members.password.reset", "system.members"),
    settings("members.role_options.list", "system.members"),
    settings("members.roles.replace", "system.members"),
    settings("members.update", "system.members"),
    settings("model_definitions.advisor.view", "system.data-models"),
    settings("model_definitions.create", "system.data-models"),
    settings("model_definitions.delete", "system.data-models"),
    settings("model_definitions.list", "system.data-models"),
    settings("model_definitions.openapi.view", "system.data-models"),
    settings("model_definitions.update", "system.data-models"),
    settings("model_fields.create", "system.data-models"),
    settings("model_fields.delete", "system.data-models"),
    settings("model_fields.update", "system.data-models"),
    settings(
        "model_provider_plugins.artifact.install",
        "system.model-providers",
    ),
    settings(
        "model_provider_plugins.artifact.refresh",
        "system.model-providers",
    ),
    settings(
        "model_provider_plugins.families.delete",
        "system.model-providers",
    ),
    settings(
        "model_provider_plugins.families.switch",
        "system.model-providers",
    ),
    settings(
        "model_provider_plugins.families.upgrade",
        "system.model-providers",
    ),
    settings(
        "model_provider_plugins.families.view",
        "system.model-providers",
    ),
    settings(
        "model_provider_plugins.install.official",
        "system.model-providers",
    ),
    settings(
        "model_provider_plugins.install.upload",
        "system.model-providers",
    ),
    settings(
        "model_provider_plugins.official_catalog.view",
        "system.model-providers",
    ),
    settings(
        "model_provider_plugins.tasks.view",
        "system.model-providers",
    ),
    other("model_providers.balance.view", "other.model-providers"),
    settings("model_providers.catalog.view", "system.model-providers"),
    other("model_providers.icons.view", "other.model-providers"),
    settings("model_providers.instances.create", "system.model-providers"),
    settings("model_providers.instances.delete", "system.model-providers"),
    settings(
        "model_providers.instances.models.refresh",
        "system.model-providers",
    ),
    settings(
        "model_providers.instances.models.view",
        "system.model-providers",
    ),
    settings(
        "model_providers.instances.secrets.reveal",
        "system.model-providers",
    ),
    settings("model_providers.instances.update", "system.model-providers"),
    settings(
        "model_providers.instances.validate",
        "system.model-providers",
    ),
    settings("model_providers.instances.view", "system.model-providers"),
    settings(
        "model_providers.main_instance.update",
        "system.model-providers",
    ),
    settings(
        "model_providers.main_instance.view",
        "system.model-providers",
    ),
    other("model_providers.options.view", "other.model-providers"),
    settings(
        "model_providers.settings_options.view",
        "system.model-providers",
    ),
    settings("model_providers.preview.view", "system.model-providers"),
    settings(
        "model_providers.request_logs.clear",
        "system.model-providers",
    ),
    settings(
        "model_providers.request_logs.delete",
        "system.model-providers",
    ),
    settings(
        "model_providers.request_logs.view",
        "system.model-providers",
    ),
    settings("model_scope_grants.create", "system.data-models"),
    settings("model_scope_grants.list", "system.data-models"),
    settings("model_scope_grants.update", "system.data-models"),
    other("node_contributions.view", "other.node-contributions"),
    other("plugins.artifact.install", "other.plugins"),
    other("plugins.artifact.refresh", "other.plugins"),
    other("plugins.assign", "other.plugins"),
    other("plugins.catalog.view", "other.plugins"),
    other("plugins.catalog_projection.refresh", "other.plugins"),
    other("plugins.enable", "other.plugins"),
    other("plugins.families.delete", "other.plugins"),
    other("plugins.families.switch", "other.plugins"),
    other("plugins.families.upgrade", "other.plugins"),
    other("plugins.families.view", "other.plugins"),
    other("plugins.install", "other.plugins"),
    other("plugins.install.official", "other.plugins"),
    other("plugins.install.upload", "other.plugins"),
    other("plugins.official_catalog.view", "other.plugins"),
    other("plugins.tasks.view", "other.plugins"),
    settings("roles.console_policy.replace", "system.roles"),
    settings("roles.console_policy.view", "system.roles"),
    settings("roles.console_policy_catalog.view", "system.roles"),
    settings("roles.create", "system.roles"),
    settings("roles.data_model_options.list", "system.roles"),
    settings("roles.data_policy.replace", "system.roles"),
    settings("roles.data_policy.view", "system.roles"),
    settings("roles.delete", "system.roles"),
    settings("roles.frontstage_routes.replace", "system.roles"),
    settings("roles.frontstage_routes.view", "system.roles"),
    settings("roles.list", "system.roles"),
    settings("roles.permission_options.list", "system.roles"),
    settings("roles.permissions.replace", "system.roles"),
    settings("roles.permissions.view", "system.roles"),
    settings("roles.update", "system.roles"),
    settings(
        "settings_feature.access.system.applications",
        "system.applications",
    ),
    settings(
        "settings_feature.access.system.data-models",
        "system.data-models",
    ),
    settings("settings_feature.access.system.docs", "system.docs"),
    settings("user_api_keys.manage", "system.api-key-authentication"),
    settings("system.release_status.view", "system.system-runtime"),
    settings("system.runtime_profile.view", "system.system-runtime"),
    other("workspace.update", "other.workspace"),
];
