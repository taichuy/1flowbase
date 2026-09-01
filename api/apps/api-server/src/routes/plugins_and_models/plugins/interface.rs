use std::sync::Arc;

use control_plane::{
    plugin_management::{
        AssignPluginCommand, DeletePluginFamilyCommand, EnablePluginCommand,
        InstallCurrentNodePluginArtifactCommand, InstallPluginCommand,
        InstallResolvedOfficialPluginCommand, InstallUploadedPluginCommand,
        PluginManagementService, RefreshCurrentNodePluginArtifactCommand,
        RefreshPluginPackageCatalogProjectionCommand, SwitchPluginVersionCommand,
        UpgradeLatestPluginFamilyCommand,
    },
    ports::OfficialPluginSourcePort,
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;

use super::*;
use crate::{
    host_infrastructure::CacheStore,
    official_extension_catalog::OfficialExtensionCatalogSourcePort,
    provider_runtime::{ApiProviderRuntime, ApiRuntimeServices},
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError, ConsoleLocaleHints,
    },
};

pub(crate) enum PluginInterfaceInput {
    ListCatalog {
        query: PluginCatalogQuery,
        locale: ConsoleLocaleHints,
    },
    ListFamilies {
        query: PluginCatalogQuery,
        locale: ConsoleLocaleHints,
    },
    ListOfficial {
        query: OfficialPluginCatalogQuery,
        locale: ConsoleLocaleHints,
    },
    InstallPath(InstallPluginBody),
    InstallUploaded {
        file_name: String,
        package_bytes: Vec<u8>,
    },
    InstallOfficial(InstallOfficialPluginBody),
    RefreshCatalogProjection {
        installation_id: String,
    },
    RefreshArtifact {
        installation_id: String,
    },
    InstallArtifact {
        installation_id: String,
    },
    UpgradeLatest {
        provider_code: String,
        body: Option<UpgradeLatestPluginFamilyBody>,
    },
    SwitchVersion {
        provider_code: String,
        body: SwitchPluginVersionBody,
    },
    DeleteFamily {
        provider_code: String,
    },
    Enable {
        installation_id: String,
    },
    Assign {
        installation_id: String,
    },
    ListTasks,
    GetTask {
        task_id: String,
    },
    ModelFamilies {
        query: PluginCatalogQuery,
        locale: ConsoleLocaleHints,
    },
    ModelOfficial {
        query: OfficialPluginCatalogQuery,
        locale: ConsoleLocaleHints,
    },
    ModelInstallOfficial(InstallOfficialPluginBody),
    ModelInstallUploaded {
        file_name: String,
        package_bytes: Vec<u8>,
    },
    ModelRefreshArtifact {
        installation_id: String,
    },
    ModelInstallArtifact {
        installation_id: String,
    },
    ModelUpgradeLatest {
        provider_code: String,
        body: Option<UpgradeLatestPluginFamilyBody>,
    },
    ModelSwitchVersion {
        provider_code: String,
        body: SwitchPluginVersionBody,
    },
    ModelDeleteFamily {
        provider_code: String,
    },
    ModelGetTask {
        task_id: String,
    },
}

impl InterfaceContract for PluginInterfaceInput {
    const CONTRACT_ID: &'static str = "console-plugin-management-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum PluginInterfaceOutput {
    Catalog(PluginCatalogResponse),
    Families(PluginFamilyCatalogResponse),
    Official(OfficialPluginCatalogResponse),
    Installed(InstallPluginResponse),
    Projection(PluginCatalogProjectionResponse),
    Artifact(PluginArtifactInstanceResponse),
    Task(PluginTaskResponse),
    Tasks(Vec<PluginTaskResponse>),
}

impl InterfaceContract for PluginInterfaceOutput {
    const CONTRACT_ID: &'static str = "console-plugin-management-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct PluginInterfaceDependencies {
    pub(crate) store: MainDurableStore,
    pub(crate) provider_runtime: Arc<ApiRuntimeServices>,
    pub(crate) official_plugin_source: Arc<dyn OfficialPluginSourcePort>,
    pub(crate) official_catalog_source: Arc<dyn OfficialExtensionCatalogSourcePort>,
    pub(crate) cache_store: Arc<dyn CacheStore>,
    pub(crate) provider_install_root: String,
    pub(crate) api_node_id: String,
    pub(crate) bootstrap_workspace_id: uuid::Uuid,
    pub(crate) allow_uploaded_host_extensions: bool,
}

