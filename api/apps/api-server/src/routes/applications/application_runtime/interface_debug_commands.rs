use std::{convert::Infallible, future::Future, pin::Pin, sync::Arc};

use axum::http::HeaderMap;
use axum::response::sse::Event;
use control_plane::{
    application::ApplicationService,
    errors::ControlPlaneError,
    orchestration_runtime::{
        debug_stream_events, project_runtime_event_stream_terminal,
        spawn_runtime_debug_event_persister, wait_for_runtime_debug_event_persister,
        CancelFlowRunCommand, CompleteCallbackTaskCommand, ContinueFlowDebugRunCommand,
        OrchestrationRuntimeService, PrepareFlowDebugRunCommand, ResumeFlowRunCommand,
        StartFlowDebugRunCommand, StartNodeDebugPreviewCommand,
    },
    ports::{ApplicationRepository, OrchestrationRuntimeRepository, RuntimeEventStreamPolicy},
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::{
    offload_application_run_detail_artifacts_with_dependencies, scope_application_activity,
    to_application_run_detail_response, to_node_last_run_response, ApplicationActivityKind,
    ApplicationRunDetailResponse, CompleteCallbackTaskBody, NodeLastRunResponse, ResumeFlowRunBody,
    StartFlowDebugRunBody, StartNodeDebugPreviewBody,
};
use crate::{
    app_state::ApiState,
    error_response::ApiError,
    routes::{
        console_interface::{
            self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
            ConsoleInterfaceTargetError, ConsoleServerStreamFuture, ConsoleServerStreamPort,
        },
        debug_run_stream,
        mcp_protocol::virtual_ui,
    },
    runtime_activity::ApplicationRuntimeActivityTracker,
};

pub(crate) enum ApplicationRuntimeDebugCommandsInput {
    Start {
        application_id: Uuid,
        body: StartFlowDebugRunBody,
        headers: HeaderMap,
    },
    Resume {
        application_id: Uuid,
        run_id: Uuid,
        body: ResumeFlowRunBody,
        headers: HeaderMap,
    },
    Cancel {
        application_id: Uuid,
        run_id: Uuid,
    },
    CompleteCallback {
        application_id: Uuid,
        callback_task_id: Uuid,
        body: CompleteCallbackTaskBody,
        headers: HeaderMap,
    },
    StartNode {
        application_id: Uuid,
        node_id: String,
        body: StartNodeDebugPreviewBody,
        headers: HeaderMap,
    },
}

impl InterfaceContract for ApplicationRuntimeDebugCommandsInput {
    const CONTRACT_ID: &'static str = "console-application-runtime-debug-commands-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[expect(
    clippy::large_enum_variant,
    reason = "the typed debug output is projected immediately into the console response"
)]
pub(crate) enum ApplicationRuntimeDebugCommandsOutput {
    Run(ApplicationRunDetailResponse),
    Node(NodeLastRunResponse),
}

#[expect(
    clippy::large_enum_variant,
    reason = "the stream command is consumed once by the typed debug adapter"
)]
pub(crate) enum ApplicationRuntimeDebugStreamInput {
    Start {
        application_id: Uuid,
        body: StartFlowDebugRunBody,
        headers: HeaderMap,
        stream_query: super::DebugRunStreamQuery,
    },
    Subscribe {
        application_id: Uuid,
        run_id: Uuid,
        from_sequence: Option<i64>,
    },
}

