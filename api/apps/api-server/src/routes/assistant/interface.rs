use std::{convert::Infallible, sync::Arc};

use axum::response::sse::Event;

use control_plane::{
    application_public_api::run_service::{
        ApplicationPublishedFlowRunRepository, AssistantConversationSummary,
        CreateAssistantConversationInput, ListAssistantConversationsInput,
    },
    ports::OrchestrationRuntimeRepository,
    profile::{ProfileService, UpdateMeMetaCommand},
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde_json::json;
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::{
    assistant_preference_for_actor, assistant_run_capabilities, available_targets,
    execute_assistant_run, launch_assistant_execution, prepare_assistant_execution,
    read_preference, validate_preference, AssistantConversationMessageResponse,
    AssistantConversationPageResponse, AssistantConversationResponse, AssistantPageReferenceBody,
    AssistantPreferenceBody, AssistantRunActivityPageResponse, AssistantRunActivityQuery,
    AssistantRunDependencies, AssistantRunResponse, AssistantSettingsResponse,
    CreateAssistantConversationBody, ListAssistantConversationsQuery, StartAssistantRunBody,
    ASSISTANT_META_KEY,
};
use super::{
    conversation_events::{
        AssistantConversationEventHub, AssistantConversationEventKind,
        AssistantConversationEventScope, AssistantConversationSummaryResponse,
    },
    run_activity::{format_assistant_activity_time, project_assistant_run_activity},
};
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError, ConsoleServerStreamFuture, ConsoleServerStreamPort,
    },
};

pub(crate) enum AssistantSettingsInput {
    Get,
    Update(AssistantPreferenceBody),
}

impl InterfaceContract for AssistantSettingsInput {
    const CONTRACT_ID: &'static str = "console-assistant-settings-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum AssistantSettingsOutput {
    Settings(AssistantSettingsResponse),
}

impl InterfaceContract for AssistantSettingsOutput {
    const CONTRACT_ID: &'static str = "console-assistant-settings-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum AssistantConversationsInput {
    GetRunActivity {
        flow_run_id: Uuid,
        query: AssistantRunActivityQuery,
    },
    CreateConversation(CreateAssistantConversationBody),
    ListConversations(ListAssistantConversationsQuery),
    GetConversationMessages {
        conversation_id: Uuid,
        query: super::AssistantConversationMessagesQuery,
    },
    GetLegacySnapshotMessages {
        flow_run_id: Uuid,
        query: super::AssistantConversationMessagesQuery,
    },
}

impl InterfaceContract for AssistantConversationsInput {
    const CONTRACT_ID: &'static str = "console-assistant-conversations-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum AssistantConversationsOutput {
    RunActivity(AssistantRunActivityPageResponse),
    Conversation(AssistantConversationResponse),
    ConversationPage(AssistantConversationPageResponse),
    ConversationMessages(Vec<AssistantConversationMessageResponse>),
}

impl InterfaceContract for AssistantConversationsOutput {
    const CONTRACT_ID: &'static str = "console-assistant-conversations-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct AssistantRunInput {
    pub(crate) body: StartAssistantRunBody,
    pub(crate) headers: axum::http::HeaderMap,
}

