use super::*;

pub(in super::super) async fn launch_assistant_execution(
    dependencies: AssistantRunDependencies,
    execution: PreparedAssistantExecution,
    client_tool_bridge: Option<Arc<AssistantClientToolBridge>>,
) -> Result<Uuid, ApiError> {
    let run_id = execution.flow_run_id;
    let application_id = execution.application_id;
    dependencies
        .runtime_event_stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await?;
    let persister_handle = start_runtime_debug_event_persister(
        dependencies.store.clone(),
        dependencies.runtime_event_stream.clone(),
        run_id,
    )
    .await;
    dependencies
        .runtime_event_stream
        .append(run_id, debug_stream_events::flow_accepted(run_id))
        .await?;
    dependencies
        .runtime_event_stream
        .append(run_id, debug_stream_events::heartbeat())
        .await?;

    spawn_assistant_conversation_projection(
        dependencies.store.clone(),
        dependencies.runtime_event_stream.clone(),
        dependencies.conversation_events.clone(),
        AssistantConversationEventScope {
            workspace_id: execution.actor.current_workspace_id,
            application_id,
            actor_user_id: execution.actor.user_id,
        },
        execution.conversation_id,
        run_id,
    );

    if let Some(client) = client_tool_bridge.as_ref() {
        dependencies
            .assistant_client_sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(run_id, client.clone());
    }

    let mcp_client_bridge = client_tool_bridge.clone();
    let mcp_runtime_invoker: Arc<
        dyn orchestration_runtime::execution_engine::RuntimeInternalToolInvoker,
    > = Arc::new(
        virtual_ui::ApiMcpRuntimeToolInvoker::new(
            dependencies
                .runtime_tool_invoker_factory
                .for_actor(&execution.actor)
                .await?,
            execution.request_headers.clone(),
            execution.actor.clone(),
            execution.mcp_instance_ids.clone(),
        )
        .await?
        .with_assistant_client(mcp_client_bridge),
    );
    let assistant_runtime_tool_invoker = Arc::new(AssistantRuntimeToolInvoker::new(
        mcp_runtime_invoker,
        client_tool_bridge.clone(),
    ));
    let background_dependencies = dependencies.clone();
    let execution_registry = dependencies.assistant_executions.clone();
    let client_session_registry = dependencies.assistant_client_sessions.clone();
    let (start_sender, start_receiver) = oneshot::channel();
    let execution_handle = tokio::spawn(async move {
        if start_receiver.await.is_err() {
            return;
        }
        let runtime = OrchestrationRuntimeService::new(
            background_dependencies.store.clone(),
            crate::provider_runtime::ApiProviderRuntime::new_with_activity(
                background_dependencies.provider_runtime.clone(),
                background_dependencies.runtime_activity.clone(),
            ),
            background_dependencies.runtime_engine.clone(),
            background_dependencies.provider_secret_master_key.clone(),
            background_dependencies.provider_transport_store.clone(),
            background_dependencies.model_billing_require_provider_usage,
        )
        .with_published_plan_cache(background_dependencies.published_plan_cache.clone())
        .with_node_artifact_context(
            background_dependencies.api_node_id.clone(),
            background_dependencies.provider_install_root.clone(),
        )
        .with_file_storage_registry(background_dependencies.file_storage_registry.clone())
        .with_notification_sender(background_dependencies.notification_sender.clone())
        .with_runtime_internal_tool_invoker(assistant_runtime_tool_invoker)
        .with_llm_routing_counter_store(background_dependencies.cache_store.clone())
        .with_provider_request_log_queue(background_dependencies.task_queue.clone())
        .with_runtime_event_stream(background_dependencies.runtime_event_stream.clone());
        let result = async {
            let detail = runtime
                .start_published_flow_run(StartPublishedFlowRunCommand {
                    transport_connection_scope: None,
                    observation_context: None,
                    application_id: execution.application_id,
                    flow_run_id: execution.flow_run_id,
                    provider_transport_slot: None,
                })
                .await?;
            Ok::<domain::ApplicationRunDetail, ApiError>(detail)
        }
        .await;

        match result {
            Ok(detail)
                if matches!(
                    detail.flow_run.status,
                    domain::FlowRunStatus::Succeeded
                        | domain::FlowRunStatus::Incomplete
                        | domain::FlowRunStatus::Failed
                        | domain::FlowRunStatus::Cancelled
                ) =>
            {
                project_runtime_event_stream_terminal(
                    background_dependencies.runtime_event_stream.clone(),
                    &detail.flow_run,
                )
                .await;
            }
            Ok(_) => {}
            Err(error) => {
                match background_dependencies
                    .store
                    .get_flow_run(application_id, run_id)
                    .await
                {
                    Ok(Some(winner)) => {
                        project_runtime_event_stream_terminal(
                            background_dependencies.runtime_event_stream.clone(),
                            &winner,
                        )
                        .await;
                    }
                    Ok(None) => tracing::error!(
                        application_id = %application_id,
                        flow_run_id = %run_id,
                        "assistant stream failure has no durable winner to project"
                    ),
                    Err(load_error) => tracing::error!(
                        application_id = %application_id,
                        flow_run_id = %run_id,
                        error = %load_error,
                        "failed to load assistant stream durable winner"
                    ),
                }
                tracing::error!(
                    application_id = %application_id,
                    flow_run_id = %run_id,
                    error = ?error,
                    "assistant streamed run failed"
                );
            }
        }
        wait_for_runtime_debug_event_persister(persister_handle, application_id, run_id).await;
        execution_registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&run_id);
        let client_session = client_session_registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&run_id);
        if let Some(client_session) = client_session {
            client_session.close().await;
        }
    });
    dependencies
        .assistant_executions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(run_id, execution_handle.abort_handle());
    // The task cannot enter the Provider/runtime boundary until its abort handle is visible to
    // every cancellation entry point.
    let _ = start_sender.send(());
    Ok(run_id)
}

