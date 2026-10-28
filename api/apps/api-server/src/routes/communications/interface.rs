use super::{FileQuery, PageQuery, provisioning, scope};
use crate::{
    app_state::{ApiNotificationService, ApiState},
    error_response::ApiError,
    routes::{
        console_interface::{
            self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
            ConsoleInterfaceTargetError,
        },
        mcp_management::interface_catalog::McpInterfaceCatalogDependencies,
    },
};
use base64::{Engine, engine::general_purpose::STANDARD};
use futures_util::FutureExt;
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct Dependencies {
    pub(crate) store: storage_durable_postgres::MainDurableStore,
    pub(crate) service: Arc<ApiNotificationService>,
    pub(crate) catalog: McpInterfaceCatalogDependencies,
    pub(crate) file_service: Arc<
        control_plane::file_management::FileAccessService<
            storage_durable_postgres::MainDurableStore,
        >,
    >,
}
impl Dependencies {
    pub(crate) fn new(state: &ApiState, catalog: McpInterfaceCatalogDependencies) -> Self {
        Self {
            store: state.store.clone(),
            service: state.notification_service.clone(),
            catalog,
            file_service: Arc::new(control_plane::file_management::FileAccessService::new(
                state.store.clone(),
                state.file_storage_registry.clone(),
                state.runtime_engine.clone(),
                state.infrastructure.ephemeral_kv_store(),
            )),
        }
    }
}
pub(crate) enum Input {
    ManagementConversations(domain::CommunicationManagementQuery),
    ManagementHistory(Uuid, PageQuery),
    ManagementMessages(domain::CommunicationManagementQuery),
    ManagementMessage(Uuid),
    Options(Option<Uuid>, Option<Uuid>),
    Binding(Uuid),
    SaveBinding(Uuid, domain::CommunicationBindingInput),
    Conversations(Uuid, PageQuery),
    Messages(Uuid, Uuid, PageQuery),
    Files(Uuid, Uuid, PageQuery),
    File(Uuid, Uuid, FileQuery),
    Retry(Uuid, Uuid),
}
impl Input {
    fn authorize_scope(&self) -> Result<(), ApiError> {
        match self {
            Self::ManagementConversations(_)
            | Self::ManagementHistory(..)
            | Self::ManagementMessages(_)
            | Self::ManagementMessage(_) => scope::require_management_scope(),
            _ => Ok(()),
        }
    }
}
pub(crate) struct Output(pub Value);
impl InterfaceContract for Input {
    const CONTRACT_ID: &'static str = "console-communications-input";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<Value> {
        Some(crate::extension_bus::managed_projection::object_schema([(
            "operation",
            crate::extension_bus::managed_projection::text_schema(),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<Value> {
        Some(
            json!({"operation":match self{Self::ManagementConversations(_)=>"management_conversations",Self::ManagementHistory(..)=>"management_history",Self::ManagementMessages(_)=>"management_messages",Self::ManagementMessage(_)=>"management_message",Self::Options(..)=>"options",Self::Binding(_)=>"binding",Self::SaveBinding(..)=>"save_binding",Self::Conversations(..)=>"conversations",Self::Messages(..)=>"messages",Self::Files(..)=>"files",Self::File(..)=>"file",Self::Retry(..)=>"retry"}}),
        )
    }
}
impl InterfaceContract for Output {
    const CONTRACT_ID: &'static str = "console-communications-output";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<Value> {
        Some(crate::extension_bus::managed_projection::json_summary_schema())
    }
    fn project_for_managed_hook(&self) -> Option<Value> {
        Some(crate::extension_bus::managed_projection::json_summary(
            &self.0,
        ))
    }
}
impl Dependencies {
    fn execute_inner<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: Input,
    ) -> ConsoleInterfaceFuture<'a, Output> {
        let actor = principal.actor();
        if let Err(error) = input.authorize_scope() {
            return Box::pin(async move { Err(ConsoleInterfaceTargetError(error)) });
        }
        // Construct the selected operation synchronously so read-only MCP calls do not
        // retain the preparation frames of binding/provisioning and file operations.
        match input {
            Input::ManagementHistory(id,q) => Box::pin(async move {
                Ok(Output(serde_json::to_value(self.service.management_history(actor,id,q.cursor,q.limit.unwrap_or(5)).await?)?))
            }.map(|result: Result<Output, ApiError>| result.map_err(ConsoleInterfaceTargetError))),
            Input::ManagementConversations(q) => Box::pin(async move {
                Ok(Output(serde_json::to_value(self.service.management_conversations(actor, &q).await?)?))
            }.map(|result: Result<Output, ApiError>| result.map_err(ConsoleInterfaceTargetError))),
            Input::ManagementMessages(q) => Box::pin(async move {
                Ok(Output(serde_json::to_value(self.service.management_messages(actor, &q).await?)?))
            }.map(|result: Result<Output, ApiError>| result.map_err(ConsoleInterfaceTargetError))),
            Input::ManagementMessage(id) => Box::pin(async move {
                Ok(Output(serde_json::to_value(self.service.management_message(actor, id).await?)?))
            }.map(|result: Result<Output, ApiError>| result.map_err(ConsoleInterfaceTargetError))),
            Input::Options(application_id, instance_id) => Box::pin(async move {
                let result = {
                let (published_agent_flows, enabled_mcp_instances) =
                    crate::routes::assistant::available_targets(&self.store, actor).await?;
                let (run_capabilities, model_unavailable_reason) = crate::communication_runtime::command_capabilities::load(
                    &self.store,
                    actor,
                    application_id,
                )
                .await?;
                let binding = match instance_id {
                    Some(id) => self.service.get_binding(actor, id).await?,
                    None => None,
                };
                let settings = binding.as_ref().map(|b| b.config.command_settings.as_slice()).unwrap_or_default();
                let mut commands = self.service.command_catalog(actor, settings).await?;
                control_plane::notifications::apply_communication_model_availability(&mut commands, model_unavailable_reason);
                json!({"published_agent_flows":published_agent_flows,"enabled_mcp_instances":enabled_mcp_instances,"run_capabilities":run_capabilities,"commands":commands})
            };
                Ok(Output(result))
            }.map(|result: Result<Output, ApiError>| result.map_err(ConsoleInterfaceTargetError))),
            Input::Binding(id) => Box::pin(async move {
                let result = {
                let instance = self.service.get_instance(actor, id).await?;
                let descriptor = self
                    .service
                    .types(actor)
                    .await?
                    .into_iter()
                    .find(|t| {
                        t.descriptor.type_code == instance.record.type_code
                            && t.installation_id == instance.record.installation_id
                    })
                    .and_then(|t| t.descriptor.receive);
                let binding = self.service.get_binding(actor, id).await?;
                let plugin_mcp_instance_id =
                    descriptor.as_ref().map(|_| provisioning::instance_id(id));
                let settings = binding.as_ref().map(|b| b.config.command_settings.as_slice()).unwrap_or_default();
                let mut commands = self.service.command_catalog(actor, settings).await?;
                let (_, reason) = crate::communication_runtime::command_capabilities::load(
                    &self.store, actor, binding.as_ref().map(|b| b.config.application_id),
                ).await?;
                control_plane::notifications::apply_communication_model_availability(&mut commands, reason);
                json!({"binding":binding,"plugin_mcp_instance_id":plugin_mcp_instance_id,"default_system_prompt":descriptor.map(|d|d.default_system_prompt),"commands":commands})
            };
                Ok(Output(result))
            }.map(|result: Result<Output, ApiError>| result.map_err(ConsoleInterfaceTargetError))),
            Input::SaveBinding(id, mut input) => Box::pin(async move {
                let result = {
                let instance = self.service.get_instance(actor, id).await?;
                let descriptor = self
                    .service
                    .types(actor)
                    .await?
                    .into_iter()
                    .find(|entry| {
                        entry.descriptor.type_code == instance.record.type_code
                            && entry.installation_id == instance.record.installation_id
                    })
                    .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                        "communication_type",
                    ))?;
                if descriptor
                    .descriptor
                    .input_schema
                    .pointer("/properties/text/type")
                    .and_then(Value::as_str)
                    != Some("string")
                {
                    return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                        "communication_text_input_schema",
                    )
                    .into());
                }

                let mcp = provisioning::ensure_channel_mcp(self, actor, id)
                    .await?
                    .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                        "communication_receive_unsupported",
                    ))?;
                if self.service.get_binding(actor, id).await?.is_none()
                    && !input.mcp_instance_ids.contains(&mcp)
                {
                    input.mcp_instance_ids.insert(0, mcp.clone());
                }
                crate::routes::assistant::validate_preference(
                    &self.store,
                    actor,
                    &crate::routes::assistant::AssistantPreferenceBody {
                        application_id: Some(input.application_id),
                        mcp_instance_ids: input.mcp_instance_ids.clone(),
                        model: input.model.clone(),
                        reasoning_effort: input.reasoning_effort.clone(),
                        enabled_client_tools: Vec::new(),
                    },
                )
                .await?;
                // Validate the selected table against the same workspace visibility used by file management.
                if let Some(table_id) = input.file_table_id {
                    use control_plane::ports::FileManagementRepository;
                    if !self
                        .store
                        .list_visible_file_tables(actor.current_workspace_id)
                        .await?
                        .iter()
                        .any(|t| t.id == table_id && t.status == "active")
                    {
                        return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                            "file_table_id",
                        )
                        .into());
                    }
                }
                serde_json::to_value(self.service.save_binding(actor, id, input, mcp).await?)?
            };
                Ok(Output(result))
            }.map(|result: Result<Output, ApiError>| result.map_err(ConsoleInterfaceTargetError))),
            Input::Conversations(id, q) => Box::pin(async move {
                let result = match scope::for_channel(actor, id)? {
                Some(s) => serde_json::to_value(
                    self.service
                        .scoped_conversations(&s, q.cursor, q.limit.unwrap_or(5))
                        .await?,
                )?,
                None => serde_json::to_value(
                    self.service
                        .conversations(actor, id, q.cursor, q.limit.unwrap_or(5))
                        .await?,
                )?,
            };
                Ok(Output(result))
            }.map(|result: Result<Output, ApiError>| result.map_err(ConsoleInterfaceTargetError))),
            Input::Messages(id, conversation, q) => Box::pin(async move {
                let result = match scope::for_channel(actor, id)? {
                Some(s) => serde_json::to_value(
                    self.service
                        .scoped_message_history(&s, conversation, q.cursor, q.limit.unwrap_or(5))
                        .await?,
                )?,
                None => serde_json::to_value(
                    self.service
                        .message_history(actor, id, conversation, q.cursor, q.limit.unwrap_or(5))
                        .await?,
                )?,
            };
                Ok(Output(result))
            }.map(|result: Result<Output, ApiError>| result.map_err(ConsoleInterfaceTargetError))),
            Input::Files(id, conversation, q) => Box::pin(async move {
                let result = match scope::for_channel(actor, id)? {
                Some(s) => serde_json::to_value(
                    self.service
                        .scoped_files(&s, conversation, q.cursor, q.limit.unwrap_or(5))
                        .await?,
                )?,
                None => serde_json::to_value(
                    self.service
                        .files(actor, id, conversation, q.cursor, q.limit.unwrap_or(5))
                        .await?,
                )?,
            };
                Ok(Output(result))
            }.map(|result: Result<Output, ApiError>| result.map_err(ConsoleInterfaceTargetError))),
            Input::File(id, message_id, q) => Box::pin(async move {
                let result = {
                let message = match scope::for_channel(actor, id)? {
                    Some(s) => self.service.scoped_file(&s, message_id).await?,
                    None => self.service.file(actor, id, message_id).await?,
                };
                self.read_file(actor, message, q).await?
            };
                Ok(Output(result))
            }.map(|result: Result<Output, ApiError>| result.map_err(ConsoleInterfaceTargetError))),
            Input::Retry(id, message) => Box::pin(async move {
                let result = {
                // Retry is a management action; read-only history MCP declarations never expose it.
                serde_json::to_value(self.service.retry_message_record(actor,id,message).await?)?
            };
                Ok(Output(result))
            }.map(|result: Result<Output, ApiError>| result.map_err(ConsoleInterfaceTargetError))),
        }
    }
    async fn read_file(
        &self,
        actor: &domain::ActorContext,
        message: domain::CommunicationMessage,
        q: FileQuery,
    ) -> Result<Value, ApiError> {
        let (Some(table), Some(record)) = message
            .file
            .as_ref()
            .map(|file| (file.file_table_id, file.file_record_id))
            .unwrap_or((None, None))
        else {
            return Ok(json!({"message":message,"url":null,"preview_url":null}));
        };
        let content = self.file_service.read(actor.clone(), table, record).await?;
        let offset = q.offset.unwrap_or(0);
        if offset > content.content_length {
            return Err(control_plane::errors::ControlPlaneError::InvalidInput("offset").into());
        }
        let length = u64::from(q.max_bytes.unwrap_or(48 * 1024).clamp(1, 48 * 1024))
            .min(content.content_length - offset);
        let mut reader = content
            .reader
            .into_inner()
            .map_err(|_| control_plane::errors::ControlPlaneError::Conflict("file_reader"))?;
        tokio::io::copy(&mut (&mut reader).take(offset), &mut tokio::io::sink()).await?;
        let mut bytes = Vec::with_capacity(length as usize);
        reader.take(length).read_to_end(&mut bytes).await?;
        let next_offset = offset + bytes.len() as u64;
        let text = std::str::from_utf8(&bytes).ok().map(str::to_owned);
        Ok(
            json!({"message":message,"file_name":content.filename,"content_type":content.content_type,"content_length":content.content_length,"offset":offset,"next_offset":next_offset,"eof":next_offset>=content.content_length,"content":text,"data_base64":STANDARD.encode(&bytes),"url":format!("/api/console/files/{table}/records/{record}/content"),"preview_url":null}),
        )
    }
}
impl ConsoleInterfacePort<Input, Output> for Dependencies {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: Input,
    ) -> ConsoleInterfaceFuture<'a, Output> {
        self.execute_inner(principal, input)
    }
}
pub(crate) fn compile_registry(
    dependencies: Dependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-communications",
        "graph:console-communications-v1",
        DECLARATIONS,
        Arc::new(dependencies),
    )
}
pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "communications.management.conversations.messages.list",
        binding_id: "http.console.communications.management.conversations.messages.list.v1",
        method: "GET",
        path: "/api/console/communications/conversations/:conversation_id/messages",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "communications.management.conversations.list",
        binding_id: "http.console.communications.management.conversations.list.v1",
        method: "GET",
        path: "/api/console/communications/conversations",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "communications.management.messages.list",
        binding_id: "http.console.communications.management.messages.list.v1",
        method: "GET",
        path: "/api/console/communications/messages",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "communications.management.messages.get",
        binding_id: "http.console.communications.management.messages.get.v1",
        method: "GET",
        path: "/api/console/communications/messages/:message_id",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "communications.options.get",
        binding_id: "http.console.communications.options.get.v1",
        method: "GET",
        path: "/api/console/communications/options",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "communications.binding.get",
        binding_id: "http.console.communications.binding.get.v1",
        method: "GET",
        path: "/api/console/communications/instances/:instance_id/binding",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "communications.binding.update",
        binding_id: "http.console.communications.binding.update.v1",
        method: "PUT",
        path: "/api/console/communications/instances/:instance_id/binding",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "communications.conversations.list",
        binding_id: "http.console.communications.conversations.list.v1",
        method: "GET",
        path: "/api/console/communications/instances/:instance_id/conversations",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "communications.messages.list",
        binding_id: "http.console.communications.messages.list.v1",
        method: "GET",
        path: "/api/console/communications/instances/:instance_id/conversations/:conversation_id/messages",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "communications.files.list",
        binding_id: "http.console.communications.files.list.v1",
        method: "GET",
        path: "/api/console/communications/instances/:instance_id/conversations/:conversation_id/files",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "communications.files.get",
        binding_id: "http.console.communications.files.get.v1",
        method: "GET",
        path: "/api/console/communications/instances/:instance_id/files/:message_id",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "communications.messages.retry",
        binding_id: "http.console.communications.messages.retry.v1",
        method: "POST",
        path: "/api/console/communications/instances/:instance_id/messages/:message_id/retry",
        mutating: true,
    },
];

#[cfg(test)]
#[path = "_tests/management_scope.rs"]
mod management_scope_tests;