struct PluginInterfaceAdapter(PluginInterfaceDependencies);

impl PluginInterfaceAdapter {
    fn service(
        &self,
        actor: &domain::ActorContext,
        operation: &'static str,
    ) -> crate::app_state::ApiPluginManagementService {
        PluginManagementService::new(
            self.0.store.for_actor(actor.clone()),
            ApiProviderRuntime::new(self.0.provider_runtime.clone()),
            self.0.official_plugin_source.clone(),
            self.0.provider_install_root.clone(),
        )
        .with_node_id(self.0.api_node_id.clone())
        .with_allow_uploaded_host_extensions(self.0.allow_uploaded_host_extensions)
        .with_model_routing_cache_store(self.0.cache_store.clone())
        .for_plugin_console_operation(
            domain::ConsolePolicyGroup::other("other.plugins")
                .expect("compiled plugin policy group must be valid"),
            operation,
        )
    }

    fn model_service(
        &self,
        actor: &domain::ActorContext,
        operation: &'static str,
    ) -> crate::app_state::ApiPluginManagementService {
        PluginManagementService::new(
            self.0.store.for_actor(actor.clone()),
            ApiProviderRuntime::new(self.0.provider_runtime.clone()),
            self.0.official_plugin_source.clone(),
            self.0.provider_install_root.clone(),
        )
        .with_node_id(self.0.api_node_id.clone())
        .with_allow_uploaded_host_extensions(self.0.allow_uploaded_host_extensions)
        .with_model_routing_cache_store(self.0.cache_store.clone())
        .for_model_provider_console_operation(operation)
    }

    async fn preferred_locale(
        &self,
        principal: &UserPrincipal,
    ) -> Result<Option<String>, ApiError> {
        Ok(self
            .0
            .store
            .find_user_by_id(principal.actor().user_id)
            .await?
            .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?
            .preferred_locale)
    }

