use std::sync::Arc;

use control_plane::{
    application_public_api::run_service::{
        ApplicationPublishedFlowRunRepository, ListAssistantConversationsInput,
    },
    orchestration_runtime::{CancelFlowRunCommand, OrchestrationRuntimeService},
    ports::OrchestrationRuntimeRepository,
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde_json::Value;
use tokio::sync::broadcast;
use uuid::Uuid;

use super::{
    abort_assistant_execution_in, launch_assistant_execution, prepare_assistant_execution,
    AssistantClientToolBridge, AssistantClientToolId, AssistantConversationPageResponse,
    AssistantRunDependencies, StartAssistantRunBody,
};
use super::{
    conversation_events::{
        AssistantConversationEvent, AssistantConversationEventScope,
        AssistantConversationSummaryResponse,
    },
    websocket::enabled_client_tools_for_connection,
};
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

pub(crate) const BINDING_ID: &str = "ws.console.assistant.runs.command.v1";

pub(crate) enum AssistantWebSocketCommandInput {
    SubscribeConversations {
        application_id: Uuid,
    },
    Create {
        application_id: Uuid,
        request: StartAssistantRunBody,
        request_headers: axum::http::HeaderMap,
        client_tool_ids: Vec<AssistantClientToolId>,
        client_tool_bridge: AssistantClientToolBridge,
    },
    Attach {
        application_id: Uuid,
        run_id: Uuid,
        after_event_id: Option<String>,
        client_tool_ids: Vec<AssistantClientToolId>,
        client_tool_bridge: AssistantClientToolBridge,
    },
    Cancel {
        application_id: Uuid,
        run_id: Uuid,
    },
    ClientToolResult {
        connection_id: Uuid,
        call_id: Uuid,
        result: Value,
        is_error: bool,
    },
}

impl InterfaceContract for AssistantWebSocketCommandInput {
    const CONTRACT_ID: &'static str = "console-assistant-websocket-command-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum AssistantWebSocketCommandOutput {
    ConversationSubscription(AssistantConversationSubscription),
    Run(AssistantWebSocketRun),
    Cancelled,
    ClientToolResult { completed: bool },
}

impl InterfaceContract for AssistantWebSocketCommandOutput {
    const CONTRACT_ID: &'static str = "console-assistant-websocket-command-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct AssistantConversationSubscription {
    pub(crate) snapshot: AssistantConversationPageResponse,
    pub(crate) events: broadcast::Receiver<AssistantConversationEvent>,
}

pub(crate) struct AssistantWebSocketRun {
    pub(crate) run_id: Uuid,
    pub(crate) from_sequence: Option<i64>,
}

struct AssistantWebSocketCommandAdapter {
    dependencies: AssistantRunDependencies,
}

pub(crate) fn command_port(
    dependencies: AssistantRunDependencies,
) -> Arc<dyn ConsoleInterfacePort<AssistantWebSocketCommandInput, AssistantWebSocketCommandOutput>>
{
    Arc::new(AssistantWebSocketCommandAdapter { dependencies })
}

impl AssistantWebSocketCommandAdapter {
    async fn subscribe_conversations(
        &self,
        principal: &UserPrincipal,
        application_id: Uuid,
    ) -> Result<AssistantConversationSubscription, ApiError> {
        let actor = principal.actor();
        let scope = AssistantConversationEventScope {
            workspace_id: actor.current_workspace_id,
            application_id,
            actor_user_id: actor.user_id,
        };
        let page = self
            .dependencies
            .store
            .list_assistant_conversations(&ListAssistantConversationsInput {
                workspace_id: scope.workspace_id,
                application_id: scope.application_id,
                actor_user_id: scope.actor_user_id,
                page: 1,
                page_size: 20,
            })
            .await?;
        Ok(AssistantConversationSubscription {
            snapshot: AssistantConversationPageResponse {
                items: page
                    .items
                    .into_iter()
                    .map(AssistantConversationSummaryResponse::from)
                    .collect(),
                total: page.total,
                page: page.page,
                page_size: page.page_size,
            },
            events: self.dependencies.conversation_events.subscribe(scope),
        })
    }

    async fn create_run(
        &self,
        principal: &UserPrincipal,
        application_id: Uuid,
        request: StartAssistantRunBody,
        request_headers: axum::http::HeaderMap,
        client_tool_ids: Vec<AssistantClientToolId>,
        client_tool_bridge: AssistantClientToolBridge,
    ) -> Result<AssistantWebSocketRun, ApiError> {
        if request.application_id != application_id {
            return Err(control_plane::errors::ControlPlaneError::PermissionDenied(
                "assistant_application_id",
            )
            .into());
        }
        let execution = prepare_assistant_execution(
            &self.dependencies.store,
            &request_headers,
            principal.actor(),
            request,
        )
        .await?;
        let enabled_client_tools =
            enabled_client_tools_for_connection(&execution.enabled_client_tools, &client_tool_ids);
        tracing::debug!(
            application_id = %application_id,
            declared_client_tools = ?client_tool_ids,
            preferred_client_tools = ?execution.enabled_client_tools,
            enabled_client_tools = ?enabled_client_tools,
            "Assistant WebSocket client tool selection"
        );
        let client_tool_bridge = Arc::new(
            client_tool_bridge
                .for_tools(enabled_client_tools, client_tool_ids)
                .await,
        );
        let run_id = launch_assistant_execution(
            self.dependencies.clone(),
            execution,
            Some(client_tool_bridge),
        )
        .await?;
        Ok(AssistantWebSocketRun {
            run_id,
            from_sequence: None,
        })
    }

    async fn attach_run(
        &self,
        principal: &UserPrincipal,
        application_id: Uuid,
        run_id: Uuid,
        after_event_id: Option<String>,
        client_tool_ids: Vec<AssistantClientToolId>,
        client_tool_bridge: AssistantClientToolBridge,
    ) -> Result<AssistantWebSocketRun, ApiError> {
        let actor = principal.actor();
        let run = self
            .dependencies
            .store
            .get_flow_run(application_id, run_id)
            .await?
            .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                "flow_run",
            ))?;
        if run.created_by != actor.user_id {
            return Err(control_plane::errors::ControlPlaneError::PermissionDenied(
                "assistant_run",
            )
            .into());
        }
        let from_sequence = crate::routes::application_public_api::native_websocket::schema::sequence_from_event_id(
            run_id,
            after_event_id.as_deref(),
        )
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("event_id"))?;
        let session = self
            .dependencies
            .assistant_client_sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&run_id)
            .cloned()
            .ok_or(control_plane::errors::ControlPlaneError::Conflict(
                "assistant_client_session",
            ))?;
        session
            .replace_connection(&client_tool_bridge, client_tool_ids)
            .await;
        Ok(AssistantWebSocketRun {
            run_id,
            from_sequence,
        })
    }

    async fn cancel_run(
        &self,
        principal: &UserPrincipal,
        application_id: Uuid,
        run_id: Uuid,
    ) -> Result<(), ApiError> {
        let actor = principal.actor();
        let run = self
            .dependencies
            .store
            .get_flow_run(application_id, run_id)
            .await?
            .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                "flow_run",
            ))?;
        if run.created_by != actor.user_id {
            return Err(control_plane::errors::ControlPlaneError::PermissionDenied(
                "assistant_run",
            )
            .into());
        }
        abort_assistant_execution_in(&self.dependencies.assistant_executions, run_id);
        let runtime = OrchestrationRuntimeService::new(
            self.dependencies.store.clone(),
            crate::provider_runtime::ApiProviderRuntime::new_with_activity(
                self.dependencies.provider_runtime.clone(),
                self.dependencies.runtime_activity.clone(),
            ),
            self.dependencies.runtime_engine.clone(),
            self.dependencies.provider_secret_master_key.clone(),
        )
        .with_node_artifact_context(
            self.dependencies.api_node_id.clone(),
            self.dependencies.provider_install_root.clone(),
        )
        .with_file_storage_registry(self.dependencies.file_storage_registry.clone())
        .with_llm_routing_counter_store(self.dependencies.cache_store.clone())
        .with_provider_request_log_queue(self.dependencies.task_queue.clone())
        .with_runtime_event_stream(self.dependencies.runtime_event_stream.clone());
        runtime
            .cancel_flow_run(CancelFlowRunCommand {
                actor_user_id: actor.user_id,
                application_id,
                flow_run_id: run_id,
            })
            .await?;
        Ok(())
    }

    async fn complete_client_tool_result(
        &self,
        connection_id: Uuid,
        call_id: Uuid,
        result: Value,
        is_error: bool,
    ) -> bool {
        let sessions = self
            .dependencies
            .assistant_client_sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for session in sessions {
            if session
                .complete_for_connection(connection_id, call_id, result.clone(), is_error)
                .await
            {
                return true;
            }
        }
        false
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: AssistantWebSocketCommandInput,
    ) -> Result<AssistantWebSocketCommandOutput, ApiError> {
        match input {
            AssistantWebSocketCommandInput::SubscribeConversations { application_id } => {
                Ok(AssistantWebSocketCommandOutput::ConversationSubscription(
                    self.subscribe_conversations(principal, application_id)
                        .await?,
                ))
            }
            AssistantWebSocketCommandInput::Create {
                application_id,
                request,
                request_headers,
                client_tool_ids,
                client_tool_bridge,
            } => Ok(AssistantWebSocketCommandOutput::Run(
                self.create_run(
                    principal,
                    application_id,
                    request,
                    request_headers,
                    client_tool_ids,
                    client_tool_bridge,
                )
                .await?,
            )),
            AssistantWebSocketCommandInput::Attach {
                application_id,
                run_id,
                after_event_id,
                client_tool_ids,
                client_tool_bridge,
            } => Ok(AssistantWebSocketCommandOutput::Run(
                self.attach_run(
                    principal,
                    application_id,
                    run_id,
                    after_event_id,
                    client_tool_ids,
                    client_tool_bridge,
                )
                .await?,
            )),
            AssistantWebSocketCommandInput::Cancel {
                application_id,
                run_id,
            } => {
                self.cancel_run(principal, application_id, run_id).await?;
                Ok(AssistantWebSocketCommandOutput::Cancelled)
            }
            AssistantWebSocketCommandInput::ClientToolResult {
                connection_id,
                call_id,
                result,
                is_error,
            } => Ok(AssistantWebSocketCommandOutput::ClientToolResult {
                completed: self
                    .complete_client_tool_result(connection_id, call_id, result, is_error)
                    .await,
            }),
        }
    }
}

