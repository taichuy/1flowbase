use std::sync::Arc;

use control_plane::flow::{FlowService, SaveFlowDraftCommand, UpdateFlowVersionMetadataCommand};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::{
    build_application_archive_zip, installed_application_archive_entry_with, parse_change_kind,
    safe_archive_name, to_import_response_with, to_response_with, to_template_preview_response,
    AgentFlowTemplatePreviewResponse, ApplicationArchiveEntry, ExportApplicationArchiveBody,
    ImportAgentFlowTemplateResponse, ImportInstalledApplicationExtensionBody,
    InstalledApplicationExtensionPreviewResponse, OrchestrationStateResponse, SaveDraftBody,
    UpdateVersionBody,
};
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError, ConsoleLocaleHints,
    },
};

pub(crate) enum ApplicationOrchestrationInput {
    Get {
        application_id: Uuid,
        locale: ConsoleLocaleHints,
    },
    SaveDraft {
        application_id: Uuid,
        body: SaveDraftBody,
        locale: ConsoleLocaleHints,
    },
    RestoreVersion {
        application_id: Uuid,
        version_id: Uuid,
        locale: ConsoleLocaleHints,
    },
    UpdateVersion {
        application_id: Uuid,
        version_id: Uuid,
        body: UpdateVersionBody,
        locale: ConsoleLocaleHints,
    },
    PreviewUploadedArchive(ApplicationArchiveEntry),
    ImportUploadedArchive {
        entry: ApplicationArchiveEntry,
        name: Option<String>,
        description: Option<String>,
        locale: ConsoleLocaleHints,
    },
    ExportArchive(ExportApplicationArchiveBody),
    PreviewInstalledArchive(Uuid),
    ImportInstalledArchive {
        installation_id: Uuid,
        body: ImportInstalledApplicationExtensionBody,
        locale: ConsoleLocaleHints,
    },
}

impl InterfaceContract for ApplicationOrchestrationInput {
    const CONTRACT_ID: &'static str = "console-application-orchestration-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[expect(
    clippy::large_enum_variant,
    reason = "the typed orchestration output is short-lived and projected at the route boundary"
)]
pub(crate) enum ApplicationOrchestrationOutput {
    State(OrchestrationStateResponse),
    ArchivePreview(AgentFlowTemplatePreviewResponse),
    ArchiveImport(ImportAgentFlowTemplateResponse),
    InstalledArchivePreview(InstalledApplicationExtensionPreviewResponse),
    ExportedArchive(ExportedApplicationArchive),
}

impl ApplicationOrchestrationOutput {
    pub(super) fn into_state(self) -> Result<OrchestrationStateResponse, ApiError> {
        match self {
            Self::State(state) => Ok(state),
            _ => Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "application_orchestration_output",
            )
            .into()),
        }
    }

    pub(super) fn into_archive_preview(self) -> Result<AgentFlowTemplatePreviewResponse, ApiError> {
        match self {
            Self::ArchivePreview(value) => Ok(value),
            _ => Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "application_orchestration_output",
            )
            .into()),
        }
    }

    pub(super) fn into_archive_import(self) -> Result<ImportAgentFlowTemplateResponse, ApiError> {
        match self {
            Self::ArchiveImport(value) => Ok(value),
            _ => Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "application_orchestration_output",
            )
            .into()),
        }
    }

    pub(super) fn into_installed_preview(
        self,
    ) -> Result<InstalledApplicationExtensionPreviewResponse, ApiError> {
        match self {
            Self::InstalledArchivePreview(value) => Ok(value),
            _ => Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "application_orchestration_output",
            )
            .into()),
        }
    }

    pub(super) fn into_exported_archive(self) -> Result<ExportedApplicationArchive, ApiError> {
        match self {
            Self::ExportedArchive(value) => Ok(value),
            _ => Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "application_orchestration_output",
            )
            .into()),
        }
    }
}