impl InterfaceContract for ApplicationRuntimeDebugStreamInput {
    const CONTRACT_ID: &'static str = "console-application-runtime-debug-stream-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct ApplicationRuntimeDebugStreamEvent(pub(crate) Result<Event, Infallible>);

impl InterfaceContract for ApplicationRuntimeDebugStreamEvent {
    const CONTRACT_ID: &'static str = "console-application-runtime-debug-stream-event";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct ApplicationRuntimeDebugStreamOutput;

impl InterfaceContract for ApplicationRuntimeDebugStreamOutput {
    const CONTRACT_ID: &'static str = "console-application-runtime-debug-stream-output";
    const CONTRACT_VERSION: &'static str = "1";
}

impl InterfaceContract for ApplicationRuntimeDebugCommandsOutput {
    const CONTRACT_ID: &'static str = "console-application-runtime-debug-commands-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) trait RuntimeDebugMcpFactory: Send + Sync + 'static {
    fn for_actor<'a>(
        &'a self,
        actor: &'a domain::ActorContext,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<virtual_ui::RuntimeInternalToolInvokerFactory, ApiError>>
                + Send
                + 'a,
        >,
    >;
}

struct StateRuntimeDebugMcpFactory {
    state: Arc<ApiState>,
}

impl RuntimeDebugMcpFactory for StateRuntimeDebugMcpFactory {
    fn for_actor<'a>(
        &'a self,
        actor: &'a domain::ActorContext,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<virtual_ui::RuntimeInternalToolInvokerFactory, ApiError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(crate::runtime_internal_tool_invoker_factory(
            &self.state,
            actor,
        ))
    }
}

#[derive(Clone)]
pub(crate) struct RuntimeDebugCommandDependencies {
    store: MainDurableStore,
    provider_runtime: Arc<crate::provider_runtime::ApiRuntimeServices>,
    runtime_activity: Arc<ApplicationRuntimeActivityTracker>,
    runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
    provider_secret_master_key: String,
    api_node_id: String,
    provider_install_root: String,
    file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
    cache_store: Arc<dyn crate::host_infrastructure::CacheStore>,
    task_queue: Arc<dyn crate::host_infrastructure::TaskQueue>,
    runtime_event_stream: Arc<dyn control_plane::ports::RuntimeEventStream>,
    assistant_executions:
        Arc<std::sync::Mutex<std::collections::HashMap<Uuid, tokio::task::AbortHandle>>>,
    mcp_factory: Arc<dyn RuntimeDebugMcpFactory>,
}

pub(crate) fn dependencies(state: Arc<ApiState>) -> RuntimeDebugCommandDependencies {
    RuntimeDebugCommandDependencies {
        store: state.store.clone(),
        provider_runtime: state.provider_runtime.clone(),
        runtime_activity: state.runtime_activity.clone(),
        runtime_engine: state.runtime_engine.clone(),
        provider_secret_master_key: state.provider_secret_master_key.clone(),
        api_node_id: state.api_node_id.clone(),
        provider_install_root: state.provider_install_root.clone(),
        file_storage_registry: state.file_storage_registry.clone(),
        cache_store: state.infrastructure.cache_store(),
        task_queue: state.infrastructure.task_queue(),
        runtime_event_stream: state.runtime_event_stream.clone(),
        assistant_executions: state.assistant_executions.clone(),
        mcp_factory: Arc::new(StateRuntimeDebugMcpFactory { state }),
    }
}

struct ApplicationRuntimeDebugCommandsAdapter {
    dependencies: RuntimeDebugCommandDependencies,
}

pub(crate) fn port(
    dependencies: RuntimeDebugCommandDependencies,
) -> Arc<
    dyn ConsoleInterfacePort<
        ApplicationRuntimeDebugCommandsInput,
        ApplicationRuntimeDebugCommandsOutput,
    >,
> {
    Arc::new(ApplicationRuntimeDebugCommandsAdapter { dependencies })
}

pub(crate) fn stream_port(
    dependencies: RuntimeDebugCommandDependencies,
) -> Arc<
    dyn ConsoleServerStreamPort<
        ApplicationRuntimeDebugStreamInput,
        ApplicationRuntimeDebugStreamEvent,
        ApplicationRuntimeDebugStreamOutput,
    >,
> {
    Arc::new(ApplicationRuntimeDebugCommandsAdapter { dependencies })
}

