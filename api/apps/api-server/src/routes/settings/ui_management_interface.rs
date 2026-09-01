use std::sync::Arc;

use control_plane::{
    ports::{
        CreateUiCodeTemplateInput, CreateUiComponentRecordInput, ReviseUiCodeTemplateInput,
        UiComponentRecordPatch,
    },
    ui_component_catalog::{UiComponentCatalogService, UiComponentCatalogUpdateStatus},
    ui_management::{OfficialUiCodeTemplate, UiManagementService},
};
use domain::{UiCodeTemplate, UiComponentRecord, UiComponentRecordUpstream};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::ui_management::{
    ArchiveTemplateBody, CatalogComponentResponse, CatalogGroupUpdateResponse,
    CatalogIndexResponse, CatalogPageResponse, CatalogSearchEntryResponse, CatalogSearchQuery,
    CatalogSearchResponse, CatalogSyncResponse, CatalogUpdateStatusResponse,
    ComponentRecordResponse, ComponentUpstreamBody, CreateComponentBody, ListTemplatesQuery,
    ManagedTemplateResponse, OfficialTemplateResponse, PublishTemplateBody,
    ResetDefaultTemplateBody, TemplateBody, TemplateListResponse, TemplateRevisionResponse,
    UpdateComponentBody, UpdateTemplateBody,
};
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
    ui_component_catalog_source::ApiUiComponentCatalogSource,
};

pub(crate) enum UiManagementInput {
    ListTemplates(ListTemplatesQuery),
    CreateTemplate(TemplateBody),
    UpdateTemplate {
        id: String,
        body: UpdateTemplateBody,
    },
    PublishTemplate {
        id: String,
        body: PublishTemplateBody,
    },
    SetDefaultTemplate {
        id: String,
    },
    ResetDefaultTemplate(ResetDefaultTemplateBody),
    ArchiveTemplate {
        id: String,
        body: ArchiveTemplateBody,
    },
    ListComponents,
    GetComponent {
        id: String,
    },
    CreateComponent(CreateComponentBody),
    UpdateComponent {
        id: String,
        body: UpdateComponentBody,
    },
    DeleteComponent {
        id: String,
    },
    CatalogIndex,
    CatalogPage {
        page: u32,
    },
    CatalogSearch(CatalogSearchQuery),
    CatalogUpdateStatus,
    CatalogDownload {
        component_code: String,
    },
    CatalogSyncGroup {
        source: String,
        group: String,
    },
}

impl InterfaceContract for UiManagementInput {
    const CONTRACT_ID: &'static str = "console-ui-management-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum UiManagementOutput {
    Templates(TemplateListResponse),
    Template(ManagedTemplateResponse),
    Components(Vec<ComponentRecordResponse>),
    Component(ComponentRecordResponse),
    CatalogIndex(CatalogIndexResponse),
    CatalogPage(CatalogPageResponse),
    CatalogSearch(CatalogSearchResponse),
    CatalogUpdateStatus(CatalogUpdateStatusResponse),
    CatalogComponent(CatalogComponentResponse),
    CatalogSync(CatalogSyncResponse),
    NoContent,
}

impl InterfaceContract for UiManagementOutput {
    const CONTRACT_ID: &'static str = "console-ui-management-output";
    const CONTRACT_VERSION: &'static str = "1";
}

#[derive(Clone)]
pub(crate) struct UiManagementDependencies {
    pub(crate) store: MainDurableStore,
    pub(crate) api_node_id: String,
}

struct UiManagementAdapter(UiManagementDependencies);

impl UiManagementAdapter {
    fn management_service(&self) -> UiManagementService<MainDurableStore> {
        UiManagementService::new(self.0.store.clone(), self.0.api_node_id.clone())
    }

