use super::*;

pub(crate) fn application_runtime_target(
    state: &ApiState,
    registry: &interface_runtime::CompiledInterfaceRegistry,
    binding_id: &interface_runtime::BindingId,
    application_id: Uuid,
) -> interface_runtime::ExecutionTargetPin {
    let plan = registry
        .plan(binding_id)
        .expect("published Native binding must own a plan");
    interface_runtime::ExecutionTargetPin::Runtime {
        handler: plan.definition().handler_reference().clone(),
        target: plan.definition().target_reference().clone(),
        plugin: interface_runtime::PluginIdentity::new("builtin.orchestration-runtime")
            .expect("runtime plugin identity is valid"),
        artifact: interface_runtime::ArtifactIdentity::new(format!(
            "api-server:{}",
            env!("CARGO_PKG_VERSION")
        ))
        .expect("runtime artifact identity is valid"),
        runtime: interface_runtime::RuntimeTargetIdentity::new(format!(
            "orchestration-runtime.local.application:{application_id}"
        ))
        .expect("runtime target identity is valid"),
        runtime_generation: interface_runtime::RuntimeGeneration::new(
            state
                .extension_boot_snapshot
                .as_ref()
                .map(|snapshot| snapshot.fingerprint())
                .unwrap_or("graph:unavailable"),
        )
        .expect("runtime generation identity is valid"),
        worker_generation: interface_runtime::WorkerGeneration::new(format!(
            "api-process:{}",
            state.process_started_at.unix_timestamp_nanos()
        ))
        .expect("worker generation identity is valid"),
    }
}

pub(crate) fn include_workflow_events(
    request: &NativeRunRequest,
) -> Result<sse::IncludeWorkflowEvents, NativeApiError> {
    include_workflow_event_visibility(request.stream_options.include_workflow_events)
}

pub(crate) fn include_workflow_event_visibility(
    visibility: control_plane::application_public_api::native::NativeWorkflowEventVisibility,
) -> Result<sse::IncludeWorkflowEvents, NativeApiError> {
    match visibility {
        control_plane::application_public_api::native::NativeWorkflowEventVisibility::None => {
            Ok(sse::IncludeWorkflowEvents::None)
        }
        control_plane::application_public_api::native::NativeWorkflowEventVisibility::Public => {
            Ok(sse::IncludeWorkflowEvents::Public)
        }
        control_plane::application_public_api::native::NativeWorkflowEventVisibility::Debug => {
            Err(NativeApiError::new(
                StatusCode::FORBIDDEN,
                "workflow_event_visibility_forbidden",
                "debug workflow events require a browser session principal",
            ))
        }
    }
}

pub(super) async fn start_native_run_event_channel_for_actor_with_dependencies(
    dependencies: ApplicationNativeRunDependencies,
    actor: control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
    run: NativeRunResult,
    include_workflow_events: sse::IncludeWorkflowEvents,
) -> Result<
    mpsc::Receiver<Result<axum::response::sse::Event, std::convert::Infallible>>,
    NativeApiError,
> {
    let runtime_internal_tool_invoker =
        runtime_internal_tool_invoker_for_run(&dependencies, &actor, &run).await?;
    start_native_run_event_channel_with_dependencies(
        dependencies,
        runtime_internal_tool_invoker,
        run,
        include_workflow_events,
    )
    .await
}

pub(super) async fn start_native_run_event_channel_with_dependencies(
    dependencies: ApplicationNativeRunDependencies,
    runtime_internal_tool_invoker: Arc<
        dyn orchestration_runtime::execution_engine::RuntimeInternalToolInvoker,
    >,
    run: NativeRunResult,
    include_workflow_events: sse::IncludeWorkflowEvents,
) -> Result<
    mpsc::Receiver<Result<axum::response::sse::Event, std::convert::Infallible>>,
    NativeApiError,
> {
    dependencies
        .runtime_event_stream
        .open_run(
            run.id,
            control_plane::ports::RuntimeEventStreamPolicy::debug_default(),
        )
        .await
        .map_err(service_error)?;

    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(sse::send_native_runtime_event_stream_with_dependencies(
        native_run_sse_dependencies(&dependencies),
        run.clone(),
        include_workflow_events,
        None,
        None,
        sender,
    ));

    let background_dependencies = dependencies.clone();
    let background_run = run.clone();
    tokio::spawn(async move {
        let _execution_activity = background_dependencies.runtime_activity.start(
            background_run.application_id,
            ApplicationActivityKind::ApplicationExecution,
        );
        let runtime_service =
            native_runtime_service(&background_dependencies, runtime_internal_tool_invoker)
                .with_runtime_event_stream(background_dependencies.runtime_event_stream.clone());
        if let Err(runtime_error) = scope_application_activity(
            background_run.application_id,
            runtime_service.start_published_flow_run(StartPublishedFlowRunCommand {
                application_id: background_run.application_id,
                flow_run_id: background_run.id,
                provider_transport_slot: None,
                transport_connection_scope: None,
                observation_context: None,
            }),
        )
        .await
        {
            if let Err(recovery_error) = recover_missing_stream_terminal_winner_with_dependencies(
                &native_run_terminal_dependencies(&background_dependencies),
                &background_run,
            )
            .await
            {
                error!(
                    application_id = %background_run.application_id,
                    flow_run_id = %background_run.id,
                    error = %recovery_error,
                    "failed to recover the durable winner after native streaming execution ended"
                );
            }
            error!(
                application_id = %background_run.application_id,
                flow_run_id = %background_run.id,
                error = %runtime_error,
                "failed to execute native streaming published run"
            );
        }
    });

    Ok(receiver)
}