impl InterfaceContract for AssistantRunInput {
    const CONTRACT_ID: &'static str = "console-assistant-run-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct AssistantRunOutput(pub(crate) AssistantRunResponse);

impl InterfaceContract for AssistantRunOutput {
    const CONTRACT_ID: &'static str = "console-assistant-run-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct AssistantRunStreamEvent(pub(crate) Result<Event, Infallible>);

impl InterfaceContract for AssistantRunStreamEvent {
    const CONTRACT_ID: &'static str = "console-assistant-run-stream-event";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct AssistantRunStreamOutput(pub(crate) Uuid);

impl InterfaceContract for AssistantRunStreamOutput {
    const CONTRACT_ID: &'static str = "console-assistant-run-stream-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct AssistantSettingsAdapter {
    store: MainDurableStore,
}

struct AssistantConversationsAdapter {
    store: MainDurableStore,
    conversation_events: Arc<AssistantConversationEventHub>,
}

struct AssistantRunAdapter {
    dependencies: AssistantRunDependencies,
}

pub(crate) fn settings_port(
    store: MainDurableStore,
) -> Arc<dyn ConsoleInterfacePort<AssistantSettingsInput, AssistantSettingsOutput>> {
    Arc::new(AssistantSettingsAdapter { store })
}

pub(crate) fn conversations_port(
    store: MainDurableStore,
    conversation_events: Arc<AssistantConversationEventHub>,
) -> Arc<dyn ConsoleInterfacePort<AssistantConversationsInput, AssistantConversationsOutput>> {
    Arc::new(AssistantConversationsAdapter {
        store,
        conversation_events,
    })
}

pub(crate) fn runs_port(dependencies: AssistantRunDependencies) -> Arc<AssistantRunAdapter> {
    Arc::new(AssistantRunAdapter { dependencies })
}

impl AssistantSettingsAdapter {
    async fn settings_response(
        &self,
        actor: &domain::ActorContext,
        preference: AssistantPreferenceBody,
    ) -> Result<AssistantSettingsResponse, ApiError> {
        let (published_agent_flows, enabled_mcp_instances) =
            available_targets(&self.store, actor).await?;
        let run_capabilities =
            assistant_run_capabilities(&self.store, actor, preference.application_id).await?;
        Ok(AssistantSettingsResponse {
            preference,
            published_agent_flows,
            enabled_mcp_instances,
            page_reference_max_bytes:
                control_plane::application_public_api::run_service::ASSISTANT_PAGE_REFERENCE_MAX_BYTES,
            page_reference_max_count:
                control_plane::application_public_api::run_service::ASSISTANT_PAGE_REFERENCE_MAX_COUNT,
            page_reference_max_total_bytes:
                control_plane::application_public_api::run_service::ASSISTANT_PAGE_REFERENCE_MAX_TOTAL_BYTES,
            run_capabilities,
        })
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: AssistantSettingsInput,
    ) -> Result<AssistantSettingsOutput, ApiError> {
        let actor = principal.actor();
        let user = self
            .store
            .find_user_by_id(actor.user_id)
            .await?
            .ok_or(control_plane::errors::ControlPlaneError::NotFound("user"))?;
        let current_preference = read_preference(&user.meta, actor.current_workspace_id);
        let preference = match input {
            AssistantSettingsInput::Get => current_preference,
            AssistantSettingsInput::Update(preference) => {
                let preference = if current_preference.application_id != preference.application_id {
                    AssistantPreferenceBody {
                        model: None,
                        reasoning_effort: None,
                        ..preference
                    }
                } else {
                    preference
                };
                validate_preference(&self.store, actor, &preference).await?;
                let workspace_id = actor.current_workspace_id;
                let meta_patch = json!({
                    ASSISTANT_META_KEY: { "workspaces": { workspace_id.to_string(): preference } }
                });
                ProfileService::new(self.store.clone())
                    .update_me_meta(UpdateMeMetaCommand {
                        actor_user_id: actor.user_id,
                        tenant_id: actor.tenant_id,
                        workspace_id,
                        meta_patch,
                    })
                    .await?;
                preference
            }
        };
        Ok(AssistantSettingsOutput::Settings(
            self.settings_response(actor, preference).await?,
        ))
    }
}

impl AssistantConversationsAdapter {
    async fn get_run_activity(
        &self,
        actor: &domain::ActorContext,
        flow_run_id: Uuid,
        query: AssistantRunActivityQuery,
    ) -> Result<AssistantRunActivityPageResponse, ApiError> {
        const DEFAULT_PAGE_SIZE: usize = 200;
        const MAX_PAGE_SIZE: usize = 500;

        assistant_preference_for_actor(&self.store, actor, query.application_id).await?;
        if !self
            .store
            .is_assistant_run_visible(
                actor.current_workspace_id,
                query.application_id,
                actor.user_id,
                flow_run_id,
            )
            .await?
        {
            return Err(control_plane::errors::ControlPlaneError::NotFound("flow_run").into());
        }
        let flow_run = OrchestrationRuntimeRepository::get_flow_run(
            &self.store,
            query.application_id,
            flow_run_id,
        )
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "flow_run",
        ))?;

