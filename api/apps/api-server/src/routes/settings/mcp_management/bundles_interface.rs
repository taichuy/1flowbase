use std::sync::Arc;

use control_plane::{
    errors::ControlPlaneError,
    mcp_bundle::{
        ExportMcpBundleCommand, ExportMcpInstanceBundleCommand, ImportMcpBundleCommand,
        McpInstanceBundleExportKind, PreviewMcpBundleCommand,
    },
    mcp_management::McpManagementService,
    plugin_management::{
        installed_extension_integrity_warnings, validate_extension_integrity_override,
        ExtensionInstallationService, ExtensionRiskOverride,
    },
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use uuid::Uuid;

use super::{
    bundles::{
        build_bundle_archive, parse_bundle_archive, BuiltinMcpTemplateImportResponse,
        BuiltinMcpTemplatePreviewResponse, BuiltinMcpTemplateSelector, ExportMcpBundleBody,
        ExportMcpInstanceBundleBody, InstalledMcpExtensionImportResponse,
        InstalledMcpExtensionIntegrityChallengeResponse, InstalledMcpExtensionPreviewResponse,
        InstalledMcpExtensionSelector, McpBundleExportDefaults, McpBundleImportSourceResponse,
        McpBundleLibraryVersionBody, McpBundlePreviewSourceResponse, McpBundleSourceBody,
        McpInstanceBundleExportProfile, OfficialMcpBundleSelector,
    },
    interface_catalog::{mcp_interface_catalog_entries_with, McpInterfaceCatalogDependencies},
};
use crate::{
    app_state::resolve_official_source_label_with,
    error_response::ApiError,
    official_extension_catalog::{
        OfficialExtensionCatalogEntry, OfficialExtensionCatalogPage,
        OfficialExtensionCatalogSourcePort,
    },
    official_mcp_bundles::OfficialMcpBundleSourcePort,
    routes::{
        console_interface::{
            self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
            ConsoleInterfaceTargetError, ConsoleLocaleHints,
        },
        plugins::extension_center::BUILTIN_FRONTSTAGE_CATALOG_ID,
    },
};

pub(crate) enum McpBundlesInput {
    ListOfficial {
        locale: ConsoleLocaleHints,
    },
    PreviewOfficial(McpBundleSourceBody),
    ImportOfficial(McpBundleSourceBody),
    Export(ExportMcpBundleBody),
    ExportDefaults,
    ExportInstance {
        instance_id: String,
        body: ExportMcpInstanceBundleBody,
    },
    PreviewUploaded {
        bytes: Vec<u8>,
    },
    ImportUploaded {
        bytes: Vec<u8>,
    },
    ListLibrary {
        refresh_remote: bool,
    },
    SyncLibrary {
        organization: String,
        bundle_id: String,
        body: McpBundleLibraryVersionBody,
    },
    PreviewLibrary {
        organization: String,
        bundle_id: String,
        body: McpBundleLibraryVersionBody,
    },
    ImportLibrary {
        organization: String,
        bundle_id: String,
        body: McpBundleLibraryVersionBody,
    },
    SwitchLibrary {
        organization: String,
        bundle_id: String,
        bundle_version: String,
    },
    DeleteLibraryRelease {
        organization: String,
        bundle_id: String,
        bundle_version: String,
    },
    RepairLibraryRelease {
        organization: String,
        bundle_id: String,
        bundle_version: String,
    },
}

impl InterfaceContract for McpBundlesInput {
    const CONTRACT_ID: &'static str = "console-mcp-bundles-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct BundleArchive {
    pub(crate) status: u16,
    pub(crate) content_type: &'static str,
    pub(crate) filename: String,
    pub(crate) bytes: Vec<u8>,
    pub(crate) headers: Vec<(String, String)>,
}

pub(crate) enum McpBundlesOutput {
    OfficialCatalog(crate::official_mcp_bundles::OfficialMcpBundleCatalogSnapshot),
    PreviewOfficial(McpBundlePreviewSourceResponse),
    ImportOfficial(McpBundleImportSourceResponse),
    IntegrityChallenge(InstalledMcpExtensionIntegrityChallengeResponse),
    Archive(BundleArchive),
    ExportDefaults(McpBundleExportDefaults),
    Preview(domain::McpBundlePreview),
    Import(domain::McpBundleImportReport),
    Library(crate::official_mcp_bundles::McpBundleLibraryCatalog),
    LibraryReceipt(crate::official_mcp_bundles::LocalMcpBundleReceipt),
    Deleted,
}

impl InterfaceContract for McpBundlesOutput {
    const CONTRACT_ID: &'static str = "console-mcp-bundles-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct McpBundlesDependencies {
    pub(crate) store: storage_durable_postgres::MainDurableStore,
    pub(crate) official_mcp_bundle_source: Arc<dyn OfficialMcpBundleSourcePort>,
    pub(crate) official_extension_catalog_source: Arc<dyn OfficialExtensionCatalogSourcePort>,
    pub(crate) provider_install_root: String,
    pub(crate) api_node_id: String,
    pub(crate) bootstrap_workspace_id: Uuid,
    pub(crate) interface_catalog: McpInterfaceCatalogDependencies,
}

struct McpBundlesAdapter(McpBundlesDependencies);

pub(crate) fn port(
    dependencies: McpBundlesDependencies,
) -> Arc<dyn ConsoleInterfacePort<McpBundlesInput, McpBundlesOutput>> {
    Arc::new(McpBundlesAdapter(dependencies))
}

#[derive(Clone, Copy)]
enum LoadedSourceKind {
    Official,
    Installed(Uuid),
    Builtin,
}

struct LoadedSource {
    kind: LoadedSourceKind,
    package: domain::McpBundlePackage,
    integrity_warnings: Vec<domain::ExtensionIntegrityWarning>,
}

impl McpBundlesAdapter {
    fn service(&self) -> McpManagementService<storage_durable_postgres::MainDurableStore> {
        McpManagementService::new(self.0.store.clone())
    }

    async fn authorize(&self, principal: &UserPrincipal) -> Result<(), ApiError> {
        self.service()
            .authorize_bundle_management(principal.actor().user_id)
            .await?;
        Ok(())
    }

    async fn catalog(
        &self,
        principal: &UserPrincipal,
    ) -> Result<Vec<domain::McpInterfaceCatalogEntry>, ApiError> {
        Ok(
            mcp_interface_catalog_entries_with(&self.0.interface_catalog, principal.actor())
                .await?,
        )
    }

    async fn parse(bytes: Vec<u8>) -> Result<domain::McpBundlePackage, ApiError> {
        if bytes.is_empty() || bytes.len() > 8 * 1024 * 1024 {
            return Err(ControlPlaneError::InvalidInput("mcp_bundle_file").into());
        }
        tokio::task::spawn_blocking(move || parse_bundle_archive(&bytes))
            .await
            .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_archive"))?
            .map_err(Into::into)
    }

    async fn load_source(
        &self,
        workspace_id: Uuid,
        body: McpBundleSourceBody,
    ) -> Result<LoadedSource, ApiError> {
        match body {
            McpBundleSourceBody::OfficialCatalog(OfficialMcpBundleSelector {
                organization,
                bundle_id,
            }) => {
                let catalog_id = format!("mcp:{organization}/{bundle_id}");
                let located = self
                    .0
                    .official_extension_catalog_source
                    .find_entry_for_workspace(workspace_id, "mcp", &catalog_id)
                    .await?
                    .ok_or(ControlPlaneError::NotFound("official_mcp_bundle"))?;
                if located.entry.organization != organization || located.entry.artifact != bundle_id
                {
                    return Err(ControlPlaneError::InvalidInput("official_mcp_bundle").into());
                }
                let downloaded = self
                    .0
                    .official_extension_catalog_source
                    .download_artifact_for_workspace(workspace_id, &located.entry)
                    .await?;
                Ok(LoadedSource {
                    kind: LoadedSourceKind::Official,
                    package: Self::parse(downloaded.artifact_bytes).await?,
                    integrity_warnings: Vec::new(),
                })
            }
            McpBundleSourceBody::InstalledExtension(InstalledMcpExtensionSelector {
                extension_installation_id,
                instance_id,
                ..
            }) => {
                let installation_id = Uuid::parse_str(&extension_installation_id)
                    .map_err(|_| ControlPlaneError::InvalidInput("extension_installation_id"))?;
                let installation = ExtensionInstallationService::new(
                    self.0.store.clone(),
                    &self.0.provider_install_root,
                )
                .find_local_installation_by_id(&self.0.api_node_id, installation_id)
                .await?
                .ok_or(ControlPlaneError::NotFound("extension_installation"))?;
                if installation.identity.category != domain::ExtensionCategory::Mcp {
                    return Err(ControlPlaneError::InvalidInput("extension_installation_id").into());
                }
                let path =
                    installation
                        .local_path
                        .as_deref()
                        .ok_or(ControlPlaneError::Conflict(
                            "extension_artifact_path_missing",
                        ))?;
                let bytes = tokio::fs::read(path).await?;
                let integrity_warnings =
                    installed_extension_integrity_warnings(&installation, &bytes);
                let package = Self::parse(bytes).await?;
                let package = match instance_id {
                    Some(instance_id) => package
                        .project_instance(&instance_id)
                        .ok_or(ControlPlaneError::NotFound("mcp_bundle_instance"))?,
                    None => package,
                };
                Ok(LoadedSource {
                    kind: LoadedSourceKind::Installed(installation_id),
                    package,
                    integrity_warnings,
                })
            }
            McpBundleSourceBody::BuiltinTemplate(BuiltinMcpTemplateSelector {
                builtin_template_id,
                instance_id,
            }) => {
                if builtin_template_id != BUILTIN_FRONTSTAGE_CATALOG_ID {
                    return Err(ControlPlaneError::NotFound("builtin_mcp_template").into());
                }
                let package = crate::official_mcp_bundles::ApiOfficialMcpBundleRegistry::bundled_frontstage_assistant_package()?
                    .project_instance(&instance_id).ok_or(ControlPlaneError::NotFound("mcp_bundle_instance"))?;
                Ok(LoadedSource {
                    kind: LoadedSourceKind::Builtin,
                    package,
                    integrity_warnings: Vec::new(),
                })
            }
        }
    }

    async fn archive(
        &self,
        package: domain::McpBundlePackage,
        filename: String,
        headers: Vec<(String, String)>,
    ) -> Result<BundleArchive, ApiError> {
        let bytes = tokio::task::spawn_blocking(move || build_bundle_archive(package))
            .await
            .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_archive"))??;
        Ok(BundleArchive {
            status: 200,
            content_type: "application/zip",
            filename,
            bytes,
            headers,
        })
    }

    async fn official_catalog(
        &self,
        principal: &UserPrincipal,
        hints: ConsoleLocaleHints,
    ) -> Result<crate::official_mcp_bundles::OfficialMcpBundleCatalogSnapshot, ApiError> {
        self.authorize(principal).await?;
        let actor = principal.actor();
        let first_page = self
            .0
            .official_extension_catalog_source
            .list_page_for_workspace(actor.current_workspace_id, "mcp", None)
            .await?;
        let mut next_cursor = first_page.metadata.next_cursor.clone();
        let mut pages = vec![first_page];
        while let Some(cursor) = next_cursor {
            let page = self
                .0
                .official_extension_catalog_source
                .list_page_for_workspace(actor.current_workspace_id, "mcp", Some(&cursor))
                .await?;
            next_cursor = page.metadata.next_cursor.clone();
            pages.push(page);
        }
        let mut catalog =
            project_official_catalog(self.0.official_extension_catalog_source.as_ref(), pages)?;
        let preferred_locale = self
            .0
            .store
            .find_user_by_id(actor.user_id)
            .await?
            .ok_or(ControlPlaneError::NotAuthenticated)?
            .preferred_locale;
        let locale = hints.resolve(preferred_locale);
        catalog.source.source_label = resolve_official_source_label_with(
            &self.0.store,
            self.0.bootstrap_workspace_id,
            &locale,
            &catalog.source.source_kind,
            catalog.source.source_label,
        )
        .await?;
        Ok(catalog)
    }

    async fn preview_source(
        &self,
        principal: &UserPrincipal,
        body: McpBundleSourceBody,
    ) -> Result<McpBundlePreviewSourceResponse, ApiError> {
        let actor = principal.actor();
        let source = self.load_source(actor.current_workspace_id, body).await?;
        let catalog = self.catalog(principal).await?;
        let preview = self
            .service()
            .preview_bundle(PreviewMcpBundleCommand {
                actor_user_id: actor.user_id,
                package: source.package,
                interface_catalog: catalog,
                current_system_version: current_version(),
            })
            .await?;
        Ok(match source.kind {
            LoadedSourceKind::Installed(id) => {
                let imported = self
                    .service()
                    .extension_bundle_is_imported(actor.user_id, id)
                    .await?;
                let status = if preview.effect_summary.changes > 0 {
                    "ready_to_import"
                } else if imported {
                    "imported"
                } else {
                    "already_present"
                };
                McpBundlePreviewSourceResponse::InstalledExtension(
                    InstalledMcpExtensionPreviewResponse {
                        extension_installation_id: id.to_string(),
                        artifact_installation_status: "installed".into(),
                        workspace_application_status: status.into(),
                        required_integrity_override: (!source.integrity_warnings.is_empty()).then(
                            || domain::ExtensionRiskChallenge {
                                warnings: source.integrity_warnings.clone(),
                                compatibility: None,
                            },
                        ),
                        integrity_warnings: source.integrity_warnings,
                        preview,
                    },
                )
            }
            LoadedSourceKind::Builtin => {
                McpBundlePreviewSourceResponse::BuiltinTemplate(BuiltinMcpTemplatePreviewResponse {
                    builtin_template_id: BUILTIN_FRONTSTAGE_CATALOG_ID.into(),
                    workspace_application_status: if preview.effect_summary.changes > 0 {
                        "ready_to_import"
                    } else {
                        "already_present"
                    }
                    .into(),
                    preview,
                })
            }
            LoadedSourceKind::Official => McpBundlePreviewSourceResponse::OfficialCatalog(preview),
        })
    }

    async fn import_source(
        &self,
        principal: &UserPrincipal,
        body: McpBundleSourceBody,
    ) -> Result<McpBundlesOutput, ApiError> {
        let actor = principal.actor();
        let integrity_override = match &body {
            McpBundleSourceBody::InstalledExtension(selector) => selector
                .integrity_override
                .as_ref()
                .map(|value| ExtensionRiskOverride {
                    reason: value.reason.clone(),
                    acknowledged_warnings: value.acknowledged_warnings.clone(),
                }),
            _ => None,
        };
        let catalog = self.catalog(principal).await?;
        let source = self.load_source(actor.current_workspace_id, body).await?;
        let service = self.service();
        if let LoadedSourceKind::Installed(id) = source.kind {
            let preview = service
                .preview_bundle(PreviewMcpBundleCommand {
                    actor_user_id: actor.user_id,
                    package: source.package.clone(),
                    interface_catalog: catalog.clone(),
                    current_system_version: current_version(),
                })
                .await?;
            if !validate_extension_integrity_override(
                &source.integrity_warnings,
                integrity_override.as_ref(),
            )? {
                return Ok(McpBundlesOutput::IntegrityChallenge(
                    InstalledMcpExtensionIntegrityChallengeResponse {
                        status: 409,
                        code: "mcp_bundle_integrity_confirmation_required".into(),
                        message: "Installed MCP artifact integrity warnings require confirmation."
                            .into(),
                        extension_installation_id: id.to_string(),
                        artifact_installation_status: "installed".into(),
                        workspace_application_status: "not_imported".into(),
                        integrity_warnings: source.integrity_warnings.clone(),
                        required_integrity_override: domain::ExtensionRiskChallenge {
                            warnings: source.integrity_warnings.clone(),
                            compatibility: None,
                        },
                        preview,
                    },
                ));
            }
        }
        let report = service
            .import_bundle(ImportMcpBundleCommand {
                actor_user_id: actor.user_id,
                package: source.package,
                interface_catalog: catalog,
                current_system_version: current_version(),
            })
            .await?;
        let reconciled = report.effect_summary.conflicts == 0 && report.effect_summary.failed == 0;
        if let LoadedSourceKind::Installed(id) = source.kind {
            if reconciled {
                service
                    .record_extension_bundle_import(actor.user_id, id, &report.status)
                    .await?;
            }
        }
        Ok(McpBundlesOutput::ImportOfficial(match source.kind {
            LoadedSourceKind::Installed(id) => McpBundleImportSourceResponse::InstalledExtension(
                InstalledMcpExtensionImportResponse {
                    extension_installation_id: id.to_string(),
                    artifact_installation_status: "installed".into(),
                    workspace_application_status: if reconciled {
                        "imported"
                    } else if report.effect_summary.changes > 0 {
                        "partially_imported"
                    } else {
                        "not_imported"
                    }
                    .into(),
                    integrity_warnings: source.integrity_warnings,
                    import_report: report,
                },
            ),
            LoadedSourceKind::Builtin => {
                McpBundleImportSourceResponse::BuiltinTemplate(BuiltinMcpTemplateImportResponse {
                    builtin_template_id: BUILTIN_FRONTSTAGE_CATALOG_ID.into(),
                    workspace_application_status: if reconciled {
                        "imported"
                    } else if report.effect_summary.changes > 0 {
                        "partially_imported"
                    } else {
                        "not_imported"
                    }
                    .into(),
                    import_report: report,
                })
            }
            LoadedSourceKind::Official => McpBundleImportSourceResponse::OfficialCatalog(report),
        }))
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: McpBundlesInput,
    ) -> Result<McpBundlesOutput, ApiError> {
        let actor = principal.actor();
        match input {
            McpBundlesInput::ListOfficial { locale } => Ok(McpBundlesOutput::OfficialCatalog(
                self.official_catalog(principal, locale).await?,
            )),
            McpBundlesInput::PreviewOfficial(body) => {
                self.authorize(principal).await?;
                Ok(McpBundlesOutput::PreviewOfficial(
                    self.preview_source(principal, body).await?,
                ))
            }
            McpBundlesInput::ImportOfficial(body) => {
                self.authorize(principal).await?;
                self.import_source(principal, body).await
            }
            McpBundlesInput::Export(body) => {
                self.authorize(principal).await?;
                let package = self
                    .service()
                    .export_bundle(ExportMcpBundleCommand {
                        actor_user_id: actor.user_id,
                        organization: body.organization,
                        bundle_id: body.bundle_id,
                        bundle_version: body.bundle_version,
                        locale: body.locale,
                        current_system_version: current_version(),
                    })
                    .await?;
                Ok(McpBundlesOutput::Archive(
                    self.archive(package, "mcp-bundle.zip".into(), Vec::new())
                        .await?,
                ))
            }
            McpBundlesInput::ExportDefaults => {
                self.authorize(principal).await?;
                let current_system_version = current_version();
                Ok(McpBundlesOutput::ExportDefaults(McpBundleExportDefaults {
                    minimum_host_version: current_system_version.clone(),
                    current_system_version,
                }))
            }
            McpBundlesInput::ExportInstance { instance_id, body } => {
                self.authorize(principal).await?;
                let filename = format!("mcp-instance-{}.zip", safe_filename_segment(&instance_id));
                let kind = match body.export_profile {
                    None | Some(McpInstanceBundleExportProfile::Portable) => {
                        McpInstanceBundleExportKind::Portable
                    }
                    Some(McpInstanceBundleExportProfile::OfficialBuiltin) => {
                        McpInstanceBundleExportKind::OfficialBuiltin {
                            interface_catalog: self.catalog(principal).await?,
                        }
                    }
                };
                let exported = self
                    .service()
                    .export_instance_bundle(ExportMcpInstanceBundleCommand {
                        actor_user_id: actor.user_id,
                        instance_id,
                        organization: body.organization,
                        bundle_id: body.bundle_id,
                        bundle_version: body.bundle_version,
                        locale: body.locale,
                        current_system_version: current_version(),
                        kind,
                    })
                    .await?;
                let headers = exported
                    .official_report
                    .map(|report| {
                        vec![
                            (
                                "x-1flowbase-mcp-excluded-tool-count".into(),
                                report.excluded_tool_count.to_string(),
                            ),
                            (
                                "x-1flowbase-mcp-exclusion-reasons".into(),
                                report.exclusion_reasons.join(","),
                            ),
                        ]
                    })
                    .unwrap_or_default();
                Ok(McpBundlesOutput::Archive(
                    self.archive(exported.package, filename, headers).await?,
                ))
            }
            McpBundlesInput::PreviewUploaded { bytes } => {
                self.authorize(principal).await?;
                let package = Self::parse(bytes).await?;
                Ok(McpBundlesOutput::Preview(
                    self.service()
                        .preview_bundle(PreviewMcpBundleCommand {
                            actor_user_id: actor.user_id,
                            package,
                            interface_catalog: self.catalog(principal).await?,
                            current_system_version: current_version(),
                        })
                        .await?,
                ))
            }
            McpBundlesInput::ImportUploaded { bytes } => {
                self.authorize(principal).await?;
                let package = Self::parse(bytes).await?;
                Ok(McpBundlesOutput::Import(
                    self.service()
                        .import_bundle(ImportMcpBundleCommand {
                            actor_user_id: actor.user_id,
                            package,
                            interface_catalog: self.catalog(principal).await?,
                            current_system_version: current_version(),
                        })
                        .await?,
                ))
            }
            McpBundlesInput::ListLibrary { refresh_remote } => {
                self.authorize(principal).await?;
                Ok(McpBundlesOutput::Library(if refresh_remote {
                    self.0.official_mcp_bundle_source.refresh_catalog().await?
                } else {
                    self.0.official_mcp_bundle_source.library_catalog().await?
                }))
            }
            McpBundlesInput::SyncLibrary {
                organization,
                bundle_id,
                body,
            } => {
                self.authorize(principal).await?;
                Ok(McpBundlesOutput::LibraryReceipt(
                    self.0
                        .official_mcp_bundle_source
                        .sync(&organization, &bundle_id, body.bundle_version.as_deref())
                        .await?,
                ))
            }
            McpBundlesInput::PreviewLibrary {
                organization,
                bundle_id,
                body,
            } => {
                self.authorize(principal).await?;
                let package = Self::parse(
                    self.0
                        .official_mcp_bundle_source
                        .resolve_artifact(&organization, &bundle_id, body.bundle_version.as_deref())
                        .await?,
                )
                .await?;
                Ok(McpBundlesOutput::Preview(
                    self.service()
                        .preview_bundle(PreviewMcpBundleCommand {
                            actor_user_id: actor.user_id,
                            package,
                            interface_catalog: self.catalog(principal).await?,
                            current_system_version: current_version(),
                        })
                        .await?,
                ))
            }
            McpBundlesInput::ImportLibrary {
                organization,
                bundle_id,
                body,
            } => {
                self.authorize(principal).await?;
                let package = Self::parse(
                    self.0
                        .official_mcp_bundle_source
                        .resolve_artifact(&organization, &bundle_id, body.bundle_version.as_deref())
                        .await?,
                )
                .await?;
                Ok(McpBundlesOutput::Import(
                    self.service()
                        .import_bundle(ImportMcpBundleCommand {
                            actor_user_id: actor.user_id,
                            package,
                            interface_catalog: self.catalog(principal).await?,
                            current_system_version: current_version(),
                        })
                        .await?,
                ))
            }
            McpBundlesInput::SwitchLibrary {
                organization,
                bundle_id,
                bundle_version,
            } => {
                self.authorize(principal).await?;
                Ok(McpBundlesOutput::LibraryReceipt(
                    self.0
                        .official_mcp_bundle_source
                        .switch_current(&organization, &bundle_id, &bundle_version)
                        .await?,
                ))
            }
            McpBundlesInput::DeleteLibraryRelease {
                organization,
                bundle_id,
                bundle_version,
            } => {
                self.authorize(principal).await?;
                self.0
                    .official_mcp_bundle_source
                    .delete_local_version(&organization, &bundle_id, &bundle_version)
                    .await?;
                Ok(McpBundlesOutput::Deleted)
            }
            McpBundlesInput::RepairLibraryRelease {
                organization,
                bundle_id,
                bundle_version,
            } => {
                self.authorize(principal).await?;
                Ok(McpBundlesOutput::LibraryReceipt(
                    self.0
                        .official_mcp_bundle_source
                        .repair(&organization, &bundle_id, &bundle_version)
                        .await?,
                ))
            }
        }
    }
}

impl ConsoleInterfacePort<McpBundlesInput, McpBundlesOutput> for McpBundlesAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: McpBundlesInput,
    ) -> ConsoleInterfaceFuture<'a, McpBundlesOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

