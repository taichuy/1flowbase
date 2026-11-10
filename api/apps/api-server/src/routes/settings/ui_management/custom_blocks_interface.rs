use super::custom_blocks::{CreateBlockBody, CustomBlockResponse, UpdateBlockBody};
use crate::{
    error_response::ApiError,
    openapi_interface::json_contract::{JsonApiInput, JsonApiOutput},
    routes::{
        console_interface::{
            self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        },
        frontend_block_catalog::FrontendBlockDependencies,
    },
};
use control_plane::frontend_block_catalog::{
    custom_blocks::CustomFrontendBlockService, FrontendBlockCatalogService,
    ListFrontendBlockCatalogQuery,
};
use control_plane_contracts::ports::custom_frontend_blocks::CustomFrontendBlockBody;
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
#[serde(untagged)]
pub(crate) enum Input {
    List,
    #[serde(serialize_with = "crate::openapi_interface::json_contract::body")]
    Create(CreateBlockBody),
    Update {
        id: String,
        body: UpdateBlockBody,
    },
    Delete {
        id: String,
    },
}
impl JsonApiInput for Input {}
impl InterfaceContract for Input {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema([("variant", mp::tag_schema("List"))]),
            mp::object_schema([
                ("variant", mp::tag_schema("Create")),
                ("provider_code", mp::text_schema()),
                ("contribution_code", mp::text_schema()),
                ("body", body_schema()),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("Update")),
                ("id", mp::text_schema()),
                ("body", body_schema()),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("Delete")),
                ("id", mp::text_schema()),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::List => mp::object_value([("variant", serde_json::json!("List"))]),
            Self::Create(body) => mp::object_value([
                ("variant", serde_json::json!("Create")),
                ("provider_code", mp::text(&body.provider_code)?),
                ("contribution_code", mp::text(&body.contribution_code)?),
                (
                    "body",
                    body_projection(&body.title, &body.source, &body.language)?,
                ),
            ]),
            Self::Update { id, body } => mp::object_value([
                ("variant", serde_json::json!("Update")),
                ("id", mp::text(id)?),
                (
                    "body",
                    body_projection(&body.title, &body.source, &body.language)?,
                ),
            ]),
            Self::Delete { id } => mp::object_value([
                ("variant", serde_json::json!("Delete")),
                ("id", mp::text(id)?),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-custom-frontend-blocks-input";
    const CONTRACT_VERSION: &'static str = "1";
}
#[derive(Serialize)]
#[serde(untagged)]
pub(crate) enum Output {
    List(Vec<CustomBlockResponse>),
    Record(CustomBlockResponse),
    Empty,
}
impl JsonApiOutput for Output {}
impl InterfaceContract for Output {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema([
                ("variant", mp::tag_schema("List")),
                ("item_count", mp::count_schema()),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("Record")),
                ("id", mp::text_schema()),
                ("workspace_id", mp::text_schema()),
                ("provider_code", mp::text_schema()),
                ("contribution_code", mp::text_schema()),
                ("body", body_schema()),
                ("revision", serde_json::json!({"type":"integer"})),
            ]),
            mp::object_schema([("variant", mp::tag_schema("Empty"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::List(rows) => mp::object_value([
                ("variant", serde_json::json!("List")),
                ("item_count", serde_json::json!(rows.len())),
            ]),
            Self::Record(row) => mp::object_value([
                ("variant", serde_json::json!("Record")),
                ("id", mp::text(&row.id)?),
                ("workspace_id", mp::text(&row.workspace_id)?),
                ("provider_code", mp::text(&row.provider_code)?),
                ("contribution_code", mp::text(&row.contribution_code)?),
                (
                    "body",
                    body_projection(&row.title, &row.source, &row.language)?,
                ),
                ("revision", serde_json::json!(row.revision)),
            ]),
            Self::Empty => mp::object_value([("variant", serde_json::json!("Empty"))]),
        })
    }

    const CONTRACT_ID: &'static str = "console-custom-frontend-blocks-output";
    const CONTRACT_VERSION: &'static str = "1";
}
struct Adapter(FrontendBlockDependencies);
impl ConsoleInterfacePort<Input, Output> for Adapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: Input,
    ) -> ConsoleInterfaceFuture<'a, Output> {
        Box::pin(async move {
            let actor = principal.actor();
            let service = CustomFrontendBlockService(self.0.store.for_actor(actor.clone()));
            let result: Result<Output, ApiError> = async {
                match input {
                    Input::List => Ok(Output::List(
                        service
                            .list(actor.current_workspace_id)
                            .await?
                            .into_iter()
                            .map(Into::into)
                            .collect(),
                    )),
                    Input::Create(body) => {
                        let catalog = FrontendBlockCatalogService::new(
                            self.0.store.for_actor(actor.clone()),
                            self.0.api_node_id.clone(),
                            Arc::clone(&self.0.graph),
                        )?
                        .list_frontend_blocks(ListFrontendBlockCatalogQuery {
                            actor_user_id: actor.user_id,
                        })
                        .await?;
                        let value = service
                            .create(
                                actor.current_workspace_id,
                                &body.provider_code,
                                &body.contribution_code,
                                CustomFrontendBlockBody {
                                    title: body.title,
                                    source: body.source,
                                    language: body.language,
                                },
                                actor.user_id,
                                &catalog.entries,
                            )
                            .await?;
                        Ok(Output::Record(value.into()))
                    }
                    Input::Update { id, body } => {
                        let id = uuid::Uuid::parse_str(&id).map_err(|_| {
                            control_plane::errors::ControlPlaneError::InvalidInput(
                                "invalid_custom_block_id",
                            )
                        })?;
                        Ok(Output::Record(
                            service
                                .update(
                                    actor.current_workspace_id,
                                    id,
                                    CustomFrontendBlockBody {
                                        title: body.title,
                                        source: body.source,
                                        language: body.language,
                                    },
                                    actor.user_id,
                                )
                                .await?
                                .into(),
                        ))
                    }
                    Input::Delete { id } => {
                        let id = uuid::Uuid::parse_str(&id).map_err(|_| {
                            control_plane::errors::ControlPlaneError::InvalidInput(
                                "invalid_custom_block_id",
                            )
                        })?;
                        service.delete(actor.current_workspace_id, id).await?;
                        Ok(Output::Empty)
                    }
                }
            }
            .await;
            result.map_err(console_interface::ConsoleInterfaceTargetError)
        })
    }
}
pub(crate) fn compile_registry(
    dependencies: FrontendBlockDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-custom-frontend-blocks",
        "graph:console-custom-frontend-blocks-v1",
        &[
            ConsoleInterfaceDeclaration {
                interface_id: "ui_management.blocks.list",
                binding_id: "http.console.ui-management.blocks.list.get.v1",
                method: "GET",
                path: "/api/console/settings/ui-management/blocks",
                mutating: false,
            },
            ConsoleInterfaceDeclaration {
                interface_id: "ui_management.blocks.create",
                binding_id: "http.console.ui-management.blocks.create.post.v1",
                method: "POST",
                path: "/api/console/settings/ui-management/blocks",
                mutating: true,
            },
            ConsoleInterfaceDeclaration {
                interface_id: "ui_management.blocks.update",
                binding_id: "http.console.ui-management.blocks.update.put.v1",
                method: "PUT",
                path: "/api/console/settings/ui-management/blocks/:id",
                mutating: true,
            },
            ConsoleInterfaceDeclaration {
                interface_id: "ui_management.blocks.delete",
                binding_id: "http.console.ui-management.blocks.delete.delete.v1",
                method: "DELETE",
                path: "/api/console/settings/ui-management/blocks/:id",
                mutating: true,
            },
        ],
        Arc::new(Adapter(dependencies)),
    )
}

fn body_schema() -> serde_json::Value {
    use crate::extension_bus::managed_projection as mp;
    mp::object_schema([
        (
            "title",
            mp::object_schema([("byte_count", mp::count_schema())]),
        ),
        (
            "source",
            mp::object_schema([("byte_count", mp::count_schema())]),
        ),
        ("language", mp::text_schema()),
    ])
}
fn body_projection(title: &str, source: &str, language: &str) -> Option<serde_json::Value> {
    use crate::extension_bus::managed_projection as mp;
    Some(mp::object_value([
        (
            "title",
            mp::object_value([("byte_count", serde_json::json!(title.len()))]),
        ),
        (
            "source",
            mp::object_value([("byte_count", serde_json::json!(source.len()))]),
        ),
        ("language", mp::text(language)?),
    ]))
}
