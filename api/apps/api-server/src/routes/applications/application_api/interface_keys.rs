use std::sync::Arc;

use control_plane::application_public_api::api_keys::{
    ApplicationApiKeyService, CreateApplicationApiKeyCommand, ListApplicationApiKeysCommand,
    RevokeApplicationApiKeyCommand,
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::{
    map_application_api_key_not_found, parse_expires_at, to_api_key_response,
    to_created_api_key_response, ApplicationApiKeyResponse, CreateApplicationApiKeyBody,
    CreatedApplicationApiKeyResponse,
};
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

pub(crate) enum ApplicationApiKeyInput {
    List {
        application_id: Uuid,
    },
    Create {
        application_id: Uuid,
        body: CreateApplicationApiKeyBody,
    },
    Revoke {
        application_id: Uuid,
        key_id: Uuid,
    },
}

impl InterfaceContract for ApplicationApiKeyInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema([
                ("variant", mp::tag_schema("List")),
                ("application_id", mp::text_schema()),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("Create")),
                ("application_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema([
                        (
                            "name",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "expires_at",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("Revoke")),
                ("application_id", mp::text_schema()),
                ("key_id", mp::text_schema()),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::List {
                application_id: _field_application_id,
                ..
            } => mp::object_value([
                ("variant", serde_json::Value::String("List".to_owned())),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
            ]),
            Self::Create {
                application_id: _field_application_id,
                body: _field_body,
                ..
            } => mp::object_value([
                ("variant", serde_json::Value::String("Create".to_owned())),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "body",
                    mp::object_value([
                        (
                            "name",
                            mp::object_value([(
                                "byte_count",
                                serde_json::json!((_field_body).name.len()),
                            )]),
                        ),
                        (
                            "expires_at",
                            match (_field_body).expires_at.as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::Revoke {
                application_id: _field_application_id,
                key_id: _field_key_id,
                ..
            } => mp::object_value([
                ("variant", serde_json::Value::String("Revoke".to_owned())),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "key_id",
                    serde_json::Value::String((_field_key_id).to_string()),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-application-api-key-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum ApplicationApiKeyOutput {
    List(Vec<ApplicationApiKeyResponse>),
    Created(CreatedApplicationApiKeyResponse),
    NoContent,
}

impl ApplicationApiKeyOutput {
    pub(super) fn into_list(self) -> Result<Vec<ApplicationApiKeyResponse>, ApiError> {
        match self {
            Self::List(value) => Ok(value),
            _ => Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "application_api_key_output",
            )
            .into()),
        }
    }

    pub(super) fn into_created(self) -> Result<CreatedApplicationApiKeyResponse, ApiError> {
        match self {
            Self::Created(value) => Ok(value),
            _ => Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "application_api_key_output",
            )
            .into()),
        }
    }
}

impl InterfaceContract for ApplicationApiKeyOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema([
                ("variant", mp::tag_schema("List")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("id",mp::text_schema()), ("name",mp::object_schema([("byte_count",mp::count_schema())])), ("creator_user_id",mp::text_schema()), ("enabled",serde_json::json!({"type":"boolean"})), ("expires_at",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("last_used_at",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema())])}),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("Created")),
                (
                    "0",
                    mp::object_schema([
                        ("id", mp::text_schema()),
                        (
                            "name",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        ("creator_user_id", mp::text_schema()),
                        ("enabled", serde_json::json!({"type":"boolean"})),
                        (
                            "expires_at",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "last_used_at",
                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("created_at", mp::text_schema()),
                        ("updated_at", mp::text_schema()),
                    ]),
                ),
            ]),
            mp::object_schema([("variant", mp::tag_schema("NoContent"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::List(_field_0) => mp::object_value([
                ("variant", serde_json::Value::String("List".to_owned())),
                ("0", {
                    if (_field_0).len() > 32 {
                        return None;
                    }
                    serde_json::Value::Array(
                        (_field_0)
                            .iter()
                            .map(|item| {
                                Some(mp::object_value([
                                    ("id", serde_json::Value::String((item).id.to_string())),
                                    (
                                        "name",
                                        mp::object_value([(
                                            "byte_count",
                                            serde_json::json!((item).name.len()),
                                        )]),
                                    ),
                                    (
                                        "creator_user_id",
                                        serde_json::Value::String(
                                            (item).creator_user_id.to_string(),
                                        ),
                                    ),
                                    ("enabled", serde_json::Value::Bool((item).enabled)),
                                    (
                                        "expires_at",
                                        match (item).expires_at.as_ref() {
                                            Some(item) => mp::text(item)?,
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "last_used_at",
                                        match (item).last_used_at.as_ref() {
                                            Some(item) => mp::object_value([(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    ("created_at", mp::text(&(item).created_at)?),
                                    ("updated_at", mp::text(&(item).updated_at)?),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
            ]),
            Self::Created(_field_0) => mp::object_value([
                ("variant", serde_json::Value::String("Created".to_owned())),
                (
                    "0",
                    mp::object_value([
                        ("id", serde_json::Value::String((_field_0).id.to_string())),
                        (
                            "name",
                            mp::object_value([(
                                "byte_count",
                                serde_json::json!((_field_0).name.len()),
                            )]),
                        ),
                        (
                            "creator_user_id",
                            serde_json::Value::String((_field_0).creator_user_id.to_string()),
                        ),
                        ("enabled", serde_json::Value::Bool((_field_0).enabled)),
                        (
                            "expires_at",
                            match (_field_0).expires_at.as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "last_used_at",
                            match (_field_0).last_used_at.as_ref() {
                                Some(item) => mp::object_value([(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("created_at", mp::text(&(_field_0).created_at)?),
                        ("updated_at", mp::text(&(_field_0).updated_at)?),
                    ]),
                ),
            ]),
            Self::NoContent => {
                mp::object_value([("variant", serde_json::Value::String("NoContent".to_owned()))])
            }
        })
    }

    const CONTRACT_ID: &'static str = "console-application-api-key-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct ApplicationApiKeyAdapter {
    store: MainDurableStore,
}

pub(crate) fn port(
    store: MainDurableStore,
) -> Arc<dyn ConsoleInterfacePort<ApplicationApiKeyInput, ApplicationApiKeyOutput>> {
    Arc::new(ApplicationApiKeyAdapter { store })
}

impl ConsoleInterfacePort<ApplicationApiKeyInput, ApplicationApiKeyOutput>
    for ApplicationApiKeyAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ApplicationApiKeyInput,
    ) -> ConsoleInterfaceFuture<'a, ApplicationApiKeyOutput> {
        Box::pin(async move {
            let result: Result<ApplicationApiKeyOutput, ApiError> = async {
                let actor = principal.actor();
                let service = ApplicationApiKeyService::new(self.store.for_actor(actor.clone()));
                let output = match input {
                    ApplicationApiKeyInput::List { application_id } => {
                        let values = service
                            .list_api_keys(ListApplicationApiKeysCommand {
                                actor_user_id: actor.user_id,
                                application_id,
                            })
                            .await?;
                        ApplicationApiKeyOutput::List(
                            values
                                .into_iter()
                                .map(to_api_key_response)
                                .collect::<Result<Vec<_>, _>>()?,
                        )
                    }
                    ApplicationApiKeyInput::Create {
                        application_id,
                        body,
                    } => {
                        let result = service
                            .create_api_key(CreateApplicationApiKeyCommand {
                                actor_user_id: actor.user_id,
                                application_id,
                                name: body.name,
                                expires_at: parse_expires_at(body.expires_at)?,
                            })
                            .await?;
                        ApplicationApiKeyOutput::Created(to_created_api_key_response(
                            result.api_key,
                            result.token,
                        )?)
                    }
                    ApplicationApiKeyInput::Revoke {
                        application_id,
                        key_id,
                    } => {
                        service
                            .revoke_api_key(RevokeApplicationApiKeyCommand {
                                actor_user_id: actor.user_id,
                                application_id,
                                api_key_id: key_id,
                            })
                            .await
                            .map_err(map_application_api_key_not_found)?;
                        ApplicationApiKeyOutput::NoContent
                    }
                };
                Ok(output)
            }
            .await;
            result.map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "applications.api-keys.list",
        binding_id: "http.console.applications.api-keys.list.v1",
        method: "GET",
        path: "/api/console/applications/:application_id/api-keys",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.api-keys.create",
        binding_id: "http.console.applications.api-keys.create.v1",
        method: "POST",
        path: "/api/console/applications/:application_id/api-keys",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.api-keys.revoke",
        binding_id: "http.console.applications.api-keys.revoke.v1",
        method: "DELETE",
        path: "/api/console/applications/:application_id/api-keys/:key_id",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    port: Arc<dyn ConsoleInterfacePort<ApplicationApiKeyInput, ApplicationApiKeyOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-application-api-keys",
        "api-server.console-application-api-keys.graph.v1",
        DECLARATIONS,
        port,
    )
}

impl crate::event_center::api_projection::ApiEventInput for ApplicationApiKeyInput {
    fn project_document_input(
        &self,
        operation: &crate::event_center::api_projection::ApiEventOperationContext,
    ) -> Result<serde_json::Value, std::sync::Arc<str>> {
        Ok(match self {
            Self::List { application_id } => {
                crate::routes::applications_group::event_projection::request(
                    operation,
                    serde_json::json!({"application_id":application_id}),
                    serde_json::json!({}),
                    serde_json::Value::Null,
                )
            }
            Self::Create {
                application_id,
                body,
            } => crate::routes::applications_group::event_projection::request(
                operation,
                serde_json::json!({"application_id":application_id}),
                serde_json::json!({}),
                crate::event_center::api_projection::to_value(body)?,
            ),
            Self::Revoke {
                application_id,
                key_id,
            } => crate::routes::applications_group::event_projection::request(
                operation,
                serde_json::json!({"application_id":application_id,"key_id":key_id}),
                serde_json::json!({}),
                serde_json::Value::Null,
            ),
        })
    }
}

impl crate::event_center::api_projection::ApiEventOutput for ApplicationApiKeyOutput {
    fn document_output_schema(
        operation: &crate::event_center::api_projection::ApiEventOperationContext,
    ) -> Option<serde_json::Value> {
        (operation.interface_id == "applications.api-keys.create").then(|| {
            let mut schema =
                crate::event_center::api_projection::schema::<super::ApplicationApiKeyResponse>();
            if let Some(properties) = schema["properties"].as_object_mut() {
                properties.remove("token_prefix");
            }
            if let Some(required) = schema["required"].as_array_mut() {
                required.retain(|field| field != "token_prefix");
            }
            schema
        })
    }
    fn project_document_output(
        &self,
        _operation: &crate::event_center::api_projection::ApiEventOperationContext,
    ) -> Result<serde_json::Value, std::sync::Arc<str>> {
        match self {
            Self::List(value) => crate::event_center::api_projection::to_value(value),
            Self::Created(value) => {
                // Build the real DTO before selecting its credential-free event view.
                let mut document = crate::event_center::api_projection::to_value(
                    &super::ApplicationApiKeyResponse {
                        id: value.id,
                        name: value.name.clone(),
                        token_prefix: value.token_prefix.clone(),
                        creator_user_id: value.creator_user_id,
                        enabled: value.enabled,
                        expires_at: value.expires_at.clone(),
                        last_used_at: value.last_used_at.clone(),
                        created_at: value.created_at.clone(),
                        updated_at: value.updated_at.clone(),
                    },
                )?;
                document
                    .as_object_mut()
                    .expect("API key response is an object")
                    .remove("token_prefix");
                Ok(document)
            }
            Self::NoContent => Ok(serde_json::Value::Null),
        }
    }
}