impl ApplicationRuntimeDebugCommandsAdapter {
    fn runtime(
        &self,
    ) -> OrchestrationRuntimeService<MainDurableStore, crate::provider_runtime::ApiProviderRuntime>
    {
        OrchestrationRuntimeService::new(
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
    }

    async fn mcp_invoker(
        &self,
        actor: &domain::ActorContext,
        headers: HeaderMap,
    ) -> Result<Arc<virtual_ui::ApiMcpRuntimeToolInvoker>, ApiError> {
        Ok(Arc::new(
            virtual_ui::ApiMcpRuntimeToolInvoker::new(
                self.dependencies.mcp_factory.for_actor(actor).await?,
                headers,
                actor.clone(),
                Vec::new(),
            )
            .await?,
        ))
    }

    async fn application(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
    ) -> Result<domain::ApplicationRecord, ApiError> {
        self.dependencies
            .store
            .get_application(actor.current_workspace_id, application_id)
            .await?
            .ok_or_else(|| ControlPlaneError::NotFound("application").into())
    }

    async fn offload(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        detail: domain::ApplicationRunDetail,
    ) -> Result<domain::ApplicationRunDetail, ApiError> {
        offload_application_run_detail_artifacts_with_dependencies(
            self.dependencies.store.clone(),
            self.dependencies.file_storage_registry.clone(),
            actor.current_workspace_id,
            application_id,
            detail,
        )
        .await
    }

    async fn start(
        &self,
        principal: &UserPrincipal,
        application_id: Uuid,
        body: StartFlowDebugRunBody,
        headers: HeaderMap,
    ) -> Result<ApplicationRunDetailResponse, ApiError> {
        let actor = principal.actor();
        let _http_activity = self
            .dependencies
            .runtime_activity
            .start(application_id, ApplicationActivityKind::HttpRequest);
        let mcp = self.mcp_invoker(actor, headers).await?;
        let detail = self
            .runtime()
            .with_runtime_internal_tool_invoker(mcp.clone())
            .start_flow_debug_run(StartFlowDebugRunCommand {
                actor_user_id: actor.user_id,
                application_id,
                input_payload: body.input_payload,
                document_snapshot: body.document,
                debug_session_id: body.debug_session_id,
            })
            .await?;
        let application = self.application(actor, application_id).await?;
        let dependencies = self.dependencies.clone();
        let workspace_id = actor.current_workspace_id;
        let flow_run_id = detail.flow_run.id;
        tokio::spawn(async move {
            let _execution_activity = dependencies.runtime_activity.start(
                application_id,
                ApplicationActivityKind::ApplicationExecution,
            );
            let service = OrchestrationRuntimeService::new(
                dependencies.store.clone(),
                crate::provider_runtime::ApiProviderRuntime::new_with_activity(
                    dependencies.provider_runtime.clone(),
                    dependencies.runtime_activity.clone(),
                ),
                dependencies.runtime_engine.clone(),
                dependencies.provider_secret_master_key.clone(),
            )
            .with_node_artifact_context(
                dependencies.api_node_id.clone(),
                dependencies.provider_install_root.clone(),
            )
            .with_file_storage_registry(dependencies.file_storage_registry.clone())
            .with_runtime_internal_tool_invoker(mcp)
            .with_llm_routing_counter_store(dependencies.cache_store.clone())
            .with_provider_request_log_queue(dependencies.task_queue.clone());
            match scope_application_activity(
                application_id,
                service.continue_flow_debug_run(ContinueFlowDebugRunCommand {
                    application_id,
                    flow_run_id,
                    workspace_id,
                }),
            )
            .await
            {
                Ok(detail) => {
                    if let Err(error) = offload_application_run_detail_artifacts_with_dependencies(
                        dependencies.store.clone(),
                        dependencies.file_storage_registry.clone(),
                        workspace_id,
                        application_id,
                        detail,
                    )
                    .await
                    {
                        tracing::error!(application_id = %application_id, flow_run_id = %flow_run_id, error = %error.0, "failed to offload flow debug artifacts");
                    }
                }
                Err(error) => {
                    tracing::error!(application_id = %application_id, flow_run_id = %flow_run_id, error = %error, "failed to continue flow debug run")
                }
            }
        });
        Ok(to_application_run_detail_response(&application, detail))
    }

    async fn resume(
        &self,
        principal: &UserPrincipal,
        application_id: Uuid,
        run_id: Uuid,
        body: ResumeFlowRunBody,
        headers: HeaderMap,
    ) -> Result<ApplicationRunDetailResponse, ApiError> {
        let actor = principal.actor();
        let _http_activity = self
            .dependencies
            .runtime_activity
            .start(application_id, ApplicationActivityKind::HttpRequest);
        let checkpoint_id = Uuid::parse_str(&body.checkpoint_id)
            .map_err(|_| ControlPlaneError::InvalidInput("checkpoint_id"))?;
        let detail = scope_application_activity(
            application_id,
            self.runtime()
                .with_runtime_internal_tool_invoker(self.mcp_invoker(actor, headers).await?)
                .resume_flow_run(ResumeFlowRunCommand {
                    actor_user_id: actor.user_id,
                    application_id,
                    flow_run_id: run_id,
                    checkpoint_id,
                    input_payload: body.input_payload,
                }),
        )
        .await?;
        let application = self.application(actor, application_id).await?;
        Ok(to_application_run_detail_response(
            &application,
            self.offload(actor, application_id, detail).await?,
        ))
    }

    async fn cancel(
        &self,
        principal: &UserPrincipal,
        application_id: Uuid,
        run_id: Uuid,
    ) -> Result<ApplicationRunDetailResponse, ApiError> {
        let actor = principal.actor();
        let application = self.application(actor, application_id).await?;
        self.dependencies
            .store
            .get_flow_run(application_id, run_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("flow_run"))?;
        crate::routes::assistant::abort_assistant_execution_in(
            &self.dependencies.assistant_executions,
            run_id,
        );
        let detail = self
            .runtime()
            .with_runtime_event_stream(self.dependencies.runtime_event_stream.clone())
            .cancel_flow_run(CancelFlowRunCommand {
                actor_user_id: actor.user_id,
                application_id,
                flow_run_id: run_id,
            })
            .await?;
        Ok(to_application_run_detail_response(
            &application,
            self.offload(actor, application_id, detail).await?,
        ))
    }

    async fn complete_callback(
        &self,
        principal: &UserPrincipal,
        application_id: Uuid,
        callback_task_id: Uuid,
        body: CompleteCallbackTaskBody,
        headers: HeaderMap,
    ) -> Result<ApplicationRunDetailResponse, ApiError> {
        let actor = principal.actor();
        let _http_activity = self
            .dependencies
            .runtime_activity
            .start(application_id, ApplicationActivityKind::HttpRequest);
        let detail = scope_application_activity(
            application_id,
            self.runtime()
                .with_runtime_internal_tool_invoker(self.mcp_invoker(actor, headers).await?)
                .complete_callback_task(CompleteCallbackTaskCommand {
                    actor_user_id: actor.user_id,
                    application_id,
                    callback_task_id,
                    response_payload: body.response_payload,
                }),
        )
        .await?;
        let application = self.application(actor, application_id).await?;
        Ok(to_application_run_detail_response(
            &application,
            self.offload(actor, application_id, detail).await?,
        ))
    }

    async fn start_node(
        &self,
        principal: &UserPrincipal,
        application_id: Uuid,
        node_id: String,
        body: StartNodeDebugPreviewBody,
        headers: HeaderMap,
    ) -> Result<NodeLastRunResponse, ApiError> {
        let actor = principal.actor();
        let _http_activity = self
            .dependencies
            .runtime_activity
            .start(application_id, ApplicationActivityKind::HttpRequest);
        let outcome = scope_application_activity(
            application_id,
            self.runtime()
                .with_runtime_internal_tool_invoker(self.mcp_invoker(actor, headers).await?)
                .start_node_debug_preview(StartNodeDebugPreviewCommand {
                    actor_user_id: actor.user_id,
                    application_id,
                    node_id,
                    input_payload: body.input_payload,
                    document_snapshot: body.document,
                    debug_session_id: body.debug_session_id,
                }),
        )
        .await?;
        let detail = self
            .offload(
                actor,
                application_id,
                domain::ApplicationRunDetail {
                    flow_run: outcome.flow_run,
                    node_runs: vec![outcome.node_run],
                    checkpoints: Vec::new(),
                    callback_tasks: Vec::new(),
                    events: outcome.events,
                    stitched_trace: Vec::new(),
                    subagent_traces: Vec::new(),
                },
            )
            .await?;
        let node_run = detail
            .node_runs
            .into_iter()
            .next()
            .ok_or(ControlPlaneError::NotFound("node_run"))?;
        Ok(to_node_last_run_response(domain::NodeLastRun {
            flow_run: detail.flow_run,
            node_run,
            checkpoints: Vec::new(),
            events: detail.events,
        }))
    }

    async fn subscribe_stream(
        &self,
        principal: &UserPrincipal,
        application_id: Uuid,
        run_id: Uuid,
        from_sequence: Option<i64>,
    ) -> Result<
        interface_runtime::InterfaceEventStream<
            ApplicationRuntimeDebugStreamEvent,
            ApplicationRuntimeDebugStreamOutput,
            ConsoleInterfaceTargetError,
        >,
        interface_runtime::InterfaceTargetFailure<ConsoleInterfaceTargetError>,
    > {
        let actor = principal.actor();
        ApplicationService::new(self.dependencies.store.for_actor(actor.clone()))
            .get_application(actor.user_id, application_id)
            .await
            .map_err(ApiError::from)
            .map_err(ConsoleInterfaceTargetError)
            .map_err(|error| {
                interface_runtime::InterfaceTargetFailure::new("console_interface", error)
            })?;
        let flow_run = self
            .dependencies
            .store
            .get_flow_run(application_id, run_id)
            .await
            .map_err(ApiError::from)
            .map_err(ConsoleInterfaceTargetError)
            .map_err(|error| {
                interface_runtime::InterfaceTargetFailure::new("console_interface", error)
            })?
            .ok_or(ControlPlaneError::NotFound("flow_run"))
            .map_err(ApiError::from)
            .map_err(ConsoleInterfaceTargetError)
            .map_err(|error| {
                interface_runtime::InterfaceTargetFailure::new("console_interface", error)
            })?;
        if flow_run.created_by != actor.user_id {
            return Err(interface_runtime::InterfaceTargetFailure::new(
                "console_interface",
                ConsoleInterfaceTargetError(ControlPlaneError::NotFound("flow_run").into()),
            ));
        }

        Ok(self.forward_runtime_stream(application_id, run_id, from_sequence))
    }

    fn forward_runtime_stream(
        &self,
        application_id: Uuid,
        run_id: Uuid,
        from_sequence: Option<i64>,
    ) -> interface_runtime::InterfaceEventStream<
        ApplicationRuntimeDebugStreamEvent,
        ApplicationRuntimeDebugStreamOutput,
        ConsoleInterfaceTargetError,
    > {
        let (publisher, stream) = interface_runtime::interface_stream_channel(32);
        let (sender, mut receiver) = tokio::sync::mpsc::channel(32);
        tokio::spawn(debug_run_stream::send_runtime_event_stream(
            self.dependencies.runtime_event_stream.clone(),
            Arc::new(self.dependencies.store.clone()),
            run_id,
            from_sequence,
            sender,
        ));
        let activity = self
            .dependencies
            .runtime_activity
            .start(application_id, ApplicationActivityKind::SseConnection);
        tokio::spawn(async move {
            let _activity = activity;
            while let Some(event) = receiver.recv().await {
                if publisher
                    .emit(ApplicationRuntimeDebugStreamEvent(event))
                    .await
                    .is_err()
                {
                    return;
                }
            }
            let _ = publisher
                .finish(interface_runtime::InterfaceStreamTerminal::Completed(
                    ApplicationRuntimeDebugStreamOutput,
                ))
                .await;
        });
        stream
    }

    async fn start_stream(
        &self,
        principal: &UserPrincipal,
        application_id: Uuid,
        body: StartFlowDebugRunBody,
        headers: HeaderMap,
        stream_query: super::DebugRunStreamQuery,
    ) -> Result<
        interface_runtime::InterfaceEventStream<
            ApplicationRuntimeDebugStreamEvent,
            ApplicationRuntimeDebugStreamOutput,
            ConsoleInterfaceTargetError,
        >,
        interface_runtime::InterfaceTargetFailure<ConsoleInterfaceTargetError>,
    > {
        let actor = principal.actor().clone();
        let _http_activity = self
            .dependencies
            .runtime_activity
            .start(application_id, ApplicationActivityKind::HttpRequest);
        let request_received_at = std::time::Instant::now();
        let mcp = self
            .mcp_invoker(&actor, headers.clone())
            .await
            .map_err(ConsoleInterfaceTargetError)
            .map_err(|error| {
                interface_runtime::InterfaceTargetFailure::new("console_interface", error)
            })?;
        let runtime = self
            .runtime()
            .with_runtime_internal_tool_invoker(mcp.clone());
        let shell = runtime
            .open_flow_debug_run_shell(StartFlowDebugRunCommand {
                actor_user_id: actor.user_id,
                application_id,
                input_payload: body.input_payload.clone(),
                document_snapshot: body.document.clone(),
                debug_session_id: body.debug_session_id.clone(),
            })
            .await
            .map_err(ApiError::from)
            .map_err(ConsoleInterfaceTargetError)
            .map_err(|error| {
                interface_runtime::InterfaceTargetFailure::new("console_interface", error)
            })?;
        let run_id = shell.id;
        let workspace_id = actor.current_workspace_id;
        let from_sequence = super::debug_run_stream_from_sequence(run_id, &stream_query, &headers);

        self.dependencies
            .runtime_event_stream
            .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
            .await
            .map_err(ApiError::from)
            .map_err(ConsoleInterfaceTargetError)
            .map_err(|error| {
                interface_runtime::InterfaceTargetFailure::new("console_interface", error)
            })?;
        let persister_handle = spawn_runtime_debug_event_persister(
            self.dependencies.store.clone(),
            self.dependencies.runtime_event_stream.clone(),
            run_id,
        );
        self.dependencies
            .runtime_event_stream
            .append(run_id, debug_stream_events::flow_accepted(run_id))
            .await
            .map_err(ApiError::from)
            .map_err(ConsoleInterfaceTargetError)
            .map_err(|error| {
                interface_runtime::InterfaceTargetFailure::new("console_interface", error)
            })?;
        self.dependencies
            .runtime_event_stream
            .append(run_id, debug_stream_events::heartbeat())
            .await
            .map_err(ApiError::from)
            .map_err(ConsoleInterfaceTargetError)
            .map_err(|error| {
                interface_runtime::InterfaceTargetFailure::new("console_interface", error)
            })?;

        let dependencies = self.dependencies.clone();
        tokio::spawn(async move {
            let _execution_activity = dependencies.runtime_activity.start(
                application_id,
                ApplicationActivityKind::ApplicationExecution,
            );
            let service = OrchestrationRuntimeService::new(
                dependencies.store.clone(),
                crate::provider_runtime::ApiProviderRuntime::new_with_activity(
                    dependencies.provider_runtime.clone(),
                    dependencies.runtime_activity.clone(),
                ),
                dependencies.runtime_engine.clone(),
                dependencies.provider_secret_master_key.clone(),
            )
            .with_node_artifact_context(
                dependencies.api_node_id.clone(),
                dependencies.provider_install_root.clone(),
            )
            .with_file_storage_registry(dependencies.file_storage_registry.clone())
            .with_runtime_internal_tool_invoker(mcp)
            .with_llm_routing_counter_store(dependencies.cache_store.clone())
            .with_provider_request_log_queue(dependencies.task_queue.clone())
            .with_runtime_event_stream(dependencies.runtime_event_stream.clone());
            let prepare_result = scope_application_activity(
                application_id,
                service.prepare_flow_debug_run_from_shell(PrepareFlowDebugRunCommand {
                    actor_user_id: actor.user_id,
                    application_id,
                    flow_run_id: run_id,
                    input_payload: body.input_payload,
                    document_snapshot: body.document,
                    debug_session_id: body.debug_session_id.unwrap_or_default(),
                }),
            )
            .await;
            let result = match prepare_result {
                Ok(_) => {
                    scope_application_activity(
                        application_id,
                        service.continue_flow_debug_run(ContinueFlowDebugRunCommand {
                            application_id,
                            flow_run_id: run_id,
                            workspace_id,
                        }),
                    )
                    .await
                }
                Err(error) => Err(error),
            };
            match result {
                Ok(detail) => {
                    if matches!(
                        detail.flow_run.status,
                        domain::FlowRunStatus::Succeeded
                            | domain::FlowRunStatus::Incomplete
                            | domain::FlowRunStatus::Failed
                            | domain::FlowRunStatus::Cancelled
                    ) {
                        project_runtime_event_stream_terminal(
                            dependencies.runtime_event_stream.clone(),
                            &detail.flow_run,
                        )
                        .await;
                    }
                    if let Err(error) = offload_application_run_detail_artifacts_with_dependencies(
                        dependencies.store.clone(),
                        dependencies.file_storage_registry.clone(),
                        workspace_id,
                        application_id,
                        detail,
                    )
                    .await
                    {
                        tracing::error!(application_id = %application_id, flow_run_id = %run_id, error = %error.0, "failed to offload streamed flow debug artifacts");
                    }
                }
                Err(error) => {
                    match dependencies
                        .store
                        .get_flow_run(application_id, run_id)
                        .await
                    {
                        Ok(Some(winner)) => {
                            project_runtime_event_stream_terminal(
                                dependencies.runtime_event_stream.clone(),
                                &winner,
                            )
                            .await;
                        }
                        Ok(None) => {
                            tracing::error!(application_id = %application_id, flow_run_id = %run_id, "failed streamed flow debug run has no durable winner to project")
                        }
                        Err(load_error) => {
                            tracing::error!(application_id = %application_id, flow_run_id = %run_id, error = %load_error, "failed to load durable winner for runtime stream projection")
                        }
                    }
                    tracing::error!(application_id = %application_id, flow_run_id = %run_id, error = %error, "failed to prepare and continue streamed flow debug run");
                }
            }
            wait_for_runtime_debug_event_persister(persister_handle, application_id, run_id).await;
        });
        tracing::info!(application_id = %application_id, flow_run_id = %run_id, http_to_sse_open_ms = request_received_at.elapsed().as_millis() as u64, "flow debug stream opened");
        Ok(self.forward_runtime_stream(application_id, run_id, from_sequence))
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: ApplicationRuntimeDebugCommandsInput,
    ) -> Result<ApplicationRuntimeDebugCommandsOutput, ApiError> {
        match input {
            ApplicationRuntimeDebugCommandsInput::Start {
                application_id,
                body,
                headers,
            } => Ok(ApplicationRuntimeDebugCommandsOutput::Run(
                self.start(principal, application_id, body, headers).await?,
            )),
            ApplicationRuntimeDebugCommandsInput::Resume {
                application_id,
                run_id,
                body,
                headers,
            } => Ok(ApplicationRuntimeDebugCommandsOutput::Run(
                self.resume(principal, application_id, run_id, body, headers)
                    .await?,
            )),
            ApplicationRuntimeDebugCommandsInput::Cancel {
                application_id,
                run_id,
            } => Ok(ApplicationRuntimeDebugCommandsOutput::Run(
                self.cancel(principal, application_id, run_id).await?,
            )),
            ApplicationRuntimeDebugCommandsInput::CompleteCallback {
                application_id,
                callback_task_id,
                body,
                headers,
            } => Ok(ApplicationRuntimeDebugCommandsOutput::Run(
                self.complete_callback(principal, application_id, callback_task_id, body, headers)
                    .await?,
            )),
            ApplicationRuntimeDebugCommandsInput::StartNode {
                application_id,
                node_id,
                body,
                headers,
            } => Ok(ApplicationRuntimeDebugCommandsOutput::Node(
                self.start_node(principal, application_id, node_id, body, headers)
                    .await?,
            )),
        }
    }
}

impl
    ConsoleInterfacePort<
        ApplicationRuntimeDebugCommandsInput,
        ApplicationRuntimeDebugCommandsOutput,
    > for ApplicationRuntimeDebugCommandsAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ApplicationRuntimeDebugCommandsInput,
    ) -> ConsoleInterfaceFuture<'a, ApplicationRuntimeDebugCommandsOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