pub(crate) struct ExportedApplicationArchive {
    pub(super) content_type: &'static str,
    pub(super) filename: String,
    pub(super) document: Vec<u8>,
}

impl InterfaceContract for ApplicationOrchestrationOutput {
    const CONTRACT_ID: &'static str = "console-application-orchestration-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct ApplicationOrchestrationAdapter {
    store: MainDurableStore,
    bootstrap_workspace_id: Uuid,
    api_node_id: String,
    provider_install_root: String,
}

pub(crate) fn port(
    store: MainDurableStore,
    bootstrap_workspace_id: Uuid,
    api_node_id: String,
    provider_install_root: String,
) -> Arc<dyn ConsoleInterfacePort<ApplicationOrchestrationInput, ApplicationOrchestrationOutput>> {
    Arc::new(ApplicationOrchestrationAdapter {
        store,
        bootstrap_workspace_id,
        api_node_id,
        provider_install_root,
    })
}

impl ApplicationOrchestrationAdapter {
    async fn preferred_locale(
        &self,
        principal: &UserPrincipal,
    ) -> Result<Option<String>, ApiError> {
        Ok(self
            .store
            .find_user_by_id(principal.actor().user_id)
            .await?
            .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?
            .preferred_locale)
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: ApplicationOrchestrationInput,
    ) -> Result<ApplicationOrchestrationOutput, ApiError> {
        let actor = principal.actor();
        let user_id = actor.user_id;
        let service = FlowService::new(self.store.for_actor(actor.clone()));
        let input = match input {
            ApplicationOrchestrationInput::PreviewUploadedArchive(entry) => {
                let resources = service.load_agent_flow_template_resources(user_id).await?;
                let preview = control_plane::application::ApplicationArchiveService::new(
                    self.store.for_actor(actor.clone()),
                )
                .preview_archive(
                    control_plane::application::PreviewApplicationArchiveCommand {
                        actor_user_id: user_id,
                        entry,
                        resources,
                    },
                )
                .await?;
                return Ok(ApplicationOrchestrationOutput::ArchivePreview(
                    to_template_preview_response(preview),
                ));
            }
            ApplicationOrchestrationInput::ImportUploadedArchive {
                entry,
                name,
                description,
                locale,
            } => {
                let resources = service.load_agent_flow_template_resources(user_id).await?;
                let imported = control_plane::application::ApplicationArchiveService::new(
                    self.store.for_actor(actor.clone()),
                )
                .import_archive(
                    control_plane::application::ImportApplicationArchiveCommand {
                        actor_user_id: user_id,
                        entry,
                        name,
                        description,
                        resources,
                        source_extension_installation_id: None,
                    },
                )
                .await?;
                let locale = locale.resolve(self.preferred_locale(principal).await?);
                return Ok(ApplicationOrchestrationOutput::ArchiveImport(
                    to_import_response_with(
                        &self.store,
                        self.bootstrap_workspace_id,
                        &locale,
                        imported,
                    )
                    .await?,
                ));
            }
            ApplicationOrchestrationInput::ExportArchive(body) => {
                let exported_at = time::OffsetDateTime::now_utc()
                    .format(&time::format_description::well_known::Rfc3339)
                    .map_err(|_| {
                        control_plane::errors::ControlPlaneError::InvalidInput(
                            "application_archive_exported_at",
                        )
                    })?;
                let package = control_plane::application::ApplicationArchiveService::new(
                    self.store.for_actor(actor.clone()),
                )
                .export_archive(
                    control_plane::application::ExportApplicationArchiveCommand {
                        actor_user_id: user_id,
                        application_ids: body.application_ids,
                        exported_from_system_version: env!("CARGO_PKG_VERSION").to_string(),
                        exported_at,
                    },
                )
                .await?;
                let exported = match package.applications.as_slice() {
                    [application] => ExportedApplicationArchive {
                        content_type: "application/json; charset=utf-8",
                        filename: format!(
                            "{}.1flowbase-application.json",
                            safe_archive_name(&application.application.name)
                        ),
                        document: serde_json::to_vec_pretty(&package)?,
                    },
                    applications => {
                        let filename = format!("applications-{}-items.zip", applications.len());
                        let document = tokio::task::spawn_blocking(move || {
                            build_application_archive_zip(&package)
                        })
                        .await
                        .map_err(|_| {
                            control_plane::errors::ControlPlaneError::InvalidInput(
                                "application_archive",
                            )
                        })??;
                        ExportedApplicationArchive {
                            content_type: "application/zip",
                            filename,
                            document,
                        }
                    }
                };
                return Ok(ApplicationOrchestrationOutput::ExportedArchive(exported));
            }
            ApplicationOrchestrationInput::PreviewInstalledArchive(installation_id) => {
                let (entry, warnings) = installed_application_archive_entry_with(
                    &self.store,
                    &self.provider_install_root,
                    &self.api_node_id,
                    installation_id,
                )
                .await?;
                let resources = service.load_agent_flow_template_resources(user_id).await?;
                let preview = control_plane::application::ApplicationArchiveService::new(
                    self.store.for_actor(actor.clone()),
                )
                .preview_archive(
                    control_plane::application::PreviewApplicationArchiveCommand {
                        actor_user_id: user_id,
                        entry,
                        resources,
                    },
                )
                .await?;
                let applied =
                    control_plane::ports::ApplicationRepository::has_application_extension_source(
                        &self.store,
                        actor.current_workspace_id,
                        installation_id,
                    )
                    .await?;
                return Ok(ApplicationOrchestrationOutput::InstalledArchivePreview(
                    InstalledApplicationExtensionPreviewResponse {
                        extension_installation_id: installation_id.to_string(),
                        application_status: if applied { "applied" } else { "not_applied" }
                            .to_string(),
                        required_integrity_override: (!warnings.is_empty()).then(|| {
                            domain::ExtensionRiskChallenge {
                                warnings: warnings.clone(),
                                compatibility: None,
                            }
                        }),
                        integrity_warnings: warnings,
                        preview: to_template_preview_response(preview),
                    },
                ));
            }
            ApplicationOrchestrationInput::ImportInstalledArchive {
                installation_id,
                body,
                locale,
            } => {
                let (entry, warnings) = installed_application_archive_entry_with(
                    &self.store,
                    &self.provider_install_root,
                    &self.api_node_id,
                    installation_id,
                )
                .await?;
                let risk_override = body.integrity_override.map(|value| {
                    control_plane::plugin_management::ExtensionRiskOverride {
                        reason: value.reason,
                        acknowledged_warnings: value.acknowledged_warnings,
                    }
                });
                if !control_plane::plugin_management::validate_extension_integrity_override(
                    &warnings,
                    risk_override.as_ref(),
                )? {
                    return Err(control_plane::errors::ControlPlaneError::Conflict(
                        "agent_flow_extension_integrity_confirmation_required",
                    )
                    .into());
                }
                let resources = service.load_agent_flow_template_resources(user_id).await?;
                let imported = control_plane::application::ApplicationArchiveService::new(
                    self.store.for_actor(actor.clone()),
                )
                .import_archive(
                    control_plane::application::ImportApplicationArchiveCommand {
                        actor_user_id: user_id,
                        entry,
                        name: body.name,
                        description: body.description,
                        resources,
                        source_extension_installation_id: Some(installation_id),
                    },
                )
                .await?;
                let locale = locale.resolve(self.preferred_locale(principal).await?);
                return Ok(ApplicationOrchestrationOutput::ArchiveImport(
                    to_import_response_with(
                        &self.store,
                        self.bootstrap_workspace_id,
                        &locale,
                        imported,
                    )
                    .await?,
                ));
            }
            input => input,
        };
        let (state, locale) = match input {
            ApplicationOrchestrationInput::Get {
                application_id,
                locale,
            } => (
                service
                    .get_or_create_editor_state(user_id, application_id)
                    .await?,
                locale,
            ),
            ApplicationOrchestrationInput::SaveDraft {
                application_id,
                body,
                locale,
            } => (
                service
                    .save_draft(SaveFlowDraftCommand {
                        actor_user_id: user_id,
                        application_id,
                        document: body.document,
                        change_kind: parse_change_kind(&body.change_kind)?,
                        summary: body.summary,
                    })
                    .await?,
                locale,
            ),
            ApplicationOrchestrationInput::RestoreVersion {
                application_id,
                version_id,
                locale,
            } => (
                service
                    .restore_version(user_id, application_id, version_id)
                    .await?,
                locale,
            ),
            ApplicationOrchestrationInput::UpdateVersion {
                application_id,
                version_id,
                body,
                locale,
            } => (
                service
                    .update_version_metadata(UpdateFlowVersionMetadataCommand {
                        actor_user_id: user_id,
                        application_id,
                        version_id,
                        summary: body.summary,
                        summary_is_custom: body.summary_is_custom,
                        is_user_protected: body.is_user_protected,
                    })
                    .await?,
                locale,
            ),
            ApplicationOrchestrationInput::PreviewUploadedArchive(_)
            | ApplicationOrchestrationInput::ImportUploadedArchive { .. }
            | ApplicationOrchestrationInput::ExportArchive(_)
            | ApplicationOrchestrationInput::PreviewInstalledArchive(_)
            | ApplicationOrchestrationInput::ImportInstalledArchive { .. } => {
                unreachable!("archive operations return before editor dispatch")
            }
        };
        let locale = locale.resolve(self.preferred_locale(principal).await?);
        Ok(ApplicationOrchestrationOutput::State(
            to_response_with(&self.store, self.bootstrap_workspace_id, &locale, state).await?,
        ))
    }
}

