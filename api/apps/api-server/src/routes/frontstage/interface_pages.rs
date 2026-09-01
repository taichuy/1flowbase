use std::sync::Arc;

use interface_runtime::{InterfaceContract, UserPrincipal};

use super::*;
use crate::routes::console_interface::{
    self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    ConsoleInterfaceTargetError, ConsoleLocaleHints,
};

pub(crate) enum FrontstagePagesInput {
    List,
    CreateGroup(CreateFrontstageGroupBody),
    CreatePage(CreateFrontstagePageBody, ConsoleLocaleHints),
    Detail(String, String, ConsoleLocaleHints),
    Update(String, UpdateFrontstagePageMetadataBody),
    Move(String, MoveFrontstagePageBody),
    Delete(String),
    ListTabs(String, ConsoleLocaleHints),
    CreateTab(String, CreateFrontstagePageTabBody),
    UpdateTab(String, String, UpdateFrontstagePageTabBody),
    DeleteTab(String, String),
    SaveDocument(String, String, SaveFrontstageTabDocumentBody),
    ListUiTemplates,
}

impl InterfaceContract for FrontstagePagesInput {
    const CONTRACT_ID: &'static str = "console-frontstage-pages-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum FrontstagePagesOutput {
    Tree(Vec<FrontstagePageTreeNodeResponse>),
    Page(FrontstagePageResponse),
    Creation(FrontstagePageCreationResponse),
    Detail(FrontstagePageDetailResponse),
    Tabs(Vec<FrontstagePageTabResponse>),
    Tab(FrontstagePageTabResponse),
    UiTemplates(Vec<FrontstageUiTemplateResponse>),
    NoContent,
}
impl InterfaceContract for FrontstagePagesOutput {
    const CONTRACT_ID: &'static str = "console-frontstage-pages-output";
    const CONTRACT_VERSION: &'static str = "1";
}

#[derive(Clone)]
pub(crate) struct FrontstagePagesDependencies {
    pub(crate) store: storage_durable_postgres::MainDurableStore,
    pub(crate) bootstrap_workspace_id: Uuid,
    pub(crate) api_node_id: String,
}
struct FrontstagePagesAdapter(FrontstagePagesDependencies);