impl
    ConsoleServerStreamPort<
        ApplicationRuntimeDebugStreamInput,
        ApplicationRuntimeDebugStreamEvent,
        ApplicationRuntimeDebugStreamOutput,
    > for ApplicationRuntimeDebugCommandsAdapter
{
    fn execute_stream(
        &self,
        principal: &UserPrincipal,
        input: ApplicationRuntimeDebugStreamInput,
    ) -> ConsoleServerStreamFuture<
        ApplicationRuntimeDebugStreamEvent,
        ApplicationRuntimeDebugStreamOutput,
    > {
        let dependencies = self.dependencies.clone();
        let principal = principal.clone();
        Box::pin(async move {
            let adapter = ApplicationRuntimeDebugCommandsAdapter { dependencies };
            match input {
                ApplicationRuntimeDebugStreamInput::Start {
                    application_id,
                    body,
                    headers,
                    stream_query,
                } => {
                    adapter
                        .start_stream(&principal, application_id, body, headers, stream_query)
                        .await
                }
                ApplicationRuntimeDebugStreamInput::Subscribe {
                    application_id,
                    run_id,
                    from_sequence,
                } => {
                    adapter
                        .subscribe_stream(&principal, application_id, run_id, from_sequence)
                        .await
                }
            }
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.debug-runs.create",
        binding_id: "http.console.applications.runtime.debug-runs.create.v1",
        method: "POST",
        path: "/api/console/applications/:id/orchestration/debug-runs",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.runs.resume",
        binding_id: "http.console.applications.runtime.runs.resume.v1",
        method: "POST",
        path: "/api/console/applications/:id/orchestration/runs/:run_id/resume",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.runs.cancel",
        binding_id: "http.console.applications.runtime.runs.cancel.v1",
        method: "POST",
        path: "/api/console/applications/:id/orchestration/runs/:run_id/cancel",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.callback-tasks.complete",
        binding_id: "http.console.applications.runtime.callback-tasks.complete.v1",
        method: "POST",
        path:
            "/api/console/applications/:id/orchestration/callback-tasks/:callback_task_id/complete",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.nodes.debug-runs.create",
        binding_id: "http.console.applications.runtime.nodes.debug-runs.create.v1",
        method: "POST",
        path: "/api/console/applications/:id/orchestration/nodes/:node_id/debug-runs",
        mutating: true,
    },
];

pub(crate) const STREAM_DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.debug-runs.stream.create",
        binding_id: "http.console.applications.runtime.debug-runs.stream.create.v1",
        method: "POST",
        path: "/api/console/applications/:id/orchestration/debug-runs/stream",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.debug-runs.stream.subscribe",
        binding_id: "http.console.applications.runtime.debug-runs.stream.subscribe.v1",
        method: "GET",
        path: "/api/console/applications/:id/orchestration/runs/:run_id/debug-stream",
        mutating: false,
    },
];

pub(crate) fn compile_registry(
    dependencies: RuntimeDebugCommandDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-application-runtime-debug-commands",
        "graph:console-application-runtime-debug-commands-v1",
        DECLARATIONS,
        port(dependencies),
    )
}

pub(crate) fn compile_stream_registry(
    dependencies: RuntimeDebugCommandDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_server_stream_registry(
        "api-server.console-application-runtime-debug-streams",
        "graph:console-application-runtime-debug-streams-v1",
        STREAM_DECLARATIONS,
        stream_port(dependencies),
    )
}
