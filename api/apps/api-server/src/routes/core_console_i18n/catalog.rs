use super::CoreConsoleDisplayText;

pub(super) const SETTINGS_MODULE: &str = "@taichuy/platform/console/settings";
pub(super) const POLICY_MODULE: &str = "@taichuy/platform/console/settings/policy";
pub(super) const RESOURCES_MODULE: &str = "@taichuy/platform/console/settings/resources";

macro_rules! settings {
    ($msgid:expr) => {
        CoreConsoleDisplayText::new(SETTINGS_MODULE, $msgid)
    };
}

macro_rules! policy {
    ($msgid:expr) => {
        CoreConsoleDisplayText::new(POLICY_MODULE, $msgid)
    };
}

macro_rules! resource {
    ($msgid:expr) => {
        CoreConsoleDisplayText::new(RESOURCES_MODULE, $msgid)
    };
}

/// Canonical English identities for the Core console display consumers migrated by D3-P4.
///
/// The compiled locale inventory keeps these identities for fail-closed registry validation. It
/// deliberately contains no translated value: request projections resolve translations from the
/// dynamic catalog and the resolver falls back to the English msgid.
pub(super) const TEXTS: &[CoreConsoleDisplayText] = &[
    settings!("API documentation"),
    settings!("Language catalog"),
    settings!("API key authentication"),
    settings!("System runtime"),
    settings!("Application management"),
    settings!("Authentication center"),
    settings!("Data source"),
    settings!("File management"),
    settings!("Infrastructure"),
    settings!("Memory observation"),
    settings!("User management"),
    settings!("Model providers"),
    settings!("MCP management"),
    settings!("Permission management"),
    settings!("API documentation operations"),
    settings!("Root language catalog operations"),
    settings!("API key authentication operations"),
    settings!("System runtime operations"),
    settings!("Application management operations"),
    settings!("Authentication center operations"),
    settings!("Data model and data source operations"),
    settings!("File management operations"),
    settings!("Host infrastructure operations"),
    settings!("Memory observation operations"),
    settings!("Member management operations"),
    settings!("Model provider operations"),
    settings!("MCP management operations"),
    settings!("Role and permission operations"),
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
