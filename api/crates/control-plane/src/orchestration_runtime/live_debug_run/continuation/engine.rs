use super::completion::{finish_continued_flow_run, ContinuedFlowCompletion};
use super::helpers::*;
use super::waiting_nodes::{wait_for_human_input, wait_for_tool_callback, WaitingNodeContext};
use super::*;
use crate::orchestration_runtime::answer_presentation::AnswerPresentationCursor;

pub(super) async fn continue_flow_debug_run_inner<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    command: &ContinueFlowDebugRunCommand,
    live_provider_events: Option<LiveProviderStreamEventSender>,
) -> Result<domain::ApplicationRunDetail>
where
    R: crate::ports::ApplicationRepository
        + crate::ports::FileManagementRepository
        + crate::ports::FlowRepository
        + OrchestrationRuntimeRepository
        + crate::ports::ModelDefinitionRepository
        + crate::ports::ModelProviderRepository
        + crate::ports::NodeContributionRepository
        + crate::ports::PluginRepository
        + Clone
        + Send
        + Sync
        + 'static,
    H: crate::ports::ProviderRuntimePort
        + crate::capability_plugin_runtime::CapabilityPluginRuntimePort
        + Clone,
{
    let flow_run = service
        .repository
        .get_flow_run(command.application_id, command.flow_run_id)
        .await?
        .ok_or_else(|| anyhow!("flow run not found"))?;
    if flow_run.status != domain::FlowRunStatus::Running {
        return load_run_detail(&service.repository, command.application_id, flow_run.id).await;
    }
    let actor = crate::ports::ApplicationRepository::load_actor_context_for_user(
        &service.repository,
        flow_run.created_by,
    )
    .await?;
    let application = service
        .repository
        .get_application(command.workspace_id, command.application_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("application"))?;
    let compiled_plan_id = flow_run
        .compiled_plan_id
        .ok_or_else(|| anyhow!("flow run compiled plan is not attached"))?;
    let compiled_record = service
        .repository
        .get_compiled_plan(compiled_plan_id)
        .await?
        .ok_or_else(|| anyhow!("compiled plan not found"))?;
    let compiled_plan: orchestration_runtime::compiled_plan::CompiledPlan =
        serde_json::from_value(compiled_record.plan)?;
    let answer_presentation = AnswerPresentationCursor::from_plan(&compiled_plan)
        .map(|cursor| Arc::new(Mutex::new(cursor)));
    let invoker = if let Some(live_provider_events) = live_provider_events {
        service.runtime_invoker_with_live_provider_events(
            application.workspace_id,
            live_provider_events,
        )
    } else {
        service.runtime_invoker(application.workspace_id)
    }
    .for_flow_run(flow_run.id);
    let invoker = if let Some(answer_presentation) = &answer_presentation {
        invoker.with_answer_presentation(answer_presentation.clone())
    } else {
        invoker
    };
    let mut variable_pool = flow_run
        .input_payload
        .as_object()
        .cloned()
        .ok_or_else(|| anyhow!("input payload must be an object"))?;
    if !variable_pool.contains_key("env") {
        let environment_variables = service
            .repository
            .list_application_environment_variables(application.workspace_id, application.id)
            .await?;
        inject_application_environment_variables(&mut variable_pool, &environment_variables);
    }
    inject_system_variables(
        &mut variable_pool,
        &flow_run,
        application.application_type,
        compiled_plan_start_node_id(&compiled_plan),
    );
    let runtime_context = service.execution_runtime_context(&compiled_plan, &variable_pool);
    let mut active_node_ids =
        orchestration_runtime::execution_engine::branching::initial_active_node_ids(&compiled_plan);
    let mut last_output_payload = json!({});
    let mut pending_failure: Option<Value> = None;
    let flow_span = append_host_span(
        &service.repository,
        AppendHostSpanInput {
            flow_run_id: flow_run.id,
            node_run_id: None,
            parent_span_id: None,
            kind: domain::RuntimeSpanKind::Flow,
            name: "debug flow".to_string(),
            started_at: flow_run.started_at,
            metadata: json!({
                "application_id": command.application_id,
                "run_mode": flow_run.run_mode.as_str(),
                "trigger_event_type": "flow_run_continued",
            }),
        },
    )
    .await?;

    for (node_index, node_id) in compiled_plan.topological_order.iter().enumerate() {
        if is_run_cancelled(&service.repository, command.application_id, flow_run.id).await? {
            return load_run_detail(&service.repository, command.application_id, flow_run.id).await;
        }

        let node = compiled_plan
            .nodes
            .get(node_id)
            .ok_or_else(|| anyhow!("compiled node missing: {node_id}"))?;
        if !active_node_ids.contains(node_id) {
            continue;
        }
        let resolved_inputs =
            orchestration_runtime::binding_runtime::resolve_node_inputs(node, &variable_pool)?;
        let rendered_templates = orchestration_runtime::binding_runtime::render_templated_bindings(
            node,
            &resolved_inputs,
        );
        let node_started_at = OffsetDateTime::now_utc();
        let node_input_payload = if matches!(node.node_type.as_str(), "start" | "workflow_start") {
            start_node_input_payload(&variable_pool, node_id)
        } else {
            Value::Object(resolved_inputs.clone())
        };
        let node_run = service
            .repository
            .create_node_run(&CreateNodeRunInput {
                flow_run_id: flow_run.id,
                node_id: node.node_id.clone(),
                node_type: node.node_type.clone(),
                node_alias: node.alias.clone(),
                status: domain::NodeRunStatus::Running,
                input_payload: node_input_payload,
                debug_payload: json!({}),
                started_at: node_started_at,
            })
            .await?;
        append_runtime_event(
            service,
            flow_run.id,
            debug_stream_events::node_started(&node_run),
        )
        .await;
        let node_span = append_host_span(
            &service.repository,
            AppendHostSpanInput {
                flow_run_id: flow_run.id,
                node_run_id: Some(node_run.id),
                parent_span_id: Some(flow_span.id),
                kind: if node.node_type == "llm" {
                    domain::RuntimeSpanKind::LlmTurn
                } else {
                    domain::RuntimeSpanKind::Node
                },
                name: node.alias.clone(),
                started_at: node_started_at,
                metadata: json!({
                    "node_id": node.node_id,
                    "node_type": node.node_type,
                }),
            },
        )
        .await?;

        let mut selected_source_handle: Option<String> = None;

        match node.node_type.as_str() {
            "start" | "workflow_start" => {
                last_output_payload = variable_pool
                    .get(node_id)
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                update_node_run_and_emit(
                    service,
                    flow_run.id,
                    &UpdateNodeRunInput {
                        node_run_id: node_run.id,
                        status: domain::NodeRunStatus::Succeeded,
                        output_payload: json!({}),
                        error_payload: None,
                        metrics_payload: json!({ "preview_mode": true }),
                        debug_payload: json!({}),
                        finished_at: Some(OffsetDateTime::now_utc()),
                    },
                )
                .await?;
                emit_answer_presentation_for_node(
                    service,
                    flow_run.id,
                    answer_presentation.as_ref(),
                    &node.node_id,
                    node_run.id,
                    &last_output_payload,
                )
                .await;
            }
            "workflow_end" => {
                let output_payload =
                    project_node_variable_payload(node, &Value::Object(resolved_inputs.clone()))?;
                last_output_payload = output_payload.clone();
                variable_pool.insert(node.node_id.clone(), output_payload.clone());
                update_node_run_and_emit(
                    service,
                    flow_run.id,
                    &UpdateNodeRunInput {
                        node_run_id: node_run.id,
                        status: domain::NodeRunStatus::Succeeded,
                        output_payload,
                        error_payload: None,
                        metrics_payload: json!({ "preview_mode": true }),
                        debug_payload: json!({}),
                        finished_at: Some(OffsetDateTime::now_utc()),
                    },
                )
                .await?;
            }
            "if_else" => {
                selected_source_handle =
                    orchestration_runtime::execution_engine::branching::select_if_else_source_handle(
                        node,
                        &variable_pool,
                    )?;
                update_node_run_and_emit(
                    service,
                    flow_run.id,
                    &UpdateNodeRunInput {
                        node_run_id: node_run.id,
                        status: domain::NodeRunStatus::Succeeded,
                        output_payload: json!({}),
                        error_payload: None,
                        metrics_payload: json!({ "preview_mode": true }),
                        debug_payload: json!({
                            "selected_source_handle": selected_source_handle.clone(),
                        }),
                        finished_at: Some(OffsetDateTime::now_utc()),
                    },
                )
                .await?;
            }
            "llm" => {
                let persist_text_run_events = service.runtime_event_stream.is_none();
                let (persist_sender, persist_receiver) = mpsc::unbounded_channel();
                let persist_handle = tokio::spawn(run_live_event_persister(
                    service.repository.clone(),
                    flow_run.id,
                    node_run.id,
                    node_span.id,
                    persist_text_run_events,
                    persist_receiver,
                ));
                let llm_invoker = invoker.for_live_llm_node_with_persist(
                    node.node_id.clone(),
                    node_run.id,
                    persist_sender,
                );
                let execution_result = orchestration_runtime::execution_engine::execute_llm_node(
                    &compiled_plan,
                    node,
                    &resolved_inputs,
                    &rendered_templates,
                    &variable_pool,
                    &runtime_context,
                    &llm_invoker,
                )
                .await;
                drop(llm_invoker);
                persist_handle
                    .await
                    .map_err(|e| anyhow!("persist task panicked: {e}"))??;
                let execution = execution_result?;
                let mut debug_payload = execution.debug_payload.clone();

                let public_output_payload = persisted_node_output_payload(
                    &execution.output_payload,
                    &execution.metrics_payload,
                    execution.error_payload.as_ref(),
                    &execution.debug_payload,
                );
                let refs = persist_llm_context_observability(
                    &service.repository,
                    application.workspace_id,
                    &application.name,
                    service.provider_request_log_queue.as_ref(),
                    flow_run.id,
                    flow_run.application_id,
                    flow_run.external_conversation_id.as_deref(),
                    node_run.id,
                    node_span.id,
                    Value::Object(resolved_inputs.clone()),
                    &execution.metrics_payload,
                    execution.error_payload.as_ref(),
                )
                .await?;
                apply_llm_debug_observability_refs(&mut debug_payload, &refs);

                if let Some(error_payload) = execution.error_payload.clone() {
                    variable_pool.insert(node.node_id.clone(), public_output_payload.clone());
                    ensure_node_run_transition(
                        domain::NodeRunStatus::Running,
                        domain::NodeRunStatus::Failed,
                        "continue_flow_debug_run",
                    )?;
                    update_node_run_and_emit(
                        service,
                        flow_run.id,
                        &UpdateNodeRunInput {
                            node_run_id: node_run.id,
                            status: domain::NodeRunStatus::Failed,
                            output_payload: public_output_payload.clone(),
                            error_payload: Some(error_payload.clone()),
                            metrics_payload: execution.metrics_payload.clone(),
                            debug_payload: debug_payload.clone(),
                            finished_at: Some(OffsetDateTime::now_utc()),
                        },
                    )
                    .await?;
                    emit_answer_presentation_for_node(
                        service,
                        flow_run.id,
                        answer_presentation.as_ref(),
                        &node.node_id,
                        node_run.id,
                        &public_output_payload,
                    )
                    .await;
                    let mut next_active_node_ids = active_node_ids.clone();
                    orchestration_runtime::execution_engine::branching::activate_downstream_nodes(
                        &compiled_plan,
                        &mut next_active_node_ids,
                        node,
                        selected_source_handle.as_deref(),
                    );
                    if can_continue_to_terminal_template_nodes(
                        &compiled_plan,
                        node_index,
                        &next_active_node_ids,
                    ) {
                        active_node_ids = next_active_node_ids;
                        pending_failure = Some(error_payload.clone());
                        continue;
                    }
                    return fail_current_live_run_after_node_error(
                        service,
                        command,
                        &flow_run,
                        node_run.id,
                        last_output_payload.clone(),
                        error_payload,
                    )
                    .await;
                }

                let pending_callback = execution.pending_callback.clone().or_else(|| {
                    orchestration_runtime::execution_engine::build_llm_tool_callback_wait(
                        node,
                        &resolved_inputs,
                        &variable_pool,
                        &execution.output_payload,
                    )
                });
                if let Some(wait) = pending_callback {
                    let checkpoint_variable_pool = wait.checkpoint_variable_pool.clone();
                    ensure_node_run_transition(
                        domain::NodeRunStatus::Running,
                        domain::NodeRunStatus::WaitingCallback,
                        "continue_flow_debug_run",
                    )?;
                    update_node_run_and_emit(
                        service,
                        flow_run.id,
                        &UpdateNodeRunInput {
                            node_run_id: node_run.id,
                            status: domain::NodeRunStatus::WaitingCallback,
                            output_payload: public_output_payload,
                            error_payload: None,
                            metrics_payload: execution.metrics_payload.clone(),
                            debug_payload: debug_payload.clone(),
                            finished_at: None,
                        },
                    )
                    .await?;

                    if is_run_cancelled(&service.repository, command.application_id, flow_run.id)
                        .await?
                    {
                        return load_run_detail(
                            &service.repository,
                            command.application_id,
                            flow_run.id,
                        )
                        .await;
                    }

                    ensure_flow_run_transition(
                        domain::FlowRunStatus::Running,
                        domain::FlowRunStatus::WaitingCallback,
                        "continue_flow_debug_run",
                    )?;
                    let answer_output_payload = materialize_ready_answer_node_run(
                        service,
                        flow_run.id,
                        &compiled_plan,
                        &checkpoint_variable_pool,
                    )
                    .await?
                    .unwrap_or_else(|| json!({}));
                    service
                        .repository
                        .create_checkpoint(&CreateCheckpointInput {
                            flow_run_id: flow_run.id,
                            node_run_id: Some(node_run.id),
                            status: "waiting_callback".to_string(),
                            reason: "等待 LLM 工具回调".to_string(),
                            locator_payload: CheckpointLocatorPayload::from_runtime_position(
                                &wait.node_id,
                                node_index,
                                orchestration_runtime::execution_engine::branching::checkpoint_active_node_ids(
                                    &active_node_ids,
                                ),
                            )
                            .into_json(),
                            variable_snapshot: Value::Object(wait.checkpoint_variable_pool),
                            external_ref_payload: Some(wait.request_payload.clone()),
                        })
                        .await?;
                    let callback_task = service
                        .repository
                        .create_callback_task(&CreateCallbackTaskInput {
                            flow_run_id: flow_run.id,
                            node_run_id: node_run.id,
                            callback_kind: "llm_tool_calls".to_string(),
                            request_payload: wait.request_payload.clone(),
                            external_ref_payload: Some(wait.request_payload),
                        })
                        .await?;
                    if service
                        .repository
                        .update_flow_run_if_status(
                            &UpdateFlowRunInput {
                                flow_run_id: flow_run.id,
                                status: domain::FlowRunStatus::WaitingCallback,
                                output_payload: answer_output_payload,
                                error_payload: None,
                                finished_at: None,
                            },
                            domain::FlowRunStatus::Running,
                        )
                        .await?
                        .is_none()
                    {
                        return load_run_detail(
                            &service.repository,
                            command.application_id,
                            flow_run.id,
                        )
                        .await;
                    }
                    append_runtime_event(
                        service,
                        flow_run.id,
                        debug_stream_events::waiting_callback_with_task(
                            flow_run.id,
                            node_run.id,
                            &wait.node_id,
                            &callback_task,
                        ),
                    )
                    .await;
                    close_runtime_event_stream(
                        service,
                        flow_run.id,
                        RuntimeEventCloseReason::WaitingCallback,
                    )
                    .await;
                    return load_run_detail(
                        &service.repository,
                        command.application_id,
                        flow_run.id,
                    )
                    .await;
                }

                ensure_node_run_transition(
                    domain::NodeRunStatus::Running,
                    domain::NodeRunStatus::Succeeded,
                    "continue_flow_debug_run",
                )?;
                update_node_run_and_emit(
                    service,
                    flow_run.id,
                    &UpdateNodeRunInput {
                        node_run_id: node_run.id,
                        status: domain::NodeRunStatus::Succeeded,
                        output_payload: public_output_payload.clone(),
                        error_payload: None,
                        metrics_payload: execution.metrics_payload.clone(),
                        debug_payload: debug_payload.clone(),
                        finished_at: Some(OffsetDateTime::now_utc()),
                    },
                )
                .await?;
                if is_run_cancelled(&service.repository, command.application_id, flow_run.id)
                    .await?
                {
                    return load_run_detail(
                        &service.repository,
                        command.application_id,
                        flow_run.id,
                    )
                    .await;
                }
                last_output_payload = public_output_payload.clone();
                variable_pool.insert(node.node_id.clone(), public_output_payload);
                emit_answer_presentation_for_node(
                    service,
                    flow_run.id,
                    answer_presentation.as_ref(),
                    &node.node_id,
                    node_run.id,
                    &last_output_payload,
                )
                .await;
            }
            "plugin_node" => {
                let execution =
                    orchestration_runtime::execution_engine::execute_capability_plugin_node(
                        node,
                        &resolved_inputs,
                        &rendered_templates,
                        &invoker,
                    )
                    .await?;
                let public_output_payload = persisted_node_output_payload(
                    &execution.output_payload,
                    &execution.metrics_payload,
                    execution.error_payload.as_ref(),
                    &execution.debug_payload,
                );
                let node_status = if execution.error_payload.is_some() {
                    domain::NodeRunStatus::Failed
                } else {
                    domain::NodeRunStatus::Succeeded
                };
                ensure_node_run_transition(
                    domain::NodeRunStatus::Running,
                    node_status,
                    "continue_flow_debug_run",
                )?;
                update_node_run_and_emit(
                    service,
                    flow_run.id,
                    &UpdateNodeRunInput {
                        node_run_id: node_run.id,
                        status: node_status,
                        output_payload: public_output_payload.clone(),
                        error_payload: execution.error_payload.clone(),
                        metrics_payload: execution.metrics_payload.clone(),
                        debug_payload: execution.debug_payload.clone(),
                        finished_at: Some(OffsetDateTime::now_utc()),
                    },
                )
                .await?;

                if is_run_cancelled(&service.repository, command.application_id, flow_run.id)
                    .await?
                {
                    return load_run_detail(
                        &service.repository,
                        command.application_id,
                        flow_run.id,
                    )
                    .await;
                }

                if let Some(error_payload) = execution.error_payload {
                    ensure_flow_run_transition(
                        domain::FlowRunStatus::Running,
                        domain::FlowRunStatus::Failed,
                        "continue_flow_debug_run",
                    )?;
                    if service
                        .repository
                        .update_flow_run_if_status(
                            &UpdateFlowRunInput {
                                flow_run_id: flow_run.id,
                                status: domain::FlowRunStatus::Failed,
                                output_payload: last_output_payload.clone(),
                                error_payload: Some(error_payload.clone()),
                                finished_at: Some(OffsetDateTime::now_utc()),
                            },
                            domain::FlowRunStatus::Running,
                        )
                        .await?
                        .is_none()
                    {
                        return load_run_detail(
                            &service.repository,
                            command.application_id,
                            flow_run.id,
                        )
                        .await;
                    }
                    emit_flow_failed_and_close(service, flow_run.id, error_payload.clone()).await;
                    service
                        .repository
                        .append_run_event(&AppendRunEventInput {
                            flow_run_id: flow_run.id,
                            node_run_id: Some(node_run.id),
                            event_type: "flow_run_failed".to_string(),
                            payload: error_payload,
                        })
                        .await?;
                    return load_run_detail(
                        &service.repository,
                        command.application_id,
                        flow_run.id,
                    )
                    .await;
                }

                last_output_payload = public_output_payload.clone();
                variable_pool.insert(node.node_id.clone(), public_output_payload);
            }
            "data_model_list" | "data_model_get" | "data_model_create" | "data_model_update"
            | "data_model_delete" => {
                let execution = data_model_runtime::execute_data_model_node(
                    service.repository.clone(),
                    service.runtime_engine.clone(),
                    &actor,
                    node,
                    &resolved_inputs,
                    &data_model_runtime::DataModelRunContext {
                        workspace_id: application.workspace_id,
                        application_id: command.application_id,
                        draft_id: flow_run.draft_id,
                        flow_run_id: flow_run.id,
                        node_run_id: node_run.id,
                    },
                )
                .await;
                if let Some(confirmation) = execution.waiting_confirmation {
                    ensure_node_run_transition(
                        domain::NodeRunStatus::Running,
                        domain::NodeRunStatus::WaitingCallback,
                        "continue_flow_debug_run",
                    )?;
                    update_node_run_and_emit(
                        service,
                        flow_run.id,
                        &UpdateNodeRunInput {
                            node_run_id: node_run.id,
                            status: domain::NodeRunStatus::WaitingCallback,
                            output_payload: json!({}),
                            error_payload: None,
                            metrics_payload: execution.metrics_payload.clone(),
                            debug_payload: json!({
                                "side_effect_policy": "confirm_each_run",
                                "idempotency_key": confirmation.idempotency_key,
                                "payload_hash": confirmation.payload_hash,
                                "expires_at": confirmation.expires_at,
                            }),
                            finished_at: None,
                        },
                    )
                    .await?;

                    if is_run_cancelled(&service.repository, command.application_id, flow_run.id)
                        .await?
                    {
                        return load_run_detail(
                            &service.repository,
                            command.application_id,
                            flow_run.id,
                        )
                        .await;
                    }

                    ensure_flow_run_transition(
                        domain::FlowRunStatus::Running,
                        domain::FlowRunStatus::WaitingCallback,
                        "continue_flow_debug_run",
                    )?;
                    let answer_output_payload = materialize_ready_answer_node_run(
                        service,
                        flow_run.id,
                        &compiled_plan,
                        &variable_pool,
                    )
                    .await?
                    .unwrap_or_else(|| json!({}));
                    let confirmation_payload = json!({
                        "kind": "data_model_side_effect_confirmation",
                        "actor_user_id": actor.user_id,
                        "node_id": node.node_id,
                        "run_id": flow_run.id,
                        "payload_hash": confirmation.payload_hash,
                        "idempotency_key": confirmation.idempotency_key,
                        "expires_at": confirmation.expires_at,
                        "request_payload": confirmation.request_payload,
                    });
                    let next_index = next_node_index(&compiled_plan, node_id)?;
                    orchestration_runtime::execution_engine::branching::activate_downstream_nodes(
                        &compiled_plan,
                        &mut active_node_ids,
                        node,
                        None,
                    );
                    service
                        .repository
                        .create_checkpoint(&CreateCheckpointInput {
                            flow_run_id: flow_run.id,
                            node_run_id: Some(node_run.id),
                            status: "waiting_data_model_side_effect_confirmation".to_string(),
                            reason: "等待 Data Model 写入确认".to_string(),
                            locator_payload: CheckpointLocatorPayload::from_runtime_position(
                                &node.node_id,
                                next_index,
                                orchestration_runtime::execution_engine::branching::checkpoint_active_node_ids(
                                    &active_node_ids,
                                ),
                            )
                            .into_json(),
                            variable_snapshot: Value::Object(variable_pool.clone()),
                            external_ref_payload: Some(confirmation_payload.clone()),
                        })
                        .await?;
                    let callback_task = service
                        .repository
                        .create_callback_task(&CreateCallbackTaskInput {
                            flow_run_id: flow_run.id,
                            node_run_id: node_run.id,
                            callback_kind: "data_model_side_effect_confirmation".to_string(),
                            request_payload: confirmation_payload.clone(),
                            external_ref_payload: Some(confirmation_payload),
                        })
                        .await?;
                    if service
                        .repository
                        .update_flow_run_if_status(
                            &UpdateFlowRunInput {
                                flow_run_id: flow_run.id,
                                status: domain::FlowRunStatus::WaitingCallback,
                                output_payload: answer_output_payload,
                                error_payload: None,
                                finished_at: None,
                            },
                            domain::FlowRunStatus::Running,
                        )
                        .await?
                        .is_none()
                    {
                        return load_run_detail(
                            &service.repository,
                            command.application_id,
                            flow_run.id,
                        )
                        .await;
                    }
                    append_runtime_event(
                        service,
                        flow_run.id,
                        debug_stream_events::waiting_callback_with_task(
                            flow_run.id,
                            node_run.id,
                            &node.node_id,
                            &callback_task,
                        ),
                    )
                    .await;
                    close_runtime_event_stream(
                        service,
                        flow_run.id,
                        RuntimeEventCloseReason::WaitingCallback,
                    )
                    .await;
                    return load_run_detail(
                        &service.repository,
                        command.application_id,
                        flow_run.id,
                    )
                    .await;
                }
                let public_output_payload = persisted_node_output_payload(
                    &execution.output_payload,
                    &execution.metrics_payload,
                    execution.error_payload.as_ref(),
                    &json!({}),
                );
                let node_status = if execution.error_payload.is_some() {
                    domain::NodeRunStatus::Failed
                } else {
                    domain::NodeRunStatus::Succeeded
                };
                ensure_node_run_transition(
                    domain::NodeRunStatus::Running,
                    node_status,
                    "continue_flow_debug_run",
                )?;
                update_node_run_and_emit(
                    service,
                    flow_run.id,
                    &UpdateNodeRunInput {
                        node_run_id: node_run.id,
                        status: node_status,
                        output_payload: public_output_payload.clone(),
                        error_payload: execution.error_payload.clone(),
                        metrics_payload: execution.metrics_payload.clone(),
                        debug_payload: json!({}),
                        finished_at: Some(OffsetDateTime::now_utc()),
                    },
                )
                .await?;

                if let Some(error_payload) = execution.error_payload {
                    ensure_flow_run_transition(
                        domain::FlowRunStatus::Running,
                        domain::FlowRunStatus::Failed,
                        "continue_flow_debug_run",
                    )?;
                    if service
                        .repository
                        .update_flow_run_if_status(
                            &UpdateFlowRunInput {
                                flow_run_id: flow_run.id,
                                status: domain::FlowRunStatus::Failed,
                                output_payload: last_output_payload.clone(),
                                error_payload: Some(error_payload.clone()),
                                finished_at: Some(OffsetDateTime::now_utc()),
                            },
                            domain::FlowRunStatus::Running,
                        )
                        .await?
                        .is_none()
                    {
                        return load_run_detail(
                            &service.repository,
                            command.application_id,
                            flow_run.id,
                        )
                        .await;
                    }
                    emit_flow_failed_and_close(service, flow_run.id, error_payload.clone()).await;
                    service
                        .repository
                        .append_run_event(&AppendRunEventInput {
                            flow_run_id: flow_run.id,
                            node_run_id: Some(node_run.id),
                            event_type: "flow_run_failed".to_string(),
                            payload: error_payload,
                        })
                        .await?;
                    return load_run_detail(
                        &service.repository,
                        command.application_id,
                        flow_run.id,
                    )
                    .await;
                }

                last_output_payload = public_output_payload.clone();
                variable_pool.insert(node.node_id.clone(), public_output_payload);
            }
            "template_transform" | "answer" => {
                let output_key = first_output_key(node);
                let output_value =
                    rendered_templates
                        .values()
                        .next()
                        .cloned()
                        .unwrap_or_else(|| {
                            resolved_inputs
                                .values()
                                .next()
                                .cloned()
                                .unwrap_or(Value::Null)
                        });
                let output_payload =
                    template_output_payload(node, output_key, output_value, &variable_pool);
                last_output_payload = output_payload.clone();
                variable_pool.insert(node.node_id.clone(), output_payload.clone());
                update_node_run_and_emit(
                    service,
                    flow_run.id,
                    &UpdateNodeRunInput {
                        node_run_id: node_run.id,
                        status: domain::NodeRunStatus::Succeeded,
                        output_payload,
                        error_payload: None,
                        metrics_payload: json!({ "preview_mode": true }),
                        debug_payload: json!({}),
                        finished_at: Some(OffsetDateTime::now_utc()),
                    },
                )
                .await?;
            }
            "human_input" => {
                return wait_for_human_input(
                    WaitingNodeContext {
                        service,
                        command,
                        flow_run: &flow_run,
                        compiled_plan: &compiled_plan,
                        variable_pool: &variable_pool,
                        active_node_ids: &active_node_ids,
                        node_id,
                        node,
                        node_run: &node_run,
                    },
                    &rendered_templates,
                )
                .await;
            }
            "tool" => {
                return wait_for_tool_callback(
                    WaitingNodeContext {
                        service,
                        command,
                        flow_run: &flow_run,
                        compiled_plan: &compiled_plan,
                        variable_pool: &variable_pool,
                        active_node_ids: &active_node_ids,
                        node_id,
                        node,
                        node_run: &node_run,
                    },
                    &resolved_inputs,
                )
                .await;
            }
            "http_request" => {
                let http_file_persister = service.http_response_file_persister(actor.clone());
                let execution = orchestration_runtime::execution_engine::execute_http_request_node(
                    node,
                    &resolved_inputs,
                    &variable_pool,
                    http_file_persister.as_ref().map(|persister| {
                        persister
                            as &dyn orchestration_runtime::execution_engine::HttpResponseFilePersister
                    }),
                )
                .await?;
                let public_output_payload = persisted_node_output_payload(
                    &execution.output_payload,
                    &execution.metrics_payload,
                    execution.error_payload.as_ref(),
                    &execution.debug_payload,
                );
                let node_status = if execution.error_payload.is_some() {
                    domain::NodeRunStatus::Failed
                } else {
                    domain::NodeRunStatus::Succeeded
                };
                ensure_node_run_transition(
                    domain::NodeRunStatus::Running,
                    node_status,
                    "continue_flow_debug_run",
                )?;
                update_node_run_and_emit(
                    service,
                    flow_run.id,
                    &UpdateNodeRunInput {
                        node_run_id: node_run.id,
                        status: node_status,
                        output_payload: public_output_payload.clone(),
                        error_payload: execution.error_payload.clone(),
                        metrics_payload: execution.metrics_payload.clone(),
                        debug_payload: execution.debug_payload.clone(),
                        finished_at: Some(OffsetDateTime::now_utc()),
                    },
                )
                .await?;

                if is_run_cancelled(&service.repository, command.application_id, flow_run.id)
                    .await?
                {
                    return load_run_detail(
                        &service.repository,
                        command.application_id,
                        flow_run.id,
                    )
                    .await;
                }

                if let Some(error_payload) = execution.error_payload {
                    ensure_flow_run_transition(
                        domain::FlowRunStatus::Running,
                        domain::FlowRunStatus::Failed,
                        "continue_flow_debug_run",
                    )?;
                    if service
                        .repository
                        .update_flow_run_if_status(
                            &UpdateFlowRunInput {
                                flow_run_id: flow_run.id,
                                status: domain::FlowRunStatus::Failed,
                                output_payload: last_output_payload.clone(),
                                error_payload: Some(error_payload.clone()),
                                finished_at: Some(OffsetDateTime::now_utc()),
                            },
                            domain::FlowRunStatus::Running,
                        )
                        .await?
                        .is_none()
                    {
                        return load_run_detail(
                            &service.repository,
                            command.application_id,
                            flow_run.id,
                        )
                        .await;
                    }
                    emit_flow_failed_and_close(service, flow_run.id, error_payload.clone()).await;
                    service
                        .repository
                        .append_run_event(&AppendRunEventInput {
                            flow_run_id: flow_run.id,
                            node_run_id: Some(node_run.id),
                            event_type: "flow_run_failed".to_string(),
                            payload: error_payload,
                        })
                        .await?;
                    return load_run_detail(
                        &service.repository,
                        command.application_id,
                        flow_run.id,
                    )
                    .await;
                }

                last_output_payload = public_output_payload.clone();
                variable_pool.insert(node.node_id.clone(), public_output_payload);
            }
            "code" => {
                let execution = orchestration_runtime::execution_engine::execute_code_node(
                    node,
                    &resolved_inputs,
                    &invoker,
                )
                .await?;
                let public_output_payload = persisted_node_output_payload(
                    &execution.output_payload,
                    &execution.metrics_payload,
                    execution.error_payload.as_ref(),
                    &execution.debug_payload,
                );
                let node_status = if execution.error_payload.is_some() {
                    domain::NodeRunStatus::Failed
                } else {
                    domain::NodeRunStatus::Succeeded
                };
                ensure_node_run_transition(
                    domain::NodeRunStatus::Running,
                    node_status,
                    "continue_flow_debug_run",
                )?;
                update_node_run_and_emit(
                    service,
                    flow_run.id,
                    &UpdateNodeRunInput {
                        node_run_id: node_run.id,
                        status: node_status,
                        output_payload: public_output_payload.clone(),
                        error_payload: execution.error_payload.clone(),
                        metrics_payload: execution.metrics_payload.clone(),
                        debug_payload: execution.debug_payload.clone(),
                        finished_at: Some(OffsetDateTime::now_utc()),
                    },
                )
                .await?;

                if is_run_cancelled(&service.repository, command.application_id, flow_run.id)
                    .await?
                {
                    return load_run_detail(
                        &service.repository,
                        command.application_id,
                        flow_run.id,
                    )
                    .await;
                }

                if let Some(error_payload) = execution.error_payload {
                    ensure_flow_run_transition(
                        domain::FlowRunStatus::Running,
                        domain::FlowRunStatus::Failed,
                        "continue_flow_debug_run",
                    )?;
                    if service
                        .repository
                        .update_flow_run_if_status(
                            &UpdateFlowRunInput {
                                flow_run_id: flow_run.id,
                                status: domain::FlowRunStatus::Failed,
                                output_payload: last_output_payload.clone(),
                                error_payload: Some(error_payload.clone()),
                                finished_at: Some(OffsetDateTime::now_utc()),
                            },
                            domain::FlowRunStatus::Running,
                        )
                        .await?
                        .is_none()
                    {
                        return load_run_detail(
                            &service.repository,
                            command.application_id,
                            flow_run.id,
                        )
                        .await;
                    }
                    emit_flow_failed_and_close(service, flow_run.id, error_payload.clone()).await;
                    service
                        .repository
                        .append_run_event(&AppendRunEventInput {
                            flow_run_id: flow_run.id,
                            node_run_id: Some(node_run.id),
                            event_type: "flow_run_failed".to_string(),
                            payload: error_payload,
                        })
                        .await?;
                    return load_run_detail(
                        &service.repository,
                        command.application_id,
                        flow_run.id,
                    )
                    .await;
                }

                last_output_payload = public_output_payload.clone();
                variable_pool.insert(node.node_id.clone(), public_output_payload);
            }
            other => {
                let error_payload = orchestration_runtime::node_errors::build_node_type_not_implemented_error_payload(
                    other,
                    "debug",
                );
                update_node_run_and_emit(
                    service,
                    flow_run.id,
                    &UpdateNodeRunInput {
                        node_run_id: node_run.id,
                        status: domain::NodeRunStatus::Failed,
                        output_payload: json!({}),
                        error_payload: Some(error_payload.clone()),
                        metrics_payload: json!({ "preview_mode": true }),
                        debug_payload: json!({}),
                        finished_at: Some(OffsetDateTime::now_utc()),
                    },
                )
                .await?;

                return Err(anyhow!("{}", error_payload));
            }
        }
        orchestration_runtime::execution_engine::branching::activate_downstream_nodes(
            &compiled_plan,
            &mut active_node_ids,
            node,
            selected_source_handle.as_deref(),
        );
    }

    finish_continued_flow_run(ContinuedFlowCompletion {
        service,
        command,
        flow_run: &flow_run,
        workspace_id: application.workspace_id,
        compiled_plan: &compiled_plan,
        variable_pool: &variable_pool,
        last_output_payload,
        pending_failure,
    })
    .await
}
