use std::sync::Arc;

use control_plane::plugin_management::{
    DeletePluginFamilyCommand, DisablePluginCommand, EnablePluginCommand, ExtensionCatalogCategory,
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use uuid::Uuid;

use super::*;
use crate::routes::console_interface::{
    self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    ConsoleInterfaceTargetError,
};

pub(crate) enum ExtensionCenterInput {
    ListInstalled(LocalExtensionInventoryQuery),
    Select(Uuid),
    Enable(Uuid),
    Disable(Uuid),
    Delete(Uuid),
    ListCatalog {
        category: String,
        query: ExtensionCatalogGatewayQuery,
    },
    GetCatalog {
        category: String,
        catalog_id: String,
    },
    CheckUpdates(ExtensionUpdateCheckBody),
}

impl InterfaceContract for ExtensionCenterInput {
    const CONTRACT_ID: &'static str = "console-extension-center-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum ExtensionCenterOutput {
    Installed(LocalExtensionInventoryPageResponse),
    Installation(LocalExtensionInventoryEntryResponse),
    Task(PluginTaskResponse),
    Catalog(ExtensionCatalogGatewayPageResponse),
    CatalogEntry(ExtensionCatalogGatewayEntryResponse),
    Updates(ExtensionUpdateCheckResponse),
}

impl InterfaceContract for ExtensionCenterOutput {
    const CONTRACT_ID: &'static str = "console-extension-center-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct ExtensionCenterAdapter(ExtensionCenterDependencies);

impl ExtensionCenterAdapter {
    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: ExtensionCenterInput,
    ) -> Result<ExtensionCenterOutput, ApiError> {
        let actor = principal.actor();
        match input {
            ExtensionCenterInput::ListInstalled(query) => {
                let category = query
                    .category
                    .as_deref()
                    .map(|value| {
                        domain::ExtensionCategory::parse(value).ok_or(
                            control_plane::errors::ControlPlaneError::InvalidInput(
                                "extension_catalog_category",
                            ),
                        )
                    })
                    .transpose()?;
                let limit = query.limit.unwrap_or(20).clamp(1, 50);
                let mut families = extension_installation_service(&self.0)
                    .list_installed_families_for_node(&self.0.api_node_id)
                    .await?;
                if let Some(category) = category {
                    families.retain(|family| family.current.identity.category == category);
                }
                let (total_entries, next_cursor, page_entries) =
                    paginate_installed_families(families, query.cursor.as_deref(), limit);
                let mut entries = Vec::with_capacity(page_entries.len());
                for family in page_entries {
                    let status = workspace_application_status(
                        &self.0,
                        actor.current_workspace_id,
                        &family.current,
                    )
                    .await?;
                    let mut response = to_local_inventory_family_entry(family);
                    if let Some(installation) =
                        control_plane::ports::PluginRepository::get_installation(
                            &self.0.store,
                            Uuid::parse_str(&response.id).map_err(|_| {
                                control_plane::errors::ControlPlaneError::InvalidInput(
                                    "extension_installation_id",
                                )
                            })?,
                        )
                        .await?
                    {
                        response.desired_state =
                            Some(installation.desired_state.as_str().to_string());
                        if let Some(artifact) =
                            control_plane::ports::PluginRepository::get_artifact_instance(
                                &self.0.store,
                                &self.0.api_node_id,
                                installation.id,
                            )
                            .await?
                        {
                            response.availability_status =
                                Some(artifact.availability_status.as_str().to_string());
                            if is_runtime_uninstall_category(installation.category)
                                && artifact.artifact_status
                                    == domain::PluginArtifactInstanceStatus::Missing
                            {
                                response.status = "uninstalled".to_string();
                            }
                        }
                    }
                    for version in &mut response.installed_versions {
                        if let Some(decision) = control_plane::ports::ExtensionInstallationRepository::extension_deletion_decision(&self.0.store, &self.0.api_node_id, Uuid::parse_str(&version.id).map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("extension_installation_id"))?).await? {
                            version.deletable = decision.deletable;
                            version.delete_reasons = decision.reasons;
                        }
                    }
                    response.application_status = status.to_string();
                    entries.push(response);
                }
                Ok(ExtensionCenterOutput::Installed(
                    LocalExtensionInventoryPageResponse {
                        limit,
                        total_entries,
                        next_cursor,
                        entries,
                    },
                ))
            }
            ExtensionCenterInput::Select(installation_id) => {
                let installation = extension_installation_service(&self.0)
                    .select_current_installation(&self.0.api_node_id, installation_id)
                    .await?
                    .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                        "extension_installation",
                    ))?;
                Ok(ExtensionCenterOutput::Installation(
                    to_local_inventory_entry(installation),
                ))
            }
            ExtensionCenterInput::Enable(installation_id) => {
                let task = service(&self.0, actor, "extension_center.installed.enable")
                    .enable_plugin(EnablePluginCommand {
                        actor_user_id: actor.user_id,
                        installation_id,
                    })
                    .await?;
                Ok(ExtensionCenterOutput::Task(to_task_response(task)))
            }
            ExtensionCenterInput::Disable(installation_id) => {
                let installation = control_plane::ports::ExtensionInstallationRepository::find_extension_installation_by_id(&self.0.store, &self.0.api_node_id, installation_id).await?
                    .ok_or(control_plane::errors::ControlPlaneError::NotFound("extension_installation"))?;
                let task = service(&self.0, actor, "extension_center.installed.disable")
                    .disable_plugin(DisablePluginCommand {
                        actor_user_id: actor.user_id,
                        installation_id,
                    })
                    .await?;
                retain_managed_schema(&self.0, actor.current_workspace_id, &installation.identity)
                    .await?;
                Ok(ExtensionCenterOutput::Task(to_task_response(task)))
            }
            ExtensionCenterInput::Delete(installation_id) => {
                let existing = control_plane::ports::ExtensionInstallationRepository::find_extension_installation_by_id(&self.0.store, &self.0.api_node_id, installation_id).await?
                    .ok_or(control_plane::errors::ControlPlaneError::NotFound("extension_installation"))?;
                let managed_schema_identity = existing.identity.clone();
                let installation = if is_runtime_uninstall_category(existing.identity.category) {
                    let plugin = control_plane::ports::PluginRepository::get_installation(
                        &self.0.store,
                        installation_id,
                    )
                    .await?
                    .ok_or(
                        control_plane::errors::ControlPlaneError::NotFound("plugin_installation"),
                    )?;
                    service(&self.0, actor, "extension_center.installed.delete")
                        .delete_family(DeletePluginFamilyCommand {
                            actor_user_id: actor.user_id,
                            provider_code: plugin.provider_code,
                        })
                        .await?;
                    domain::ExtensionInstallationRecord {
                        local_path: None,
                        status: domain::ExtensionInstallationStatus::Missing,
                        is_current: false,
                        ..existing
                    }
                } else if existing.identity.category == domain::ExtensionCategory::Mcp {
                    self.0
                        .official_mcp_bundle_source
                        .delete_local_version(
                            &existing.identity.organization,
                            &existing.identity.artifact_id,
                            &existing.identity.version,
                        )
                        .await?;
                    domain::ExtensionInstallationRecord {
                        status: domain::ExtensionInstallationStatus::Missing,
                        is_current: false,
                        ..existing
                    }
                } else {
                    extension_installation_service(&self.0)
                        .delete_local_installation(&self.0.api_node_id, installation_id)
                        .await?
                        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                            "extension_installation",
                        ))?
                };
                retain_managed_schema(
                    &self.0,
                    actor.current_workspace_id,
                    &managed_schema_identity,
                )
                .await?;
                Ok(ExtensionCenterOutput::Installation(
                    to_local_inventory_entry(installation),
                ))
            }
            ExtensionCenterInput::ListCatalog { category, query } => {
                let category = ExtensionCatalogCategory::parse(&category)?;
                Ok(ExtensionCenterOutput::Catalog(
                    load_catalog_page(&self.0, actor.current_workspace_id, category, query).await?,
                ))
            }
            ExtensionCenterInput::GetCatalog {
                category,
                catalog_id,
            } => {
                let category = ExtensionCatalogCategory::parse(&category)?;
                let identity = catalog_identity(category, &catalog_id)?;
                if category == ExtensionCatalogCategory::Mcp
                    && catalog_id == BUILTIN_FRONTSTAGE_CATALOG_ID
                {
                    let installed = installed_catalog_joins(&self.0, category).await?;
                    return Ok(ExtensionCenterOutput::CatalogEntry(
                        builtin_frontstage_catalog_entry(
                            &self.0,
                            actor.current_workspace_id,
                            &installed,
                        )
                        .await?,
                    ));
                }
                let located = find_catalog_entry_for_requested_identity(
                    &self.0,
                    actor.current_workspace_id,
                    category,
                    &catalog_id,
                )
                .await?
                .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                    "extension_catalog_entry",
                ))?;
                if located.entry.category != category.as_str()
                    || located.entry.artifact != identity.artifact_id()
                    || located.entry.id != catalog_id
                    || located.entry.organization != identity.organization()
                {
                    return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                        "extension_catalog_identity",
                    )
                    .into());
                }
                let installed = installed_catalog_joins(&self.0, category).await?;
                let trusted_key_ids = self
                    .0
                    .official_plugin_source
                    .trusted_public_keys()
                    .iter()
                    .map(|key| key.key_id.clone())
                    .collect::<Vec<_>>();
                let catalog_source = if located.source_kind == "official_repository" {
                    "official"
                } else {
                    "mirror"
                };
                Ok(ExtensionCenterOutput::CatalogEntry(project_catalog_entry(
                    located.entry,
                    catalog_source,
                    &installed,
                    &trusted_key_ids,
                )))
            }
            ExtensionCenterInput::CheckUpdates(body) => {
                if body.items.len() > 50 {
                    return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                        "extension_update_check_page",
                    )
                    .into());
                }
                let category = ExtensionCatalogCategory::parse(&body.category)?;
                for item in &body.items {
                    catalog_identity(category, &item.catalog_id)?;
                    if !valid_extension_segment(&item.current_version)
                        || item.installed_versions.is_empty()
                        || item
                            .installed_versions
                            .iter()
                            .any(|version| !valid_extension_segment(version))
                    {
                        return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                            "installed_extension_versions",
                        )
                        .into());
                    }
                }
                let mut items = Vec::with_capacity(body.items.len());
                for item in body.items {
                    let latest_version = find_catalog_entry_for_requested_identity(
                        &self.0,
                        actor.current_workspace_id,
                        category,
                        &item.catalog_id,
                    )
                    .await?
                    .map(|located| located.entry.version);
                    let status = extension_update_status(
                        latest_version.as_deref(),
                        &item.installed_versions,
                    );
                    items.push(ExtensionUpdateCheckItemResponse {
                        catalog_id: item.catalog_id,
                        current_version: item.current_version,
                        latest_version,
                        status: status.to_string(),
                    });
                }
                Ok(ExtensionCenterOutput::Updates(
                    ExtensionUpdateCheckResponse {
                        category: body.category,
                        items,
                    },
                ))
            }
        }
    }
}

