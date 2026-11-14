use std::collections::HashMap;

use uuid::Uuid;

use crate::{
    error_response::ApiError, official_extension_catalog::OfficialExtensionCatalogEntrySource,
};

use super::{
    compatibility_for_requirement, ExtensionCatalogGatewayEntryResponse,
    ExtensionCenterDependencies, InstalledCatalogJoin, McpExtensionTemplateInstanceResponse,
};

pub(crate) const BUILTIN_FRONTSTAGE_CATALOG_ID: &str = "mcp:1flowbase/frontstage_assistant";
pub(super) const BUILTIN_FRONTSTAGE_CURSOR: &str = "builtin:mcp:1flowbase/frontstage_assistant";

pub(super) fn builtin_frontstage_matches_query(
    package: &domain::McpBundlePackage,
    query: Option<&str>,
    slot_code: Option<&str>,
) -> bool {
    if slot_code.is_some() {
        return false;
    }
    let Some(query) = query.map(str::trim).filter(|query| !query.is_empty()) else {
        return true;
    };
    let query = query.to_lowercase();
    package.manifest.bundle_id.to_lowercase().contains(&query)
        || BUILTIN_FRONTSTAGE_CATALOG_ID
            .to_lowercase()
            .contains(&query)
        || package.instances.iter().any(|instance| {
            instance.name.to_lowercase().contains(&query)
                || instance
                    .description_short
                    .as_deref()
                    .is_some_and(|description| description.to_lowercase().contains(&query))
        })
}

pub(super) async fn builtin_frontstage_catalog_entry(
    dependencies: &ExtensionCenterDependencies,
    workspace_id: Uuid,
    installed: &HashMap<String, InstalledCatalogJoin>,
) -> Result<ExtensionCatalogGatewayEntryResponse, ApiError> {
    let package =
        crate::official_mcp_bundles::ApiOfficialMcpBundleRegistry::bundled_frontstage_assistant_package()?;
    let installation = installed.get(BUILTIN_FRONTSTAGE_CATALOG_ID);
    let mut mcp_instances = Vec::with_capacity(package.instances.len());
    for template in &package.instances {
        let current = control_plane::ports::McpManagementRepository::get_mcp_instance(
            &dependencies.store,
            workspace_id,
            &template.instance_id,
        )
        .await?;
        let workspace_status = match current {
            None => "missing",
            Some(instance)
                if instance.managed_by.as_ref().is_some_and(|source| {
                    source.organization == package.manifest.organization
                        && source.bundle_id == package.manifest.bundle_id
                        && source.bundle_version == package.manifest.bundle_version
                }) =>
            {
                "applied"
            }
            Some(_) => "modified",
        };
        mcp_instances.push(McpExtensionTemplateInstanceResponse {
            instance_id: template.instance_id.clone(),
            name: template.name.clone(),
            description_short: template.description_short.clone(),
            workspace_status: workspace_status.to_string(),
        });
    }
    let description = package
        .instances
        .iter()
        .filter_map(|instance| instance.description_short.as_deref())
        .collect::<Vec<_>>()
        .join("；");
    let host_version_requirement = format!(">={}", package.manifest.minimum_host_version);
    let bundle_version = package.manifest.bundle_version.clone();
    Ok(ExtensionCatalogGatewayEntryResponse {
        category: "mcp".to_string(),
        id: BUILTIN_FRONTSTAGE_CATALOG_ID.to_string(),
        name: package.manifest.bundle_id.clone(),
        organization: package.manifest.organization,
        artifact: package.manifest.bundle_id,
        version: bundle_version.clone(),
        description,
        host_version_requirement: host_version_requirement.clone(),
        slot_codes: Vec::new(),
        keywords: vec!["builtin".to_string(), "frontstage".to_string()],
        source: serde_json::to_value(OfficialExtensionCatalogEntrySource {
            kind: "builtin".to_string(),
            locator: "embedded://mcp/frontstage-assistant".to_string(),
            metadata: Default::default(),
        })
        .expect("typed built-in extension catalog source must serialize"),
        signature: None,
        checksum: None,
        download_locator: serde_json::json!({
            "kind": "builtin",
            "locator": "embedded://mcp/frontstage-assistant"
        }),
        catalog_page: 0,
        catalog_source: "builtin".to_string(),
        current_version: Some(bundle_version),
        installation_status: "installed".to_string(),
        artifact_kind: None,
        installation_source: Some("builtin".to_string()),
        extension_installation_id: installation.map(|value| value.installation_id.to_string()),
        builtin_template_id: Some(BUILTIN_FRONTSTAGE_CATALOG_ID.to_string()),
        trust: "official".to_string(),
        warnings: Vec::new(),
        compatibility: compatibility_for_requirement(&host_version_requirement),
        mcp_instances,
    })
}