        let page_size = query
            .page_size
            .unwrap_or(DEFAULT_PAGE_SIZE)
            .clamp(1, MAX_PAGE_SIZE);
        let mut events = OrchestrationRuntimeRepository::list_runtime_event_backfill_page(
            &self.store,
            flow_run_id,
            // An omitted cursor must include sequence zero when a runtime stream
            // implementation persisted one, rather than silently skipping it.
            query.after_sequence.unwrap_or(-1),
            page_size + 1,
        )
        .await?;
        let has_more = events.len() > page_size;
        events.truncate(page_size);
        let trace_events = events
            .into_iter()
            .map(crate::routes::debug_run_stream::to_runtime_event_record_response)
            .collect::<Vec<_>>();
        let next_sequence = has_more
            .then(|| trace_events.last().map(|item| item.sequence))
            .flatten();
        let items = trace_events
            .iter()
            .filter_map(project_assistant_run_activity)
            .collect();
        let finished_at = flow_run.finished_at;
        let duration_ms = finished_at.map(|value| {
            let milliseconds = (value - flow_run.started_at).whole_milliseconds();
            i64::try_from(milliseconds).unwrap_or(i64::MAX).max(0)
        });

        Ok(AssistantRunActivityPageResponse {
            status: flow_run.status.as_str().to_string(),
            started_at: format_assistant_activity_time(flow_run.started_at),
            finished_at: finished_at.map(format_assistant_activity_time),
            duration_ms,
            items,
            trace_events,
            has_more,
            next_sequence,
        })
    }

    async fn create_conversation(
        &self,
        actor: &domain::ActorContext,
        body: CreateAssistantConversationBody,
    ) -> Result<AssistantConversationResponse, ApiError> {
        assistant_preference_for_actor(&self.store, actor, body.application_id).await?;
        if let Some(legacy_flow_run_id) = body.seed_legacy_flow_run_id {
            let seed_messages = self
                .store
                .list_assistant_legacy_snapshot_messages(
                    actor.current_workspace_id,
                    body.application_id,
                    actor.user_id,
                    legacy_flow_run_id,
                )
                .await?;
            if seed_messages.is_empty() {
                return Err(control_plane::errors::ControlPlaneError::PermissionDenied(
                    "assistant_legacy_snapshot",
                )
                .into());
            }
        }
        let conversation = self
            .store
            .create_assistant_conversation(&CreateAssistantConversationInput {
                conversation_id: Uuid::now_v7(),
                workspace_id: actor.current_workspace_id,
                application_id: body.application_id,
                actor_user_id: actor.user_id,
                seed_legacy_flow_run_id: body.seed_legacy_flow_run_id,
            })
            .await?;
        self.conversation_events.publish(
            AssistantConversationEventScope {
                workspace_id: actor.current_workspace_id,
                application_id: body.application_id,
                actor_user_id: actor.user_id,
            },
            AssistantConversationEventKind::Created,
            AssistantConversationSummaryResponse::from(AssistantConversationSummary {
                conversation_id: Some(conversation.conversation_id),
                legacy_flow_run_id: None,
                latest_flow_run_id: None,
                latest_flow_run_status: None,
                title: None,
                created_at: conversation.created_at,
                updated_at: conversation.updated_at,
            }),
        );
        Ok(assistant_conversation_response(conversation))
    }