    fn catalog_service(&self) -> crate::app_state::ApiUiComponentCatalogService {
        UiComponentCatalogService::new(
            self.0.store.clone(),
            ApiUiComponentCatalogSource::default_taichuy(),
        )
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: UiManagementInput,
    ) -> Result<UiManagementOutput, ApiError> {
        let actor_user_id = principal.actor().user_id;
        match input {
            UiManagementInput::ListTemplates(query) => {
                let (official, managed) = self
                    .management_service()
                    .list_templates(query.include_archived)
                    .await?;
                Ok(UiManagementOutput::Templates(TemplateListResponse {
                    official: official.into_iter().map(official_response).collect(),
                    managed: managed.into_iter().map(template_response).collect(),
                }))
            }
            UiManagementInput::CreateTemplate(body) => {
                Ok(UiManagementOutput::Template(template_response(
                    self.management_service()
                        .create_template(CreateUiCodeTemplateInput {
                            provider_code: body.provider_code,
                            contribution_code: body.contribution_code,
                            name: body.name,
                            source: body.source,
                            language: body.language,
                            actor_user_id,
                        })
                        .await?,
                )))
            }
            UiManagementInput::UpdateTemplate { id, body } => {
                Ok(UiManagementOutput::Template(template_response(
                    self.management_service()
                        .revise_template(ReviseUiCodeTemplateInput {
                            template_id: parse_template_id(&id)?,
                            name: body.name,
                            source: body.source,
                            language: body.language,
                            actor_user_id,
                        })
                        .await?,
                )))
            }
            UiManagementInput::PublishTemplate { id, body } => {
                Ok(UiManagementOutput::Template(template_response(
                    self.management_service()
                        .publish_template(parse_template_id(&id)?, body.revision, actor_user_id)
                        .await?,
                )))
            }
            UiManagementInput::SetDefaultTemplate { id } => {
                self.management_service()
                    .set_template_default(parse_template_id(&id)?, actor_user_id)
                    .await?;
                Ok(UiManagementOutput::NoContent)
            }
            UiManagementInput::ResetDefaultTemplate(body) => {
                self.management_service()
                    .reset_template_default(&body.provider_code, &body.contribution_code)
                    .await?;
                Ok(UiManagementOutput::NoContent)
            }
            UiManagementInput::ArchiveTemplate { id, body } => {
                Ok(UiManagementOutput::Template(template_response(
                    self.management_service()
                        .set_template_archived(
                            parse_template_id(&id)?,
                            body.archived,
                            actor_user_id,
                        )
                        .await?,
                )))
            }
            UiManagementInput::ListComponents => Ok(UiManagementOutput::Components(
                self.management_service()
                    .list_component_records()
                    .await?
                    .into_iter()
                    .map(component_response)
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            UiManagementInput::GetComponent { id } => {
                Ok(UiManagementOutput::Component(component_response(
                    self.management_service()
                        .get_component_record(parse_component_id(&id)?)
                        .await?,
                )?))
            }
            UiManagementInput::CreateComponent(body) => {
                Ok(UiManagementOutput::Component(component_response(
                    self.management_service()
                        .create_component_record(CreateUiComponentRecordInput {
                            component_code: body.component_code,
                            name: body.name,
                            description: body.description,
                            import_code: body.import_code,
                            source_code: body.source_code,
                            source: body.source,
                            group: body.group,
                            upstream: UiComponentRecordUpstream {
                                identity: body.upstream.identity,
                                version: body.upstream.version,
                            },
                            version: body.version,
                            keywords: body.keywords,
                            actor_user_id,
                        })
                        .await?,
                )?))
            }
            UiManagementInput::UpdateComponent { id, body } => {
                Ok(UiManagementOutput::Component(component_response(
                    self.management_service()
                        .update_component_record(
                            parse_component_id(&id)?,
                            UiComponentRecordPatch {
                                name: body.name,
                                description: body.description,
                                import_code: body.import_code,
                                source_code: body.source_code,
                                source: body.source,
                                group: body.group,
                                upstream: UiComponentRecordUpstream {
                                    identity: body.upstream.identity,
                                    version: body.upstream.version,
                                },
                                version: body.version,
                                keywords: body.keywords,
                                actor_user_id,
                            },
                        )
                        .await?,
                )?))
            }
            UiManagementInput::DeleteComponent { id } => {
                self.management_service()
                    .delete_component_record(parse_component_id(&id)?)
                    .await?;
                Ok(UiManagementOutput::NoContent)
            }
            UiManagementInput::CatalogIndex => {
                let value = self.catalog_service().index().await?;
                use time::format_description::well_known::Rfc3339;
                Ok(UiManagementOutput::CatalogIndex(CatalogIndexResponse {
                    catalog_version: value.catalog_version,
                    generated_at: value.generated_at.format(&Rfc3339)?,
                    page_size: value.page_size,
                    total_components: value.total_components,
                    source_fingerprint: value.source_fingerprint,
                }))
            }
            UiManagementInput::CatalogPage { page } => {
                let value = self.catalog_service().page(page).await?;
                Ok(UiManagementOutput::CatalogPage(CatalogPageResponse {
                    catalog_version: value.catalog_version,
                    total_components: value.total_components,
                    page_size: value.page_size,
                    page: value.page,
                    cursor: value.cursor,
                    next_cursor: value.next_cursor,
                    records: value
                        .records
                        .into_iter()
                        .map(|record| {
                            catalog_component_response(record.catalog, record.local_version)
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                }))
            }
            UiManagementInput::CatalogSearch(query) => {
                let value = self
                    .catalog_service()
                    .search(&query.q, query.page, query.page_size)
                    .await?;
                Ok(UiManagementOutput::CatalogSearch(CatalogSearchResponse {
                    catalog_version: value.catalog_version,
                    page: value.page,
                    page_size: value.page_size,
                    total_entries: value.total_entries,
                    entries: value
                        .entries
                        .into_iter()
                        .map(|projection| {
                            let entry = projection.catalog;
                            CatalogSearchEntryResponse {
                                component_code: entry.component_code,
                                name: entry.name,
                                description: entry.description,
                                source: entry.source,
                                group: entry.group,
                                upstream: ComponentUpstreamBody {
                                    identity: entry.upstream.identity,
                                    version: entry.upstream.version,
                                },
                                version: entry.version,
                                keywords: entry.keywords,
                                catalog_page: entry.catalog_page,
                                local_version: projection.local_version,
                            }
                        })
                        .collect(),
                }))
            }
            UiManagementInput::CatalogUpdateStatus => Ok(UiManagementOutput::CatalogUpdateStatus(
                catalog_update_status_response(self.catalog_service().update_status().await?),
            )),
            UiManagementInput::CatalogDownload { component_code } => {
                let value = self
                    .catalog_service()
                    .download_component(&component_code, actor_user_id)
                    .await?;
                let local_version = Some(value.version.clone());
                Ok(UiManagementOutput::CatalogComponent(
                    catalog_component_response(value, local_version)?,
                ))
            }
            UiManagementInput::CatalogSyncGroup { source, group } => {
                let synchronized_records = self
                    .catalog_service()
                    .sync_source_group(&source, &group, actor_user_id)
                    .await?;
                Ok(UiManagementOutput::CatalogSync(CatalogSyncResponse {
                    synchronized_records,
                }))
            }
        }
    }
}

impl ConsoleInterfacePort<UiManagementInput, UiManagementOutput> for UiManagementAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: UiManagementInput,
    ) -> ConsoleInterfaceFuture<'a, UiManagementOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

fn parse_template_id(value: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("template_id").into())
}

fn parse_component_id(value: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value).map_err(|_| {
        control_plane::errors::ControlPlaneError::InvalidInput("ui_component_record_id").into()
    })
}

fn template_response(value: UiCodeTemplate) -> ManagedTemplateResponse {
    ManagedTemplateResponse {
        id: value.id.to_string(),
        provider_code: value.provider_code,
        contribution_code: value.contribution_code,
        name: value.name,
        latest_revision: TemplateRevisionResponse {
            revision: value.latest_revision.revision,
            source: value.latest_revision.source,
            language: value.latest_revision.language,
            is_published: value.latest_revision.is_published,
        },
        published_revision: value
            .published_revision
            .map(|revision| TemplateRevisionResponse {
                revision: revision.revision,
                source: revision.source,
                language: revision.language,
                is_published: true,
            }),
        is_default: value.is_default,
        is_archived: value.archived_at.is_some(),
    }
}

fn official_response(value: OfficialUiCodeTemplate) -> OfficialTemplateResponse {
    OfficialTemplateResponse {
        provider_code: value.provider_code,
        contribution_code: value.contribution_code,
        title: value.title,
        source: value.source,
        language: value.language,
        version: value.version,
        is_default: value.is_default,
    }
}

fn component_response(value: UiComponentRecord) -> Result<ComponentRecordResponse, ApiError> {
    use time::format_description::well_known::Rfc3339;

    Ok(ComponentRecordResponse {
        id: value.id.to_string(),
        scope_id: value.scope_id.to_string(),
        component_code: value.component_code,
        name: value.name,
        description: value.description,
        import_code: value.import_code,
        source_code: value.source_code,
        origin: value.origin,
        source: value.source,
        group: value.group,
        upstream: ComponentUpstreamBody {
            identity: value.upstream.identity,
            version: value.upstream.version,
        },
        version: value.version,
        keywords: value.keywords,
        catalog_updated_at: value
            .catalog_updated_at
            .map(|timestamp| timestamp.format(&Rfc3339))
            .transpose()?,
        source_locator: value.source_locator,
        source_checksum: value.source_checksum,
        created_at: value.created_at.format(&Rfc3339)?,
        updated_at: value.updated_at.format(&Rfc3339)?,
    })
}

fn catalog_component_response(
    value: control_plane::ports::OfficialUiComponentCatalogRecord,
    local_version: Option<String>,
) -> Result<CatalogComponentResponse, ApiError> {
    use time::format_description::well_known::Rfc3339;

    Ok(CatalogComponentResponse {
        component_code: value.component_code,
        name: value.name,
        description: value.description,
        import_code: value.import_code,
        source_code: value.source_code,
        source: value.source,
        group: value.group,
        upstream: ComponentUpstreamBody {
            identity: value.upstream.identity,
            version: value.upstream.version,
        },
        version: value.version,
        keywords: value.keywords,
        catalog_updated_at: value.catalog_updated_at.format(&Rfc3339)?,
        source_locator: value.source_locator,
        source_checksum: value.source_checksum,
        local_version,
    })
}

fn catalog_update_status_response(
    value: UiComponentCatalogUpdateStatus,
) -> CatalogUpdateStatusResponse {
    CatalogUpdateStatusResponse {
        catalog_version: value.catalog_version,
        source_fingerprint: value.source_fingerprint,
        update_available: value.update_available,
        groups: value
            .groups
            .into_iter()
            .map(|group| {
                let update_available = group.update_available();
                CatalogGroupUpdateResponse {
                    source: group.source,
                    group: group.group,
                    remote_records: group.remote_records,
                    new_or_updated_records: group.new_or_updated_records,
                    removed_records: group.removed_records,
                    update_available,
                }
            })
            .collect(),
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "ui_management.templates.list",
        binding_id: "http.console.ui-management.templates.list.get.v1",
        method: "GET",
        path: "/api/console/settings/ui-management/templates",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "ui_management.templates.create",
        binding_id: "http.console.ui-management.templates.create.post.v1",
        method: "POST",
        path: "/api/console/settings/ui-management/templates",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "ui_management.templates.default.reset",
        binding_id: "http.console.ui-management.templates.default.reset.delete.v1",
        method: "DELETE",
        path: "/api/console/settings/ui-management/templates/default",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "ui_management.templates.update",
        binding_id: "http.console.ui-management.templates.update.put.v1",
        method: "PUT",
        path: "/api/console/settings/ui-management/templates/:id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "ui_management.templates.publish",
        binding_id: "http.console.ui-management.templates.publish.post.v1",
        method: "POST",
        path: "/api/console/settings/ui-management/templates/:id/publish",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "ui_management.templates.default.set",
        binding_id: "http.console.ui-management.templates.default.set.put.v1",
        method: "PUT",
        path: "/api/console/settings/ui-management/templates/:id/default",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "ui_management.templates.archive",
        binding_id: "http.console.ui-management.templates.archive.put.v1",
        method: "PUT",
        path: "/api/console/settings/ui-management/templates/:id/archive",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "ui_management.components.list",
        binding_id: "http.console.ui-management.components.list.get.v1",
        method: "GET",
        path: "/api/console/settings/ui-management/components",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "ui_management.components.create",
        binding_id: "http.console.ui-management.components.create.post.v1",
        method: "POST",
        path: "/api/console/settings/ui-management/components",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "ui_management.components.view",
        binding_id: "http.console.ui-management.components.view.get.v1",
        method: "GET",
        path: "/api/console/settings/ui-management/components/:id",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "ui_management.components.update",
        binding_id: "http.console.ui-management.components.update.put.v1",
        method: "PUT",
        path: "/api/console/settings/ui-management/components/:id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "ui_management.components.delete",
        binding_id: "http.console.ui-management.components.delete.delete.v1",
        method: "DELETE",
        path: "/api/console/settings/ui-management/components/:id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "ui_management.catalog.index",
        binding_id: "http.console.ui-management.catalog.index.get.v1",
        method: "GET",
        path: "/api/console/settings/ui-management/components/catalog/index",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "ui_management.catalog.page",
        binding_id: "http.console.ui-management.catalog.page.get.v1",
        method: "GET",
        path: "/api/console/settings/ui-management/components/catalog/pages/:page",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "ui_management.catalog.search",
        binding_id: "http.console.ui-management.catalog.search.get.v1",
        method: "GET",
        path: "/api/console/settings/ui-management/components/catalog/search",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "ui_management.catalog.update_status",
        binding_id: "http.console.ui-management.catalog.update-status.get.v1",
        method: "GET",
        path: "/api/console/settings/ui-management/components/catalog/update-status",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "ui_management.catalog.download",
        binding_id: "http.console.ui-management.catalog.download.post.v1",
        method: "POST",
        path: "/api/console/settings/ui-management/components/catalog/:component_code/download",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "ui_management.catalog.sync_group",
        binding_id: "http.console.ui-management.catalog.sync-group.post.v1",
        method: "POST",
        path: "/api/console/settings/ui-management/components/catalog/groups/:source/:group/sync",
        mutating: true,
    },
];

pub(crate) fn ui_management_port(
    dependencies: UiManagementDependencies,
) -> Arc<dyn ConsoleInterfacePort<UiManagementInput, UiManagementOutput>> {
    Arc::new(UiManagementAdapter(dependencies))
}

pub(crate) fn compile_registry(
    port: Arc<dyn ConsoleInterfacePort<UiManagementInput, UiManagementOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-ui-management",
        "graph:console-ui-management-v1",
        DECLARATIONS,
        port,
    )
}

#[cfg(test)]
struct UnavailableUiManagementPort;

#[cfg(test)]
impl ConsoleInterfacePort<UiManagementInput, UiManagementOutput> for UnavailableUiManagementPort {
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: UiManagementInput,
    ) -> ConsoleInterfaceFuture<'a, UiManagementOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("ui management fixture unavailable").into(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f12b_registry_freezes_all_ui_management_bindings() {
        let registry = compile_registry(Arc::new(UnavailableUiManagementPort)).unwrap();
        for declaration in DECLARATIONS {
            let binding = registry
                .binding(&interface_runtime::BindingId::new(declaration.binding_id).unwrap())
                .expect("declared UI management binding must be frozen");
            let route = binding.projection().http_route().unwrap();
            assert_eq!(route.method(), declaration.method);
            assert_eq!(route.path(), declaration.path);
        }
        assert_eq!(DECLARATIONS.len(), 18);
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }
}