fn current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
fn safe_filename_segment(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn project_official_catalog(
    source: &dyn OfficialExtensionCatalogSourcePort,
    pages: Vec<OfficialExtensionCatalogPage>,
) -> anyhow::Result<crate::official_mcp_bundles::OfficialMcpBundleCatalogSnapshot> {
    let first = pages
        .first()
        .ok_or_else(|| anyhow::anyhow!("official MCP catalog has no page"))?;
    let source_kind = first.source_kind.clone();
    let catalog_url = first.metadata.locator.clone();
    let entries = pages
        .into_iter()
        .flat_map(|page| page.entries)
        .map(|entry| project_official_entry(source, entry))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(
        crate::official_mcp_bundles::OfficialMcpBundleCatalogSnapshot {
            source: crate::official_mcp_bundles::OfficialMcpBundleCatalogSource {
                source_label: source_kind.clone(),
                source_kind,
                catalog_url,
            },
            entries,
        },
    )
}
fn project_official_entry(
    source: &dyn OfficialExtensionCatalogSourcePort,
    entry: OfficialExtensionCatalogEntry,
) -> anyhow::Result<crate::official_mcp_bundles::OfficialMcpBundleCatalogEntry> {
    if entry.category != "mcp" {
        anyhow::bail!("official MCP catalog projection received another category");
    }
    let metadata = |field| {
        entry
            .source
            .metadata
            .get(field)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .ok_or_else(|| anyhow::anyhow!("official extension catalog entry is missing {field}"))
    };
    let locale = metadata("locale")?;
    let exported_from_system_version = metadata("exported_from_system_version")?;
    let release_tag = metadata("release_tag")?;
    let descriptor = source.resolve_artifact(&entry)?;
    Ok(crate::official_mcp_bundles::OfficialMcpBundleCatalogEntry {
        organization: entry.organization,
        bundle_id: entry.artifact,
        latest_version: entry.version,
        locale,
        minimum_host_version: entry.host_version_requirement,
        exported_from_system_version,
        release_tag,
        download_url: descriptor.locator,
        artifact_sha256: descriptor.expected_checksum,
    })
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration { interface_id: "mcp.bundles.official.list", binding_id: "http.console.mcp.bundles.official.list.v1", method: "GET", path: "/api/console/mcp/bundles/official", mutating: false },
    ConsoleInterfaceDeclaration { interface_id: "mcp.bundles.preview", binding_id: "http.console.mcp.bundles.preview-official.v1", method: "POST", path: "/api/console/mcp/bundles/preview-official", mutating: true },
    ConsoleInterfaceDeclaration { interface_id: "mcp.bundles.import", binding_id: "http.console.mcp.bundles.import-official.v1", method: "POST", path: "/api/console/mcp/bundles/import-official", mutating: true },
    ConsoleInterfaceDeclaration { interface_id: "mcp.bundles.export", binding_id: "http.console.mcp.bundles.export.v1", method: "POST", path: "/api/console/mcp/bundles/export", mutating: true },
    ConsoleInterfaceDeclaration { interface_id: "mcp.bundles.export", binding_id: "http.console.mcp.bundles.export-defaults.v1", method: "GET", path: "/api/console/mcp/bundles/export-defaults", mutating: false },
    ConsoleInterfaceDeclaration { interface_id: "mcp.instances.export", binding_id: "http.console.mcp.instances.bundles.export.v1", method: "POST", path: "/api/console/mcp/instances/:instance_id/bundles/export", mutating: true },
    ConsoleInterfaceDeclaration { interface_id: "mcp.bundles.preview", binding_id: "http.console.mcp.bundles.preview-upload.v1", method: "POST", path: "/api/console/mcp/bundles/preview-upload", mutating: true },
    ConsoleInterfaceDeclaration { interface_id: "mcp.bundles.import", binding_id: "http.console.mcp.bundles.import-upload.v1", method: "POST", path: "/api/console/mcp/bundles/import-upload", mutating: true },
    ConsoleInterfaceDeclaration { interface_id: "mcp.bundle_library.list", binding_id: "http.console.mcp.bundles.library.list.v1", method: "GET", path: "/api/console/mcp/bundles/library", mutating: false },
    ConsoleInterfaceDeclaration { interface_id: "mcp.bundle_library.sync", binding_id: "http.console.mcp.bundles.library.sync.v1", method: "POST", path: "/api/console/mcp/bundles/library/:organization/:bundle_id/sync", mutating: true },
    ConsoleInterfaceDeclaration { interface_id: "mcp.bundle_library.preview", binding_id: "http.console.mcp.bundles.library.preview.v1", method: "POST", path: "/api/console/mcp/bundles/library/:organization/:bundle_id/preview", mutating: true },
    ConsoleInterfaceDeclaration { interface_id: "mcp.bundle_library.import", binding_id: "http.console.mcp.bundles.library.import.v1", method: "POST", path: "/api/console/mcp/bundles/library/:organization/:bundle_id/import", mutating: true },
    ConsoleInterfaceDeclaration { interface_id: "mcp.bundle_library.current.switch", binding_id: "http.console.mcp.bundles.library.current.switch.v1", method: "POST", path: "/api/console/mcp/bundles/library/:organization/:bundle_id/current/:bundle_version", mutating: true },
    ConsoleInterfaceDeclaration { interface_id: "mcp.bundle_library.releases.delete", binding_id: "http.console.mcp.bundles.library.releases.delete.v1", method: "DELETE", path: "/api/console/mcp/bundles/library/:organization/:bundle_id/releases/:bundle_version", mutating: true },
    ConsoleInterfaceDeclaration { interface_id: "mcp.bundle_library.releases.repair", binding_id: "http.console.mcp.bundles.library.releases.repair.v1", method: "POST", path: "/api/console/mcp/bundles/library/:organization/:bundle_id/releases/:bundle_version/repair", mutating: true },
];
pub(crate) fn compile_registry(
    port: Arc<dyn ConsoleInterfacePort<McpBundlesInput, McpBundlesOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-mcp-bundles",
        "graph:console-mcp-bundles-v1",
        DECLARATIONS,
        port,
    )
}
#[cfg(test)]
struct UnavailableMcpBundlesPort;
#[cfg(test)]
impl ConsoleInterfacePort<McpBundlesInput, McpBundlesOutput> for UnavailableMcpBundlesPort {
    fn execute<'a>(
        &'a self,
        _: &'a UserPrincipal,
        _: McpBundlesInput,
    ) -> ConsoleInterfaceFuture<'a, McpBundlesOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("MCP bundle fixture unavailable").into(),
            ))
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn f11a_registry_freezes_mcp_bundle_bindings() {
        let registry = compile_registry(Arc::new(UnavailableMcpBundlesPort)).unwrap();
        for declaration in DECLARATIONS {
            let binding = registry
                .binding(&interface_runtime::BindingId::new(declaration.binding_id).unwrap())
                .expect("declared MCP bundle binding must be frozen");
            let route = binding.projection().http_route().unwrap();
            assert_eq!(route.method(), declaration.method);
            assert_eq!(route.path(), declaration.path);
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }
}