    async fn list_conversations(
        &self,
        actor: &domain::ActorContext,
        query: ListAssistantConversationsQuery,
    ) -> Result<AssistantConversationPageResponse, ApiError> {
        assistant_preference_for_actor(&self.store, actor, query.application_id).await?;
        let page = self
            .store
            .list_assistant_conversations(&ListAssistantConversationsInput {
                workspace_id: actor.current_workspace_id,
                application_id: query.application_id,
                actor_user_id: actor.user_id,
                page: query.page.unwrap_or(1),
                page_size: query.page_size.unwrap_or(20),
            })
            .await?;
        Ok(AssistantConversationPageResponse {
            items: page
                .items
                .into_iter()
                .map(AssistantConversationSummaryResponse::from)
                .collect(),
            total: page.total,
            page: page.page,
            page_size: page.page_size,
        })
    }

    async fn conversation_messages(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        conversation_id: Uuid,
    ) -> Result<Vec<AssistantConversationMessageResponse>, ApiError> {
        assistant_preference_for_actor(&self.store, actor, application_id).await?;
        let messages = self
            .store
            .list_assistant_conversation_messages(
                actor.current_workspace_id,
                application_id,
                actor.user_id,
                conversation_id,
            )
            .await?;
        Ok(messages
            .into_iter()
            .map(assistant_conversation_message_response)
            .collect())
    }

    async fn legacy_snapshot_messages(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Vec<AssistantConversationMessageResponse>, ApiError> {
        assistant_preference_for_actor(&self.store, actor, application_id).await?;
        let messages = self
            .store
            .list_assistant_legacy_snapshot_messages(
                actor.current_workspace_id,
                application_id,
                actor.user_id,
                flow_run_id,
            )
            .await?;
        Ok(messages
            .into_iter()
            .map(assistant_conversation_message_response)
            .collect())
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: AssistantConversationsInput,
    ) -> Result<AssistantConversationsOutput, ApiError> {
        let actor = principal.actor();
        match input {
            AssistantConversationsInput::GetRunActivity { flow_run_id, query } => {
                Ok(AssistantConversationsOutput::RunActivity(
                    self.get_run_activity(actor, flow_run_id, query).await?,
                ))
            }
            AssistantConversationsInput::CreateConversation(body) => {
                Ok(AssistantConversationsOutput::Conversation(
                    self.create_conversation(actor, body).await?,
                ))
            }
            AssistantConversationsInput::ListConversations(query) => {
                Ok(AssistantConversationsOutput::ConversationPage(
                    self.list_conversations(actor, query).await?,
                ))
            }
            AssistantConversationsInput::GetConversationMessages {
                conversation_id,
                query,
            } => Ok(AssistantConversationsOutput::ConversationMessages(
                self.conversation_messages(actor, query.application_id, conversation_id)
                    .await?,
            )),
            AssistantConversationsInput::GetLegacySnapshotMessages { flow_run_id, query } => {
                Ok(AssistantConversationsOutput::ConversationMessages(
                    self.legacy_snapshot_messages(actor, query.application_id, flow_run_id)
                        .await?,
                ))
            }
        }
    }
}

fn assistant_conversation_response(
    conversation: control_plane::application_public_api::run_service::AssistantConversationRecord,
) -> AssistantConversationResponse {
    AssistantConversationResponse {
        conversation_id: conversation.conversation_id,
        application_id: conversation.application_id,
        created_at: rfc3339(conversation.created_at),
        updated_at: rfc3339(conversation.updated_at),
    }
}

fn assistant_conversation_message_response(
    message: control_plane::application_public_api::run_service::AssistantConversationMessage,
) -> AssistantConversationMessageResponse {
    AssistantConversationMessageResponse {
        id: message.id,
        flow_run_id: message.flow_run_id,
        role: message.role,
        content: message.content,
        status: message.status,
        page_references: message
            .page_references
            .into_iter()
            .map(AssistantPageReferenceBody::from_reference)
            .collect(),
        created_at: rfc3339(message.created_at),
    }
}

fn rfc3339(value: time::OffsetDateTime) -> String {
    value
        .format(&time::format_description::well_known::Rfc3339)
        .expect("Rfc3339 is a valid fixed formatter")
}

impl ConsoleInterfacePort<AssistantSettingsInput, AssistantSettingsOutput>
    for AssistantSettingsAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: AssistantSettingsInput,
    ) -> ConsoleInterfaceFuture<'a, AssistantSettingsOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

impl ConsoleInterfacePort<AssistantConversationsInput, AssistantConversationsOutput>
    for AssistantConversationsAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: AssistantConversationsInput,
    ) -> ConsoleInterfaceFuture<'a, AssistantConversationsOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

