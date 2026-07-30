use super::CoreConsoleDisplayText;

macro_rules! settings_feature {
    ($reference:expr, $msgid:expr) => {
        CoreConsoleDisplayText::referenced($reference, $msgid)
    };
}

macro_rules! policy {
    ($msgid:expr) => {
        CoreConsoleDisplayText::new($msgid)
    };
}

macro_rules! resource {
    ($msgid:expr) => {
        CoreConsoleDisplayText::new($msgid)
    };
}

/// Canonical English identities for the Core console display consumers migrated by D3-P4.
///
/// The compiled locale inventory keeps these identities for fail-closed registry validation. It
/// deliberately contains no translated value: request projections resolve translations from the
/// dynamic catalog and the resolver falls back to the English msgid.
pub(super) const TEXTS: &[CoreConsoleDisplayText] = &[
    settings_feature!("auto.api_documentation", "API documentation"),
    settings_feature!("auto.translation_catalog_title", "Language catalog"),
    settings_feature!("auto.api_key_authentication", "API key authentication"),
    settings_feature!("auto.system_runtime", "System runtime"),
    settings_feature!("auto.application_management", "Application management"),
    settings_feature!("auto.auth_center", "Authentication center"),
    settings_feature!("auto.data_source", "Data source"),
    settings_feature!("auto.file_management", "File management"),
    settings_feature!("auto.infrastructure", "Infrastructure"),
    settings_feature!("auto.memory_observation", "Memory observation"),
    settings_feature!("auto.user_management", "User management"),
    settings_feature!("auto.model_providers", "Model providers"),
    settings_feature!("auto.mcp_management", "MCP management"),
    settings_feature!("auto.permission_management", "Permission management"),
    settings_feature!(
        "console.policy_groups.settings.system.docs.description",
        "API documentation operations"
    ),
    settings_feature!(
        "auto.translation_catalog_description",
        "Root language catalog operations"
    ),
    settings_feature!(
        "console.policy_groups.settings.system.api-key-authentication.description",
        "API key authentication operations"
    ),
    settings_feature!(
        "console.policy_groups.settings.system.system-runtime.description",
        "System runtime operations"
    ),
    settings_feature!(
        "console.policy_groups.settings.system.applications.description",
        "Application management operations"
    ),
    settings_feature!(
        "console.policy_groups.settings.system.auth-center.description",
        "Authentication center operations"
    ),
    settings_feature!(
        "console.policy_groups.settings.system.data-models.description",
        "Data model and data source operations"
    ),
    settings_feature!(
        "console.policy_groups.settings.system.files.description",
        "File management operations"
    ),
    settings_feature!(
        "console.policy_groups.settings.system.host-infrastructure.description",
        "Host infrastructure operations"
    ),
    settings_feature!(
        "console.policy_groups.settings.system.memory-observation.description",
        "Memory observation operations"
    ),
    settings_feature!(
        "console.policy_groups.settings.system.members.description",
        "Member management operations"
    ),
    settings_feature!(
        "console.policy_groups.settings.system.model-providers.description",
        "Model provider operations"
    ),
    settings_feature!(
        "console.policy_groups.settings.system.mcp-management.description",
        "MCP management operations"
    ),
    settings_feature!(
        "console.policy_groups.settings.system.roles.description",
        "Role and permission operations"
    ),
    policy!("Signed-in console"),
    policy!("Console routes available to every signed-in user"),
    policy!("Agent Flow"),
    policy!("Registered Agent Flow operations outside system settings"),
    policy!("Data source utilities"),
    policy!("Registered data source operations outside system settings"),
    policy!("Frontend blocks"),
    policy!("Registered frontend block catalog operations"),
    policy!("JavaScript dependencies"),
    policy!("Registered JavaScript dependency operations"),
    policy!("Model provider utilities"),
    policy!("Registered model provider operations outside system settings"),
    policy!("Node contributions"),
    policy!("Registered node contribution catalog operations"),
    policy!("Plugins"),
    policy!("Registered plugin catalog and lifecycle operations"),
    policy!("Current workspace"),
    policy!("Registered operations for the current workspace"),
    policy!("Full access"),
    policy!("Grant every operation in this group"),
    policy!("Custom access"),
    policy!("Choose operations and row scopes individually"),
    policy!("Disabled"),
    policy!("Do not grant this operation"),
    policy!("Own records"),
    policy!("Allow records created by the current user"),
    policy!("Allow records in the current workspace"),
    resource!("Applications"),
    resource!("Applications in the current workspace"),
    resource!("Create"),
    resource!("Create an application"),
    resource!("View"),
    resource!("View an application"),
    resource!("Update"),
    resource!("Update an application"),
    resource!("Delete"),
    resource!("Delete an application"),
    resource!("Data source instances"),
    resource!("Configured data source instances in the current workspace"),
    resource!("View a data source instance"),
];