pub(crate) async fn execute_assistant_run(
    dependencies: &AssistantRunDependencies,
    principal: &interface_runtime::UserPrincipal,
    body: StartAssistantRunBody,
    headers: HeaderMap,
) -> Result<AssistantRunResponse, ApiError> {
    let execution = prepare_assistant_execution(
        &dependencies.store,
        &dependencies.published_plan_cache,
        &dependencies.published_publication_cache,
        &headers,
        principal.actor(),
        body,
    )
    .await?;
    let mcp_runtime_invoker = Arc::new(
        virtual_ui::ApiMcpRuntimeToolInvoker::new(
            dependencies
                .runtime_tool_invoker_factory
                .for_actor(&execution.actor)
                .await?,
            execution.request_headers.clone(),
            execution.actor.clone(),
            execution.mcp_instance_ids.clone(),
        )
        .await?,
    );
    let runtime = OrchestrationRuntimeService::new(
        dependencies.store.clone(),
        crate::provider_runtime::ApiProviderRuntime::new_with_activity(
            dependencies.provider_runtime.clone(),
            dependencies.runtime_activity.clone(),
        ),
        dependencies.runtime_engine.clone(),
        dependencies.provider_secret_master_key.clone(),
        dependencies.provider_transport_store.clone(),
        dependencies.model_billing_require_provider_usage,
    )
    .with_published_plan_cache(dependencies.published_plan_cache.clone())
    .with_node_artifact_context(
        dependencies.api_node_id.clone(),
        dependencies.provider_install_root.clone(),
    )
    .with_file_storage_registry(dependencies.file_storage_registry.clone())
    .with_notification_sender(dependencies.notification_sender.clone())
    .with_runtime_internal_tool_invoker(mcp_runtime_invoker)
    .with_llm_routing_counter_store(dependencies.cache_store.clone())
    .with_provider_request_log_queue(dependencies.task_queue.clone());
    let detail = runtime
        .start_published_flow_run(StartPublishedFlowRunCommand {
            transport_connection_scope: None,
            observation_context: None,
            application_id: execution.application_id,
            flow_run_id: execution.flow_run_id,
            provider_transport_slot: None,
        })
        .await?;
    publish_assistant_conversation_summary(
        &dependencies.store,
        &dependencies.conversation_events,
        AssistantConversationEventScope {
            workspace_id: principal.actor().current_workspace_id,
            application_id: execution.application_id,
            actor_user_id: principal.actor().user_id,
        },
        execution.conversation_id,
        AssistantConversationEventKind::Updated,
    )
    .await;
    let native_result =
        control_plane::application_public_api::run_service::native_result_from_run_detail(
            &detail,
            json!({}),
        );
    Ok(AssistantRunResponse {
        id: detail.flow_run.id,
        application_id: execution.application_id,
        conversation_id: execution.conversation_id,
        status: detail.flow_run.status.as_str().to_string(),
        answer: native_result.answer,
        output_payload: detail.flow_run.output_payload,
        error_payload: detail.flow_run.error_payload,
    })
}