impl ConsoleInterfacePort<AssistantRunInput, AssistantRunOutput> for AssistantRunAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: AssistantRunInput,
    ) -> ConsoleInterfaceFuture<'a, AssistantRunOutput> {
        Box::pin(async move {
            execute_assistant_run(&self.dependencies, principal, input.body, input.headers)
                .await
                .map(AssistantRunOutput)
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

impl ConsoleServerStreamPort<AssistantRunInput, AssistantRunStreamEvent, AssistantRunStreamOutput>
    for AssistantRunAdapter
{
    fn execute_stream(
        &self,
        principal: &UserPrincipal,
        input: AssistantRunInput,
    ) -> ConsoleServerStreamFuture<AssistantRunStreamEvent, AssistantRunStreamOutput> {
        let dependencies = self.dependencies.clone();
        let actor = principal.actor().clone();
        Box::pin(async move {
            let execution = prepare_assistant_execution(
                &dependencies.store,
                &input.headers,
                &actor,
                input.body,
            )
            .await
            .map_err(ConsoleInterfaceTargetError)
            .map_err(|error| {
                interface_runtime::InterfaceTargetFailure::new("console_interface", error)
            })?;
            let run_id = launch_assistant_execution(dependencies.clone(), execution, None)
                .await
                .map_err(ConsoleInterfaceTargetError)
                .map_err(|error| {
                    interface_runtime::InterfaceTargetFailure::new("console_interface", error)
                })?;
            let (publisher, stream) = interface_runtime::interface_stream_channel(32);
            let (sender, mut receiver) = tokio::sync::mpsc::channel(32);
            tokio::spawn(crate::routes::debug_run_stream::send_runtime_event_stream(
                dependencies.runtime_event_stream.clone(),
                Arc::new(dependencies.store.clone()),
                run_id,
                None,
                sender,
            ));
            tokio::spawn(async move {
                while let Some(event) = receiver.recv().await {
                    if publisher
                        .emit(AssistantRunStreamEvent(event))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
                let _ = publisher
                    .finish(interface_runtime::InterfaceStreamTerminal::Completed(
                        AssistantRunStreamOutput(run_id),
                    ))
                    .await;
            });
            Ok(stream)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "assistant.settings.get",
        binding_id: "http.console.assistant.settings.get.v1",
        method: "GET",
        path: "/api/console/assistant/settings",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "assistant.settings.update",
        binding_id: "http.console.assistant.settings.update.v1",
        method: "PATCH",
        path: "/api/console/assistant/settings",
        mutating: true,
    },
];

pub(crate) const CONVERSATION_DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "assistant.runs.activity.get",
        binding_id: "http.console.assistant.runs.activity.get.v1",
        method: "GET",
        path: "/api/console/assistant/runs/:flow_run_id/activity",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "assistant.conversations.create",
        binding_id: "http.console.assistant.conversations.create.v1",
        method: "POST",
        path: "/api/console/assistant/conversations",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "assistant.conversations.list",
        binding_id: "http.console.assistant.conversations.list.v1",
        method: "GET",
        path: "/api/console/assistant/conversations",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "assistant.conversations.messages.get",
        binding_id: "http.console.assistant.conversations.messages.get.v1",
        method: "GET",
        path: "/api/console/assistant/conversations/:conversation_id/messages",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "assistant.legacy-runs.messages.get",
        binding_id: "http.console.assistant.legacy-runs.messages.get.v1",
        method: "GET",
        path: "/api/console/assistant/legacy-runs/:flow_run_id/messages",
        mutating: false,
    },
];

pub(crate) const RUN_DECLARATIONS: &[ConsoleInterfaceDeclaration] =
    &[ConsoleInterfaceDeclaration {
        interface_id: "assistant.runs.create",
        binding_id: "http.console.assistant.runs.create.v1",
        method: "POST",
        path: "/api/console/assistant/runs",
        mutating: true,
    }];

pub(crate) const RUN_STREAM_DECLARATIONS: &[ConsoleInterfaceDeclaration] =
    &[ConsoleInterfaceDeclaration {
        interface_id: "assistant.runs.stream.create",
        binding_id: "http.console.assistant.runs.stream.v1",
        method: "POST",
        path: "/api/console/assistant/runs/stream",
        mutating: true,
    }];

pub(crate) fn compile_registry(
    store: MainDurableStore,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    compile_registry_with_port(settings_port(store))
}

fn compile_registry_with_port(
    port: Arc<dyn ConsoleInterfacePort<AssistantSettingsInput, AssistantSettingsOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-assistant-settings",
        "graph:console-assistant-settings-v1",
        DECLARATIONS,
        port,
    )
}

pub(crate) fn compile_conversations_registry(
    store: MainDurableStore,
    conversation_events: Arc<AssistantConversationEventHub>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    compile_conversations_registry_with_port(conversations_port(store, conversation_events))
}

fn compile_conversations_registry_with_port(
    port: Arc<dyn ConsoleInterfacePort<AssistantConversationsInput, AssistantConversationsOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-assistant-conversations",
        "graph:console-assistant-conversations-v1",
        CONVERSATION_DECLARATIONS,
        port,
    )
}

/// Compiles the unary embedded Assistant run binding for contribution assembly.
/// Shared boot wiring deliberately lives outside this Assistant-local packet.
pub(crate) fn compile_runs_registry(
    dependencies: AssistantRunDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-assistant-runs",
        "graph:console-assistant-runs-v1",
        RUN_DECLARATIONS,
        runs_port(dependencies),
    )
}