    async fn resolved_official_command(
        &self,
        actor_user_id: uuid::Uuid,
        workspace_id: uuid::Uuid,
        body: InstallOfficialPluginBody,
    ) -> Result<InstallResolvedOfficialPluginCommand, ApiError> {
        let mut cursor = None;
        let (entry, source_kind) = loop {
            let page = self
                .0
                .official_catalog_source
                .list_page_for_workspace(workspace_id, "runtime-extensions", cursor.as_deref())
                .await?;
            if let Some(entry) = page.entries.into_iter().find(|entry| {
                entry
                    .source
                    .metadata
                    .get("plugin_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(body.plugin_id.as_str())
            }) {
                break (entry, page.source_kind);
            }
            let Some(next) = page.metadata.next_cursor else {
                return Err(
                    control_plane::errors::ControlPlaneError::NotFound("official_plugin").into(),
                );
            };
            cursor = Some(next);
        };
        let plugin_type = entry
            .source
            .metadata
            .get("plugin_type")
            .and_then(serde_json::Value::as_str)
            .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                "official_plugin_type",
            ))?
            .to_string();
        let downloaded = self
            .0
            .official_catalog_source
            .download_artifact_for_workspace(workspace_id, &entry)
            .await?;
        let expected_checksum = downloaded.descriptor.expected_checksum.clone().ok_or(
            control_plane::errors::ControlPlaneError::InvalidInput("official_plugin_checksum"),
        )?;
        Ok(InstallResolvedOfficialPluginCommand {
            actor_user_id,
            plugin_id: body.plugin_id,
            plugin_type,
            minimum_host_version: entry.host_version_requirement,
            source_kind,
            file_name: downloaded.file_name,
            package_bytes: downloaded.artifact_bytes,
            expected_checksum,
            compatibility_override: to_compatibility_override(body.compatibility_override),
            risk_override: to_risk_override(body.risk_override),
        })
    }

    async fn official_response(
        &self,
        locale_meta: LocaleMetaResponse,
        catalog: control_plane::plugin_management::OfficialPluginCatalogView,
    ) -> Result<OfficialPluginCatalogResponse, ApiError> {
        let locale = domain::CatalogLocale::new(locale_meta.resolved_locale.clone())
            .expect("resolved locale is valid");
        let source_label = crate::app_state::resolve_official_source_label_with(
            &self.0.store,
            self.0.bootstrap_workspace_id,
            &locale,
            &catalog.source_kind,
            catalog.source_label,
        )
        .await?;
        Ok(OfficialPluginCatalogResponse {
            source_kind: catalog.source_kind,
            source_label,
            registry_url: catalog.registry_url,
            source_freshness: catalog.source_freshness,
            locale_meta,
            page: OfficialPluginCatalogPageResponse {
                limit: catalog.page.limit,
                next_cursor: catalog.page.next_cursor,
            },
            entries: catalog
                .entries
                .into_iter()
                .map(to_official_catalog_entry_response)
                .collect(),
        })
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: PluginInterfaceInput,
    ) -> Result<PluginInterfaceOutput, ApiError> {
        let actor = principal.actor();
        match input {
            PluginInterfaceInput::ListCatalog { query, locale } => {
                let locale_meta = locale.resolve_meta(
                    query.locale.clone(),
                    self.preferred_locale(principal).await?,
                );
                let catalog = self
                    .service(actor, "plugins.catalog.view")
                    .list_catalog(
                        actor.user_id,
                        filter_from_query(&query),
                        requested_locales(&locale_meta),
                    )
                    .await?;
                Ok(PluginInterfaceOutput::Catalog(PluginCatalogResponse {
                    locale_meta,
                    i18n_catalog: serde_json::to_value(catalog.i18n_catalog).unwrap(),
                    entries: catalog
                        .entries
                        .into_iter()
                        .map(to_catalog_response)
                        .collect(),
                }))
            }
            PluginInterfaceInput::ListFamilies { query, locale } => {
                let locale_meta = locale.resolve_meta(
                    query.locale.clone(),
                    self.preferred_locale(principal).await?,
                );
                let families = self
                    .service(actor, "plugins.families.view")
                    .list_families(
                        actor.user_id,
                        filter_from_query(&query),
                        requested_locales(&locale_meta),
                    )
                    .await?;
                Ok(PluginInterfaceOutput::Families(
                    PluginFamilyCatalogResponse {
                        locale_meta,
                        i18n_catalog: serde_json::to_value(families.i18n_catalog).unwrap(),
                        entries: families
                            .entries
                            .into_iter()
                            .map(to_family_response)
                            .collect(),
                    },
                ))
            }
            PluginInterfaceInput::ListOfficial { query, locale } => {
                let locale_meta = locale.resolve_meta(
                    query.locale.clone(),
                    self.preferred_locale(principal).await?,
                );
                let catalog = self
                    .service(actor, "plugins.official_catalog.view")
                    .list_official_catalog(
                        actor.user_id,
                        official_filter_from_query(&query),
                        requested_locales(&locale_meta),
                    )
                    .await?;
                Ok(PluginInterfaceOutput::Official(
                    self.official_response(locale_meta, catalog).await?,
                ))
            }
            PluginInterfaceInput::InstallPath(body) => {
                let result = self
                    .service(actor, "plugins.install")
                    .install_plugin(InstallPluginCommand {
                        actor_user_id: actor.user_id,
                        package_root: body.package_root,
                    })
                    .await?;
                Ok(PluginInterfaceOutput::Installed(to_install_response(
                    result,
                )))
            }
            PluginInterfaceInput::InstallUploaded {
                file_name,
                package_bytes,
            } => {
                let result = self
                    .service(actor, "plugins.install.upload")
                    .install_uploaded_plugin(InstallUploadedPluginCommand {
                        actor_user_id: actor.user_id,
                        file_name,
                        package_bytes,
                    })
                    .await?;
                Ok(PluginInterfaceOutput::Installed(to_install_response(
                    result,
                )))
            }
            PluginInterfaceInput::InstallOfficial(body) => {
                let command = self
                    .resolved_official_command(actor.user_id, actor.current_workspace_id, body)
                    .await?;
                let result = self
                    .service(actor, "plugins.install.official")
                    .install_resolved_official_plugin(command)
                    .await?;
                Ok(PluginInterfaceOutput::Installed(to_install_response(
                    result,
                )))
            }
            PluginInterfaceInput::RefreshCatalogProjection { installation_id } => Ok(
                PluginInterfaceOutput::Projection(to_catalog_projection_response(
                    self.service(actor, "plugins.catalog_projection.refresh")
                        .refresh_catalog_projection(RefreshPluginPackageCatalogProjectionCommand {
                            actor_user_id: actor.user_id,
                            installation_id: parse_uuid(&installation_id, "installation_id")?,
                        })
                        .await?,
                )),
            ),
            PluginInterfaceInput::RefreshArtifact { installation_id } => Ok(
                PluginInterfaceOutput::Artifact(to_artifact_instance_response(
                    self.service(actor, "plugins.artifact.refresh")
                        .refresh_current_node_artifact(RefreshCurrentNodePluginArtifactCommand {
                            actor_user_id: actor.user_id,
                            installation_id: parse_uuid(&installation_id, "installation_id")?,
                        })
                        .await?,
                )),
            ),
            PluginInterfaceInput::InstallArtifact { installation_id } => Ok(
                PluginInterfaceOutput::Artifact(to_artifact_instance_response(
                    self.service(actor, "plugins.artifact.install")
                        .install_current_node_artifact(InstallCurrentNodePluginArtifactCommand {
                            actor_user_id: actor.user_id,
                            installation_id: parse_uuid(&installation_id, "installation_id")?,
                        })
                        .await?,
                )),
            ),
            PluginInterfaceInput::UpgradeLatest {
                provider_code,
                body,
            } => {
                let compatibility_override = body.as_ref().and_then(|body| {
                    to_compatibility_override(body.compatibility_override.clone())
                });
                let risk_override = body.and_then(|body| to_risk_override(body.risk_override));
                let task = self
                    .service(actor, "plugins.families.upgrade")
                    .upgrade_latest(UpgradeLatestPluginFamilyCommand {
                        actor_user_id: actor.user_id,
                        provider_code,
                        compatibility_override,
                        risk_override,
                    })
                    .await?;
                Ok(PluginInterfaceOutput::Task(to_task_response(task)))
            }
            PluginInterfaceInput::SwitchVersion {
                provider_code,
                body,
            } => Ok(PluginInterfaceOutput::Task(to_task_response(
                self.service(actor, "plugins.families.switch")
                    .switch_version(SwitchPluginVersionCommand {
                        actor_user_id: actor.user_id,
                        provider_code,
                        target_installation_id: parse_uuid(
                            &body.installation_id,
                            "installation_id",
                        )?,
                    })
                    .await?,
            ))),
            PluginInterfaceInput::DeleteFamily { provider_code } => {
                Ok(PluginInterfaceOutput::Task(to_task_response(
                    self.service(actor, "plugins.families.delete")
                        .delete_family(DeletePluginFamilyCommand {
                            actor_user_id: actor.user_id,
                            provider_code,
                        })
                        .await?,
                )))
            }
            PluginInterfaceInput::Enable { installation_id } => {
                Ok(PluginInterfaceOutput::Task(to_task_response(
                    self.service(actor, "plugins.enable")
                        .enable_plugin(EnablePluginCommand {
                            actor_user_id: actor.user_id,
                            installation_id: parse_uuid(&installation_id, "installation_id")?,
                        })
                        .await?,
                )))
            }
            PluginInterfaceInput::Assign { installation_id } => {
                Ok(PluginInterfaceOutput::Task(to_task_response(
                    self.service(actor, "plugins.assign")
                        .assign_plugin(AssignPluginCommand {
                            actor_user_id: actor.user_id,
                            installation_id: parse_uuid(&installation_id, "installation_id")?,
                        })
                        .await?,
                )))
            }
            PluginInterfaceInput::ListTasks => Ok(PluginInterfaceOutput::Tasks(
                self.service(actor, "plugins.tasks.view")
                    .list_tasks(actor.user_id)
                    .await?
                    .into_iter()
                    .map(to_task_response)
                    .collect(),
            )),
            PluginInterfaceInput::GetTask { task_id } => {
                Ok(PluginInterfaceOutput::Task(to_task_response(
                    self.service(actor, "plugins.tasks.view")
                        .get_task(actor.user_id, parse_uuid(&task_id, "task_id")?)
                        .await?,
                )))
            }
            PluginInterfaceInput::ModelFamilies { mut query, locale } => {
                query.plugin_type = Some(settings_routes::MODEL_PROVIDER_PLUGIN_TYPE.to_string());
                let locale_meta = locale.resolve_meta(
                    query.locale.clone(),
                    self.preferred_locale(principal).await?,
                );
                let families = self
                    .model_service(actor, "model_provider_plugins.families.view")
                    .list_families(
                        actor.user_id,
                        filter_from_query(&query),
                        requested_locales(&locale_meta),
                    )
                    .await?;
                Ok(PluginInterfaceOutput::Families(
                    PluginFamilyCatalogResponse {
                        locale_meta,
                        i18n_catalog: serde_json::to_value(families.i18n_catalog)?,
                        entries: families
                            .entries
                            .into_iter()
                            .map(to_family_response)
                            .collect(),
                    },
                ))
            }
            PluginInterfaceInput::ModelOfficial { mut query, locale } => {
                query.plugin_type = Some(settings_routes::MODEL_PROVIDER_PLUGIN_TYPE.to_string());
                let locale_meta = locale.resolve_meta(
                    query.locale.clone(),
                    self.preferred_locale(principal).await?,
                );
                let local_catalog = self
                    .model_service(actor, "model_provider_plugins.official_catalog.view")
                    .list_catalog(
                        actor.user_id,
                        filter_from_query(&PluginCatalogQuery {
                            plugin_type: query.plugin_type.clone(),
                            locale: query.locale.clone(),
                        }),
                        requested_locales(&locale_meta),
                    )
                    .await?;
                let filter = official_filter_from_query(&query);
                let page = self
                    .0
                    .official_catalog_source
                    .search_for_workspace(
                        actor.current_workspace_id,
                        "runtime-extensions",
                        crate::official_extension_catalog::OfficialExtensionCatalogSearchQuery {
                            slot_code: Some(
                                settings_routes::MODEL_PROVIDER_PLUGIN_TYPE.to_string(),
                            ),
                            q: filter.search_query.clone(),
                            limit: filter.limit,
                            cursor: query.cursor.clone(),
                        },
                    )
                    .await?;
                let installed = local_catalog
                    .entries
                    .into_iter()
                    .map(|entry| {
                        (
                            settings_routes::model_provider_catalog_id(&entry.installation),
                            settings_routes::model_provider_catalog_install_status(
                                entry.local_artifact.artifact_status,
                                entry.assigned_to_current_workspace,
                            ),
                        )
                    })
                    .collect::<std::collections::HashMap<_, _>>();
                let entries = page
                    .entries
                    .into_iter()
                    .filter_map(
                        |entry| match settings_routes::project_model_provider_catalog_entry(
                            self.0.official_catalog_source.as_ref(),
                            entry,
                            &installed,
                        ) {
                            Ok(Some(entry)) => Some(Ok(entry)),
                            Ok(None) => None,
                            Err(error) => Some(Err(error)),
                        },
                    )
                    .collect::<anyhow::Result<Vec<_>>>()?;
                let locale = domain::CatalogLocale::new(locale_meta.resolved_locale.clone())
                    .expect("resolved locale is valid");
                let source_label = crate::app_state::resolve_official_source_label_with(
                    &self.0.store,
                    self.0.bootstrap_workspace_id,
                    &locale,
                    &page.source_kind,
                    page.source_kind.clone(),
                )
                .await?;
                Ok(PluginInterfaceOutput::Official(
                    OfficialPluginCatalogResponse {
                        source_kind: page.source_kind,
                        source_label,
                        registry_url: page.snapshot_locator,
                        source_freshness: "fresh".to_string(),
                        locale_meta,
                        page: OfficialPluginCatalogPageResponse {
                            limit: filter.limit,
                            next_cursor: page.next_cursor,
                        },
                        entries,
                    },
                ))
            }
            PluginInterfaceInput::ModelInstallOfficial(body) => {
                let command = self
                    .resolved_official_command(actor.user_id, actor.current_workspace_id, body)
                    .await?;
                let result = self
                    .model_service(actor, "model_provider_plugins.install.official")
                    .install_resolved_official_plugin(command)
                    .await?;
                Ok(PluginInterfaceOutput::Installed(to_install_response(
                    result,
                )))
            }
            PluginInterfaceInput::ModelInstallUploaded {
                file_name,
                package_bytes,
            } => {
                let result = self
                    .model_service(actor, "model_provider_plugins.install.upload")
                    .install_uploaded_model_provider(InstallUploadedPluginCommand {
                        actor_user_id: actor.user_id,
                        file_name,
                        package_bytes,
                    })
                    .await?;
                Ok(PluginInterfaceOutput::Installed(to_install_response(
                    result,
                )))
            }
            PluginInterfaceInput::ModelRefreshArtifact { installation_id } => Ok(
                PluginInterfaceOutput::Artifact(to_artifact_instance_response(
                    self.model_service(actor, "model_provider_plugins.artifact.refresh")
                        .refresh_current_node_artifact(RefreshCurrentNodePluginArtifactCommand {
                            actor_user_id: actor.user_id,
                            installation_id: parse_uuid(&installation_id, "installation_id")?,
                        })
                        .await?,
                )),
            ),
            PluginInterfaceInput::ModelInstallArtifact { installation_id } => Ok(
                PluginInterfaceOutput::Artifact(to_artifact_instance_response(
                    self.model_service(actor, "model_provider_plugins.artifact.install")
                        .install_current_node_artifact(InstallCurrentNodePluginArtifactCommand {
                            actor_user_id: actor.user_id,
                            installation_id: parse_uuid(&installation_id, "installation_id")?,
                        })
                        .await?,
                )),
            ),
            PluginInterfaceInput::ModelUpgradeLatest {
                provider_code,
                body,
            } => {
                let compatibility_override = body.as_ref().and_then(|body| {
                    to_compatibility_override(body.compatibility_override.clone())
                });
                let risk_override = body.and_then(|body| to_risk_override(body.risk_override));
                Ok(PluginInterfaceOutput::Task(to_task_response(
                    self.model_service(actor, "model_provider_plugins.families.upgrade")
                        .upgrade_latest(UpgradeLatestPluginFamilyCommand {
                            actor_user_id: actor.user_id,
                            provider_code,
                            compatibility_override,
                            risk_override,
                        })
                        .await?,
                )))
            }
            PluginInterfaceInput::ModelSwitchVersion {
                provider_code,
                body,
            } => Ok(PluginInterfaceOutput::Task(to_task_response(
                self.model_service(actor, "model_provider_plugins.families.switch")
                    .switch_version(SwitchPluginVersionCommand {
                        actor_user_id: actor.user_id,
                        provider_code,
                        target_installation_id: parse_uuid(
                            &body.installation_id,
                            "installation_id",
                        )?,
                    })
                    .await?,
            ))),
            PluginInterfaceInput::ModelDeleteFamily { provider_code } => {
                Ok(PluginInterfaceOutput::Task(to_task_response(
                    self.model_service(actor, "model_provider_plugins.families.delete")
                        .delete_family(DeletePluginFamilyCommand {
                            actor_user_id: actor.user_id,
                            provider_code,
                        })
                        .await?,
                )))
            }
            PluginInterfaceInput::ModelGetTask { task_id } => {
                Ok(PluginInterfaceOutput::Task(to_task_response(
                    self.model_service(actor, "model_provider_plugins.tasks.view")
                        .get_task(actor.user_id, parse_uuid(&task_id, "task_id")?)
                        .await?,
                )))
            }
        }
    }
}