impl FrontstagePagesAdapter {
    async fn localize_default_tab(
        &self,
        actor: &domain::ActorContext,
        locale: ConsoleLocaleHints,
        tab: &mut domain::frontstage::FrontstagePageTabRecord,
    ) -> Result<(), ApiError> {
        if !tab.is_default {
            return Ok(());
        }
        let preferred = self
            .0
            .store
            .find_user_by_id(actor.user_id)
            .await?
            .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?
            .preferred_locale;
        let locale = locale.resolve(preferred);
        let stored = tab.title.as_deref().unwrap_or_default();
        tab.title = Some(
            crate::app_state::project_canonical_display_with(
                &self.0.store,
                self.0.bootstrap_workspace_id,
                &locale,
                "Default",
                stored,
            )
            .await?,
        );
        Ok(())
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: FrontstagePagesInput,
    ) -> Result<FrontstagePagesOutput, ApiError> {
        let actor = principal.actor();
        let workspace_id = actor.current_workspace_id;
        let service = FrontstagePageService::for_actor(self.0.store.clone(), actor.clone());
        match input {
            FrontstagePagesInput::List => Ok(FrontstagePagesOutput::Tree(
                service
                    .list_page_tree(actor.user_id, workspace_id)
                    .await?
                    .into_iter()
                    .map(to_tree_node_response)
                    .collect(),
            )),
            FrontstagePagesInput::CreateGroup(body) => {
                let page = service
                    .create_group(CreateFrontstageGroupCommand {
                        actor_user_id: actor.user_id,
                        workspace_id,
                        title: body.title,
                        icon: body.icon,
                        tooltip: body.tooltip,
                        parent_id: parse_optional_uuid(body.parent_id.as_deref(), "parent_id")?,
                        rank: body.rank,
                        placement: to_domain_placement(body.placement),
                        slug: body.slug,
                    })
                    .await?;
                Ok(FrontstagePagesOutput::Page(to_page_response(page)))
            }
            FrontstagePagesInput::CreatePage(body, locale) => {
                let creation = service
                    .create_page(CreateFrontstagePageCommand {
                        actor_user_id: actor.user_id,
                        workspace_id,
                        title: body.title,
                        icon: body.icon,
                        tooltip: body.tooltip,
                        parent_id: parse_optional_uuid(body.parent_id.as_deref(), "parent_id")?,
                        rank: body.rank,
                        placement: to_domain_placement(body.placement),
                        slug: body.slug,
                    })
                    .await?;
                let mut default_tab = creation.default_tab.ok_or(
                    control_plane::errors::ControlPlaneError::Conflict(
                        "frontstage_page_requires_tab",
                    ),
                )?;
                self.localize_default_tab(actor, locale, &mut default_tab)
                    .await?;
                Ok(FrontstagePagesOutput::Creation(
                    FrontstagePageCreationResponse {
                        page: to_page_response(creation.page),
                        default_tab: to_tab_response(default_tab),
                    },
                ))
            }
            FrontstagePagesInput::Detail(page_id, tab_reference, locale) => {
                let mut detail = service
                    .get_page_detail(GetFrontstagePageDetailCommand {
                        actor_user_id: actor.user_id,
                        workspace_id,
                        page_id: parse_uuid(&page_id, "page_id")?,
                        tab_reference,
                    })
                    .await?;
                self.localize_default_tab(actor, locale, &mut detail.tab)
                    .await?;
                Ok(FrontstagePagesOutput::Detail(to_page_detail_response(
                    detail,
                )))
            }
            FrontstagePagesInput::Update(page_id, body) => {
                let page = service
                    .update_metadata(UpdateFrontstagePageMetadataCommand {
                        actor_user_id: actor.user_id,
                        workspace_id,
                        page_id: parse_uuid(&page_id, "page_id")?,
                        title: body.title,
                        icon: body.icon,
                        tooltip: body.tooltip,
                        is_hidden: body.is_hidden,
                        placement: body.placement.map(to_domain_placement),
                        content_presentation: body
                            .content_presentation
                            .map(to_domain_content_presentation),
                        slug: body.slug,
                    })
                    .await?;
                Ok(FrontstagePagesOutput::Page(to_page_response(page)))
            }
            FrontstagePagesInput::Move(page_id, body) => {
                let page = service
                    .move_page(MoveFrontstagePageCommand {
                        actor_user_id: actor.user_id,
                        workspace_id,
                        page_id: parse_uuid(&page_id, "page_id")?,
                        parent_id: parse_optional_uuid(body.parent_id.as_deref(), "parent_id")?,
                        rank: body.rank,
                    })
                    .await?;
                Ok(FrontstagePagesOutput::Page(to_page_response(page)))
            }
            FrontstagePagesInput::Delete(page_id) => {
                service
                    .delete_page(DeleteFrontstagePageCommand {
                        actor_user_id: actor.user_id,
                        workspace_id,
                        page_id: parse_uuid(&page_id, "page_id")?,
                    })
                    .await?;
                Ok(FrontstagePagesOutput::NoContent)
            }
            FrontstagePagesInput::ListTabs(page_id, locale) => {
                let mut tabs = service
                    .list_page_tabs(
                        actor.user_id,
                        workspace_id,
                        parse_uuid(&page_id, "page_id")?,
                    )
                    .await?;
                for tab in &mut tabs {
                    self.localize_default_tab(actor, locale.clone(), tab)
                        .await?;
                }
                Ok(FrontstagePagesOutput::Tabs(
                    tabs.into_iter().map(to_tab_response).collect(),
                ))
            }
            FrontstagePagesInput::CreateTab(page_id, body) => {
                let tab = service
                    .create_page_tab(CreateFrontstagePageTabCommand {
                        actor_user_id: actor.user_id,
                        workspace_id,
                        page_id: parse_uuid(&page_id, "page_id")?,
                        title: body.title,
                        route_segment: body.route_segment,
                        rank: body.rank,
                    })
                    .await?;
                Ok(FrontstagePagesOutput::Tab(to_tab_response(tab)))
            }
            FrontstagePagesInput::UpdateTab(page_id, tab_id, body) => {
                let tab = service
                    .update_page_tab(UpdateFrontstagePageTabCommand {
                        actor_user_id: actor.user_id,
                        workspace_id,
                        page_id: parse_uuid(&page_id, "page_id")?,
                        tab_id: parse_uuid(&tab_id, "tab_id")?,
                        title: body.title,
                        rank: body.rank,
                    })
                    .await?;
                Ok(FrontstagePagesOutput::Tab(to_tab_response(tab)))
            }
            FrontstagePagesInput::DeleteTab(page_id, tab_id) => {
                service
                    .delete_page_tab(DeleteFrontstagePageTabCommand {
                        actor_user_id: actor.user_id,
                        workspace_id,
                        page_id: parse_uuid(&page_id, "page_id")?,
                        tab_id: parse_uuid(&tab_id, "tab_id")?,
                    })
                    .await?;
                Ok(FrontstagePagesOutput::NoContent)
            }
            FrontstagePagesInput::SaveDocument(page_id, tab_id, body) => {
                let detail = service
                    .save_tab_document(SaveFrontstageTabDocumentCommand {
                        actor_user_id: actor.user_id,
                        workspace_id,
                        page_id: parse_uuid(&page_id, "page_id")?,
                        tab_id: parse_uuid(&tab_id, "tab_id")?,
                        document_payload: body.payload,
                    })
                    .await?;
                Ok(FrontstagePagesOutput::Detail(to_page_detail_response(
                    detail,
                )))
            }
            FrontstagePagesInput::ListUiTemplates => {
                if !actor.has_permission("frontstage.page.design") {
                    return Err(control_plane::errors::ControlPlaneError::PermissionDenied(
                        "frontstage.page.design",
                    )
                    .into());
                }
                let values = control_plane::ui_management::UiManagementService::new(
                    self.0.store.clone(),
                    self.0.api_node_id.clone(),
                )
                .list_published_templates_for_workspace(actor.current_workspace_id)
                .await?;
                Ok(FrontstagePagesOutput::UiTemplates(
                    values
                        .into_iter()
                        .map(|value| FrontstageUiTemplateResponse {
                            template_id: value.template_id.map(|id| id.to_string()),
                            provider_code: value.provider_code,
                            contribution_code: value.contribution_code,
                            name: value.name,
                            source: value.source,
                            language: value.language,
                            version: value.version,
                            is_official: value.is_official,
                            is_default: value.is_default,
                        })
                        .collect(),
                ))
            }
        }
    }
}

