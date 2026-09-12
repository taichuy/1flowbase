use std::sync::Arc;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use control_plane::ports::CacheStore;
use interface_runtime::{InterfaceContract, UserPrincipal};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use storage_durable_postgres::MainDurableStore;
use time::Duration;
use uuid::Uuid;

use super::{
    assistant_preference_for_actor,
    websocket::{AssistantWebSocketTicketResponse, CreateAssistantWebSocketTicketBody},
};
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

pub(super) const ASSISTANT_WEBSOCKET_PROTOCOL: &str = "1flowbase.assistant.v1";
pub(super) const ASSISTANT_WEBSOCKET_TICKET_PREFIX: &str = "1flowbase.assistant.ticket.";
pub(super) const ASSISTANT_WEBSOCKET_TICKET_TTL_SECONDS: i64 = 60;

pub(crate) enum AssistantWebSocketTicketInput {
    Create {
        body: CreateAssistantWebSocketTicketBody,
        origin: Option<String>,
    },
}

impl InterfaceContract for AssistantWebSocketTicketInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![mp::object_schema(&[
            ("variant", mp::tag_schema("Create")),
            (
                "body",
                mp::object_schema(&[("application_id", mp::text_schema())]),
            ),
        ])]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Create {
                body: _field_body, ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Create".to_owned())),
                (
                    "body",
                    mp::object_value(&[(
                        "application_id",
                        serde_json::Value::String((&(_field_body).application_id).to_string()),
                    )]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-assistant-websocket-ticket-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum AssistantWebSocketTicketOutput {
    Ticket(AssistantWebSocketTicketResponse),
}

impl InterfaceContract for AssistantWebSocketTicketOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![mp::object_schema(&[
            ("variant", mp::tag_schema("Ticket")),
            (
                "0",
                mp::object_schema(&[
                    (
                        "ticket",
                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                    ),
                    ("protocol", mp::text_schema()),
                    ("expires_in_seconds", serde_json::json!({"type":"integer"})),
                ]),
            ),
        ])]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Ticket(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Ticket".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "ticket",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).ticket).len()),
                            )]),
                        ),
                        ("protocol", mp::text(&(_field_0).protocol)?),
                        (
                            "expires_in_seconds",
                            serde_json::json!(*(&(_field_0).expires_in_seconds)),
                        ),
                    ]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-assistant-websocket-ticket-output";
    const CONTRACT_VERSION: &'static str = "1";
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct AssistantWebSocketTicket {
    pub(super) user_id: Uuid,
    pub(super) workspace_id: Uuid,
    pub(super) application_id: Uuid,
    pub(super) origin: String,
}

struct AssistantWebSocketTicketAdapter {
    store: MainDurableStore,
    cache: Arc<dyn CacheStore>,
}

pub(crate) fn websocket_ticket_port(
    store: MainDurableStore,
    cache: Arc<dyn CacheStore>,
) -> Arc<dyn ConsoleInterfacePort<AssistantWebSocketTicketInput, AssistantWebSocketTicketOutput>> {
    Arc::new(AssistantWebSocketTicketAdapter { store, cache })
}

impl AssistantWebSocketTicketAdapter {
    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: AssistantWebSocketTicketInput,
    ) -> Result<AssistantWebSocketTicketOutput, ApiError> {
        let actor = principal.actor();
        match input {
            AssistantWebSocketTicketInput::Create { body, origin } => {
                assistant_preference_for_actor(&self.store, actor, body.application_id).await?;
                let origin = required_origin(origin)?;
                let ticket = AssistantWebSocketTicket {
                    user_id: actor.user_id,
                    workspace_id: actor.current_workspace_id,
                    application_id: body.application_id,
                    origin,
                };
                let token = store_ticket(self.cache.as_ref(), &ticket).await?;
                Ok(AssistantWebSocketTicketOutput::Ticket(
                    AssistantWebSocketTicketResponse {
                        ticket: token,
                        protocol: ASSISTANT_WEBSOCKET_PROTOCOL,
                        expires_in_seconds: ASSISTANT_WEBSOCKET_TICKET_TTL_SECONDS,
                    },
                ))
            }
        }
    }
}

impl ConsoleInterfacePort<AssistantWebSocketTicketInput, AssistantWebSocketTicketOutput>
    for AssistantWebSocketTicketAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: AssistantWebSocketTicketInput,
    ) -> ConsoleInterfaceFuture<'a, AssistantWebSocketTicketOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[ConsoleInterfaceDeclaration {
    interface_id: "assistant.runs.websocket-ticket.create",
    binding_id: "http.console.assistant.runs.websocket-ticket.create.v1",
    method: "POST",
    path: "/api/console/assistant/runs/websocket-ticket",
    mutating: true,
}];

pub(crate) fn compile_registry(
    store: MainDurableStore,
    cache: Arc<dyn CacheStore>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    compile_registry_with_port(websocket_ticket_port(store, cache))
}

fn compile_registry_with_port(
    port: Arc<
        dyn ConsoleInterfacePort<AssistantWebSocketTicketInput, AssistantWebSocketTicketOutput>,
    >,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-assistant-websocket-ticket",
        "graph:console-assistant-websocket-ticket-v1",
        DECLARATIONS,
        port,
    )
}

pub(super) async fn store_ticket(
    cache: &dyn CacheStore,
    ticket: &AssistantWebSocketTicket,
) -> Result<String, ApiError> {
    for _ in 0..3 {
        let mut bytes = [0_u8; 32];
        OsRng.fill_bytes(&mut bytes);
        let token = URL_SAFE_NO_PAD.encode(bytes);
        if cache
            .set_if_absent_json(
                &ticket_key(&token),
                serde_json::to_value(ticket)?,
                Some(Duration::seconds(ASSISTANT_WEBSOCKET_TICKET_TTL_SECONDS)),
            )
            .await?
        {
            return Ok(token);
        }
    }
    Err(control_plane::errors::ControlPlaneError::Conflict("assistant_websocket_ticket").into())
}

fn ticket_key(token: &str) -> String {
    format!(
        "assistant-websocket-ticket:{}",
        control_plane::auth::hash_api_key_token(token)
    )
}

fn required_origin(origin: Option<String>) -> Result<String, ApiError> {
    origin
        .filter(|origin| !origin.is_empty() && origin != "null")
        .ok_or_else(|| {
            control_plane::errors::ControlPlaneError::PermissionDenied("assistant_websocket_origin")
                .into()
        })
}

#[cfg(test)]
struct UnavailableAssistantWebSocketTicketPort;

#[cfg(test)]
impl ConsoleInterfacePort<AssistantWebSocketTicketInput, AssistantWebSocketTicketOutput>
    for UnavailableAssistantWebSocketTicketPort
{
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: AssistantWebSocketTicketInput,
    ) -> ConsoleInterfaceFuture<'a, AssistantWebSocketTicketOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("assistant websocket ticket fixture unavailable").into(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use interface_runtime::BindingId;

    use super::*;

    #[test]
    fn f09b1c_registry_freezes_assistant_websocket_ticket_binding() {
        let registry =
            compile_registry_with_port(Arc::new(UnavailableAssistantWebSocketTicketPort)).unwrap();
        for declaration in DECLARATIONS {
            assert!(registry
                .binding(&BindingId::new(declaration.binding_id).unwrap())
                .is_some());
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }
}