impl ConsoleInterfacePort<AssistantWebSocketCommandInput, AssistantWebSocketCommandOutput>
    for AssistantWebSocketCommandAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: AssistantWebSocketCommandInput,
    ) -> ConsoleInterfaceFuture<'a, AssistantWebSocketCommandOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[ConsoleInterfaceDeclaration {
    interface_id: "assistant.runs.websocket.command",
    binding_id: BINDING_ID,
    method: "GET",
    path: "/api/console/assistant/runs/websocket",
    mutating: true,
}];

pub(crate) fn compile_registry(
    dependencies: AssistantRunDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    compile_registry_with_port(command_port(dependencies))
}

fn compile_registry_with_port(
    port: Arc<
        dyn ConsoleInterfacePort<AssistantWebSocketCommandInput, AssistantWebSocketCommandOutput>,
    >,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-assistant-websocket-commands",
        "graph:console-assistant-websocket-commands-v1",
        DECLARATIONS,
        port,
    )
}

#[cfg(test)]
struct UnavailableAssistantWebSocketCommandPort;

#[cfg(test)]
impl ConsoleInterfacePort<AssistantWebSocketCommandInput, AssistantWebSocketCommandOutput>
    for UnavailableAssistantWebSocketCommandPort
{
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: AssistantWebSocketCommandInput,
    ) -> ConsoleInterfaceFuture<'a, AssistantWebSocketCommandOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("assistant websocket command fixture unavailable").into(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use interface_runtime::BindingId;

    use super::*;

    #[test]
    fn f09b2_registry_freezes_assistant_websocket_command_binding() {
        let registry =
            compile_registry_with_port(Arc::new(UnavailableAssistantWebSocketCommandPort)).unwrap();
        for declaration in DECLARATIONS {
            assert!(registry
                .binding(&BindingId::new(declaration.binding_id).unwrap())
                .is_some());
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }
}