impl ConsoleInterfacePort<FrontstagePagesInput, FrontstagePagesOutput> for FrontstagePagesAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: FrontstagePagesInput,
    ) -> ConsoleInterfaceFuture<'a, FrontstagePagesOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.pages.view",
        binding_id: "http.console.frontstage.pages.get.v1",
        method: "GET",
        path: "/api/console/frontstage/pages",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.pages.create",
        binding_id: "http.console.frontstage.pages.post.v1",
        method: "POST",
        path: "/api/console/frontstage/pages",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.groups.create",
        binding_id: "http.console.frontstage.groups.post.v1",
        method: "POST",
        path: "/api/console/frontstage/pages/groups",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.pages.view",
        binding_id: "http.console.frontstage.page-detail.get.v1",
        method: "GET",
        path: "/api/console/frontstage/pages/:page_id/tabs/:tab_reference",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.pages.update",
        binding_id: "http.console.frontstage.pages.patch.v1",
        method: "PATCH",
        path: "/api/console/frontstage/pages/:page_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.pages.move",
        binding_id: "http.console.frontstage.pages.move.v1",
        method: "POST",
        path: "/api/console/frontstage/pages/:page_id/move",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.pages.delete",
        binding_id: "http.console.frontstage.pages.delete.v1",
        method: "DELETE",
        path: "/api/console/frontstage/pages/:page_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.tabs.view",
        binding_id: "http.console.frontstage.tabs.get.v1",
        method: "GET",
        path: "/api/console/frontstage/pages/:page_id/tabs",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.tabs.create",
        binding_id: "http.console.frontstage.tabs.post.v1",
        method: "POST",
        path: "/api/console/frontstage/pages/:page_id/tabs",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.tabs.update",
        binding_id: "http.console.frontstage.tabs.patch.v1",
        method: "PATCH",
        path: "/api/console/frontstage/pages/:page_id/tabs/:tab_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.tabs.delete",
        binding_id: "http.console.frontstage.tabs.delete.v1",
        method: "DELETE",
        path: "/api/console/frontstage/pages/:page_id/tabs/:tab_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.tabs.document.save",
        binding_id: "http.console.frontstage.tabs.document.put.v1",
        method: "PUT",
        path: "/api/console/frontstage/pages/:page_id/tabs/:tab_id/document",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.ui_templates.view",
        binding_id: "http.console.frontstage.ui-templates.get.v1",
        method: "GET",
        path: "/api/console/frontstage/ui-templates",
        mutating: false,
    },
];

pub(crate) fn compile_registry(
    dependencies: FrontstagePagesDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-frontstage-pages",
        "graph:console-frontstage-pages-v1",
        DECLARATIONS,
        Arc::new(FrontstagePagesAdapter(dependencies)),
    )
}