/// Compiles the typed server-stream embedded Assistant run binding for contribution assembly.
/// Shared boot wiring deliberately lives outside this Assistant-local packet.
pub(crate) fn compile_run_stream_registry(
    dependencies: AssistantRunDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_server_stream_registry(
        "api-server.console-assistant-run-stream",
        "graph:console-assistant-run-stream-v1",
        RUN_STREAM_DECLARATIONS,
        runs_port(dependencies),
    )
}

#[cfg(test)]
struct UnavailableAssistantSettingsPort;

#[cfg(test)]
struct UnavailableAssistantConversationsPort;

#[cfg(test)]
impl ConsoleInterfacePort<AssistantSettingsInput, AssistantSettingsOutput>
    for UnavailableAssistantSettingsPort
{
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: AssistantSettingsInput,
    ) -> ConsoleInterfaceFuture<'a, AssistantSettingsOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("assistant settings fixture unavailable").into(),
            ))
        })
    }
}

#[cfg(test)]
impl ConsoleInterfacePort<AssistantConversationsInput, AssistantConversationsOutput>
    for UnavailableAssistantConversationsPort
{
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: AssistantConversationsInput,
    ) -> ConsoleInterfaceFuture<'a, AssistantConversationsOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("assistant conversations fixture unavailable").into(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use interface_runtime::BindingId;

    use super::*;

    #[test]
    fn f09b1a_registry_freezes_assistant_settings_bindings() {
        let registry =
            compile_registry_with_port(Arc::new(UnavailableAssistantSettingsPort)).unwrap();
        for declaration in DECLARATIONS {
            assert!(registry
                .binding(&BindingId::new(declaration.binding_id).unwrap())
                .is_some());
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }

    #[test]
    fn f09b1b_registry_freezes_assistant_conversation_bindings() {
        let registry = compile_conversations_registry_with_port(Arc::new(
            UnavailableAssistantConversationsPort,
        ))
        .unwrap();
        for declaration in CONVERSATION_DECLARATIONS {
            assert!(registry
                .binding(&BindingId::new(declaration.binding_id).unwrap())
                .is_some());
        }
        assert_eq!(registry.bindings().count(), CONVERSATION_DECLARATIONS.len());
    }
}