pub(super) fn spawn_assistant_conversation_projection(
    store: MainDurableStore,
    runtime_event_stream: Arc<dyn RuntimeEventStream>,
    conversation_events: Arc<conversation_events::AssistantConversationEventHub>,
    scope: AssistantConversationEventScope,
    conversation_id: Uuid,
    run_id: Uuid,
) {
    tokio::spawn(async move {
        let Ok(mut subscription) = runtime_event_stream.subscribe(run_id, Some(0)).await else {
            tracing::warn!(
                application_id = %scope.application_id,
                flow_run_id = %run_id,
                "assistant conversation projection could not subscribe to runtime events"
            );
            return;
        };

        for event in subscription.replay {
            let terminal =
                RuntimeEventCloseReason::from_terminal_event_type(&event.event_type).is_some();
            if should_publish_assistant_conversation_event(&event.event_type) {
                publish_assistant_conversation_summary(
                    &store,
                    &conversation_events,
                    scope,
                    conversation_id,
                    AssistantConversationEventKind::Updated,
                )
                .await;
            }
            if terminal {
                return;
            }
        }

        while let Some(event) = subscription.live_events.recv().await {
            let terminal =
                RuntimeEventCloseReason::from_terminal_event_type(&event.event_type).is_some();
            if should_publish_assistant_conversation_event(&event.event_type) {
                publish_assistant_conversation_summary(
                    &store,
                    &conversation_events,
                    scope,
                    conversation_id,
                    AssistantConversationEventKind::Updated,
                )
                .await;
            }
            if terminal {
                return;
            }
        }
    });
}

pub(super) fn should_publish_assistant_conversation_event(event_type: &str) -> bool {
    matches!(
        event_type,
        "flow_accepted"
            | "flow_started"
            | "flow_finished"
            | "flow_incomplete"
            | "flow_failed"
            | "flow_cancelled"
            | "waiting_human"
            | "waiting_callback"
    )
}

pub(super) async fn publish_assistant_conversation_summary(
    store: &MainDurableStore,
    conversation_events: &conversation_events::AssistantConversationEventHub,
    scope: AssistantConversationEventScope,
    conversation_id: Uuid,
    kind: AssistantConversationEventKind,
) {
    match store
        .get_assistant_conversation_summary(
            scope.workspace_id,
            scope.application_id,
            scope.actor_user_id,
            conversation_id,
        )
        .await
    {
        Ok(Some(summary)) => conversation_events.publish(
            scope,
            kind,
            AssistantConversationSummaryResponse::from(summary),
        ),
        Ok(None) => tracing::warn!(
            application_id = %scope.application_id,
            conversation_id = %conversation_id,
            "assistant conversation projection could not find its durable summary"
        ),
        Err(error) => tracing::warn!(
            application_id = %scope.application_id,
            conversation_id = %conversation_id,
            error = %error,
            "assistant conversation projection could not load its durable summary"
        ),
    }
}

pub(super) fn abort_registered_assistant_execution(
    executions: &std::sync::Mutex<std::collections::HashMap<Uuid, tokio::task::AbortHandle>>,
    run_id: Uuid,
) -> Option<tokio::task::AbortHandle> {
    executions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&run_id)
}

pub(crate) fn abort_assistant_execution_in(
    executions: &Mutex<HashMap<Uuid, tokio::task::AbortHandle>>,
    run_id: Uuid,
) -> bool {
    let handle = abort_registered_assistant_execution(executions, run_id);
    if let Some(handle) = handle {
        handle.abort();
        true
    } else {
        false
    }
}