impl ConsoleInterfacePort<ExtensionCenterInput, ExtensionCenterOutput> for ExtensionCenterAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ExtensionCenterInput,
    ) -> ConsoleInterfaceFuture<'a, ExtensionCenterOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "extension_center.installed.view",
        binding_id: "http.console.extension-center.installed.v1",
        method: "GET",
        path: "/api/console/settings/extension-center/installed",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "extension_center.installed.select",
        binding_id: "http.console.extension-center.select.v1",
        method: "POST",
        path: "/api/console/settings/extension-center/installed/:installation_id/select",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "extension_center.installed.enable",
        binding_id: "http.console.extension-center.enable.v1",
        method: "POST",
        path: "/api/console/settings/extension-center/installed/:installation_id/enable",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "extension_center.installed.disable",
        binding_id: "http.console.extension-center.disable.v1",
        method: "POST",
        path: "/api/console/settings/extension-center/installed/:installation_id/disable",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "extension_center.installed.delete",
        binding_id: "http.console.extension-center.delete.v1",
        method: "DELETE",
        path: "/api/console/settings/extension-center/installed/:installation_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "extension_center.catalog.view",
        binding_id: "http.console.extension-center.catalog.v1",
        method: "GET",
        path: "/api/console/settings/extension-center/catalog/:category",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "extension_center.catalog.detail",
        binding_id: "http.console.extension-center.catalog-entry.v1",
        method: "GET",
        path: "/api/console/settings/extension-center/catalog/:category/:catalog_id",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "extension_center.update_check",
        binding_id: "http.console.extension-center.update-check.v1",
        method: "POST",
        path: "/api/console/settings/extension-center/update-check",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    dependencies: ExtensionCenterDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-extension-center",
        "graph:console-extension-center-v1",
        DECLARATIONS,
        Arc::new(ExtensionCenterAdapter(dependencies)),
    )
}