impl ConsoleInterfacePort<PluginInterfaceInput, PluginInterfaceOutput> for PluginInterfaceAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: PluginInterfaceInput,
    ) -> ConsoleInterfaceFuture<'a, PluginInterfaceOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "plugins.catalog.view",
        binding_id: "http.console.plugins.catalog.v1",
        method: "GET",
        path: "/api/console/plugins/catalog",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "plugins.families.view",
        binding_id: "http.console.plugins.families.v1",
        method: "GET",
        path: "/api/console/plugins/families",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "plugins.official_catalog.view",
        binding_id: "http.console.plugins.official-catalog.v1",
        method: "GET",
        path: "/api/console/plugins/official-catalog",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "plugins.install",
        binding_id: "http.console.plugins.install-path.v1",
        method: "POST",
        path: "/api/console/plugins/install",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "plugins.install.upload",
        binding_id: "http.console.plugins.install-upload.v1",
        method: "POST",
        path: "/api/console/plugins/install-upload",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "plugins.install.official",
        binding_id: "http.console.plugins.install-official.v1",
        method: "POST",
        path: "/api/console/plugins/install-official",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "plugins.catalog_projection.refresh",
        binding_id: "http.console.plugins.catalog-projection-refresh.v1",
        method: "POST",
        path: "/api/console/plugins/:installation_id/catalog-projection/refresh",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "plugins.artifact.refresh",
        binding_id: "http.console.plugins.artifact-refresh.v1",
        method: "POST",
        path: "/api/console/plugins/:installation_id/artifact/refresh",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "plugins.artifact.install",
        binding_id: "http.console.plugins.artifact-install.v1",
        method: "POST",
        path: "/api/console/plugins/:installation_id/artifact/install-current-node",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "plugins.families.upgrade",
        binding_id: "http.console.plugins.family-upgrade.v1",
        method: "POST",
        path: "/api/console/plugins/families/:provider_code/upgrade-latest",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "plugins.families.switch",
        binding_id: "http.console.plugins.family-switch.v1",
        method: "POST",
        path: "/api/console/plugins/families/:provider_code/switch-version",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "plugins.families.delete",
        binding_id: "http.console.plugins.family-delete.v1",
        method: "DELETE",
        path: "/api/console/plugins/families/:provider_code",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "plugins.enable",
        binding_id: "http.console.plugins.enable.v1",
        method: "POST",
        path: "/api/console/plugins/:installation_id/enable",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "plugins.assign",
        binding_id: "http.console.plugins.assign.v1",
        method: "POST",
        path: "/api/console/plugins/:installation_id/assign",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "plugins.tasks.view",
        binding_id: "http.console.plugins.tasks.v1",
        method: "GET",
        path: "/api/console/plugins/tasks",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "plugins.tasks.view",
        binding_id: "http.console.plugins.task.v1",
        method: "GET",
        path: "/api/console/plugins/tasks/:task_id",
        mutating: false,
    },
    ConsoleInterfaceDeclaration { interface_id: "model_provider_plugins.families.view", binding_id: "http.console.model-provider-plugins.families.v1", method: "GET", path: "/api/console/settings/model-providers/plugins/families", mutating: false },
    ConsoleInterfaceDeclaration { interface_id: "model_provider_plugins.official_catalog.view", binding_id: "http.console.model-provider-plugins.official-catalog.v1", method: "GET", path: "/api/console/settings/model-providers/plugins/official-catalog", mutating: false },
    ConsoleInterfaceDeclaration { interface_id: "model_provider_plugins.install.official", binding_id: "http.console.model-provider-plugins.install-official.v1", method: "POST", path: "/api/console/settings/model-providers/plugins/install-official", mutating: true },
    ConsoleInterfaceDeclaration { interface_id: "model_provider_plugins.install.upload", binding_id: "http.console.model-provider-plugins.install-upload.v1", method: "POST", path: "/api/console/settings/model-providers/plugins/install-upload", mutating: true },
    ConsoleInterfaceDeclaration { interface_id: "model_provider_plugins.artifact.refresh", binding_id: "http.console.model-provider-plugins.artifact-refresh.v1", method: "POST", path: "/api/console/settings/model-providers/plugins/:installation_id/artifact/refresh", mutating: true },
    ConsoleInterfaceDeclaration { interface_id: "model_provider_plugins.artifact.install", binding_id: "http.console.model-provider-plugins.artifact-install.v1", method: "POST", path: "/api/console/settings/model-providers/plugins/:installation_id/artifact/install-current-node", mutating: true },
    ConsoleInterfaceDeclaration { interface_id: "model_provider_plugins.families.upgrade", binding_id: "http.console.model-provider-plugins.family-upgrade.v1", method: "POST", path: "/api/console/settings/model-providers/plugins/families/:provider_code/upgrade-latest", mutating: true },
    ConsoleInterfaceDeclaration { interface_id: "model_provider_plugins.families.switch", binding_id: "http.console.model-provider-plugins.family-switch.v1", method: "POST", path: "/api/console/settings/model-providers/plugins/families/:provider_code/switch-version", mutating: true },
    ConsoleInterfaceDeclaration { interface_id: "model_provider_plugins.families.delete", binding_id: "http.console.model-provider-plugins.family-delete.v1", method: "DELETE", path: "/api/console/settings/model-providers/plugins/families/:provider_code", mutating: true },
    ConsoleInterfaceDeclaration { interface_id: "model_provider_plugins.tasks.view", binding_id: "http.console.model-provider-plugins.task.v1", method: "GET", path: "/api/console/settings/model-providers/plugins/tasks/:task_id", mutating: false },
];

pub(crate) fn compile_registry(
    dependencies: PluginInterfaceDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-plugins",
        "graph:console-plugins-v1",
        DECLARATIONS,
        Arc::new(PluginInterfaceAdapter(dependencies)),
    )
}