impl ConsoleInterfacePort<ApplicationOrchestrationInput, ApplicationOrchestrationOutput>
    for ApplicationOrchestrationAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ApplicationOrchestrationInput,
    ) -> ConsoleInterfaceFuture<'a, ApplicationOrchestrationOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "applications.archive.export",
        binding_id: "http.console.applications.archive.export.v1",
        method: "POST",
        path: "/api/console/applications/archive/export",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.archive.preview",
        binding_id: "http.console.applications.archive.preview.v1",
        method: "POST",
        path: "/api/console/applications/archive/preview",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.archive.import",
        binding_id: "http.console.applications.archive.import.v1",
        method: "POST",
        path: "/api/console/applications/archive/import",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.archive.installed.preview",
        binding_id: "http.console.applications.archive.installed.preview.v1",
        method: "GET",
        path: "/api/console/applications/archive/installed-extension/:installation_id/preview",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.archive.installed.import",
        binding_id: "http.console.applications.archive.installed.import.v1",
        method: "POST",
        path: "/api/console/applications/archive/installed-extension/:installation_id/import",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.orchestration.get",
        binding_id: "http.console.applications.orchestration.get.v1",
        method: "GET",
        path: "/api/console/applications/:id/orchestration",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.orchestration.draft.save",
        binding_id: "http.console.applications.orchestration.draft.save.v1",
        method: "PUT",
        path: "/api/console/applications/:id/orchestration/draft",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.orchestration.version.restore",
        binding_id: "http.console.applications.orchestration.version.restore.v1",
        method: "POST",
        path: "/api/console/applications/:id/orchestration/versions/:version_id/restore",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.orchestration.version.update",
        binding_id: "http.console.applications.orchestration.version.update.v1",
        method: "PATCH",
        path: "/api/console/applications/:id/orchestration/versions/:version_id",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    port: Arc<
        dyn ConsoleInterfacePort<ApplicationOrchestrationInput, ApplicationOrchestrationOutput>,
    >,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-application-orchestration",
        "api-server.console-application-orchestration.graph.v1",
        DECLARATIONS,
        port,
    )
}
