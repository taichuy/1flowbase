use super::*;

impl<R, H> OrchestrationRuntimeService<R, H>
where
    R: ApplicationRepository
        + FlowRepository
        + OrchestrationRuntimeRepository
        + ModelDefinitionRepository
        + ModelProviderRepository
        + NodeContributionRepository
        + PluginRepository
        + Clone
        + Send
        + Sync
        + 'static,
    H: ProviderRuntimePort + CapabilityPluginRuntimePort + Clone,
{
    pub async fn start_node_debug_preview(
        &self,
        command: StartNodeDebugPreviewCommand,
    ) -> Result<domain::NodeDebugPreviewResult>
    where
        R: ApplicationJsDependencySelectionRepository
            + BillingRepository
            + crate::ports::FileManagementRepository,
    {
        if !command.input_payload.is_object() {
            return Err(anyhow!("input payload must be an object"));
        }
        let context = self
            .load_application_run_context(command.actor_user_id, command.application_id)
            .await?;
        let editor_state = self
            .repository
            .get_or_create_editor_state(
                context.actor.current_workspace_id,
                context.application.id,
                context.actor.user_id,
            )
            .await?;
        let ApplicationRunContext { actor, application } = context;
        let compile_context = self
            .build_compile_context(application.workspace_id, application.id)
            .await?;

        let preview_document = command
            .document_snapshot
            .as_ref()
            .unwrap_or(&editor_state.draft.document);

        let mut compiled_plan = match application.application_type {
            domain::ApplicationType::AgentLogs => {
                return Err(ControlPlaneError::InvalidInput("application_type").into())
            }
            domain::ApplicationType::AgentFlow => {
                orchestration_runtime::compiler::FlowCompiler::compile(
                    editor_state.flow.id,
                    &editor_state.draft.id.to_string(),
                    preview_document,
                    &compile_context,
                )?
            }
            domain::ApplicationType::Workflow => {
                orchestration_runtime::compiler::FlowCompiler::compile_workflow(
                    editor_state.flow.id,
                    &editor_state.draft.id.to_string(),
                    preview_document,
                    &compile_context,
                )?
            }
        };
        freeze_failover_queue_routes(
            &self.repository,
            application.workspace_id,
            &mut compiled_plan,
        )
        .await?;
        ensure_compiled_plan_runnable_for_node(&compiled_plan, &command.node_id)?;
        let mut preview_input = command.input_payload.clone();
        if application.application_type == domain::ApplicationType::Workflow {
            let variables = self
                .repository
                .list_application_environment_variables(application.workspace_id, application.id)
                .await?;
            preview_input = live_debug_run::freeze_run_input_environment(
                preview_input,
                &variables,
                &application,
            );
        }
        let started_at = OffsetDateTime::now_utc();
        let compiled_record = self
            .repository
            .upsert_compiled_plan(&build_compiled_plan_input(
                command.actor_user_id,
                &editor_state,
                &compiled_plan,
                preview_document,
            )?)
            .await?;
        let flow_run = self
            .repository
            .create_flow_run(&build_flow_run_input(
                command.actor_user_id,
                command.application_id,
                &editor_state,
                &compiled_record,
                &command,
                preview_document,
                started_at,
            ))
            .await?;
        // The create response does not include the authorized account. Freeze the
        // existing read-model attribution once, before provider execution; log
        // persistence consumes this snapshot without querying mutable user data.
        let flow_run = self
            .repository
            .get_flow_run(application.id, flow_run.id)
            .await?
            .ok_or(ControlPlaneError::NotFound("flow_run"))?;
        let flow_execution_context = self.runtime_flow_execution_context(
            actor.clone(),
            flow_run.authorized_account.clone(),
            application.id,
            editor_state.draft.id,
            flow_run.id,
            None,
        );
        let lifecycle = live_debug_run::PersistedNodeLifecycle::new(
            self,
            flow_run.id,
            flow_execution_context.clone(),
        );
        if application.application_type == domain::ApplicationType::Workflow {
            let pool = preview_input
                .as_object_mut()
                .expect("preview input is validated and environment freezing preserves an object");
            live_debug_run::inject_system_variables(
                pool,
                &flow_run,
                application.application_type,
                live_debug_run::compiled_plan_start_node_id(&compiled_plan),
            );
        }
        let invoker = self
            .runtime_invoker(application.workspace_id)
            .for_flow_run(flow_run.id)
            .with_provider_session_scope(&flow_run)
            .with_flow_execution_context(flow_execution_context);
        let http_file_persister = self.http_response_file_persister(actor);
        let preview_result = orchestration_runtime::preview_executor::run_node_preview_with_http_file_persister_and_counter_store_and_lifecycle(
            &compiled_plan,
            &command.node_id,
            &preview_input,
            &invoker,
            http_file_persister.as_ref().map(|persister| {
                persister as &dyn orchestration_runtime::execution_engine::HttpResponseFilePersister
            }),
            self.llm_routing_counter_store.clone(),
            &lifecycle,
        )
        .await;
        let preview = match preview_result {
            Ok(preview) => preview,
            Err(error) => {
                for node_run in lifecycle.prepared_node_runs()?.values() {
                    ensure_node_run_transition(
                        node_run.status,
                        domain::NodeRunStatus::Failed,
                        "fail_node_debug_preview",
                    )?;
                    self.repository
                        .complete_node_run(&crate::ports::CompleteNodeRunInput {
                            node_run_id: node_run.id,
                            status: domain::NodeRunStatus::Failed,
                            output_payload: json!({}),
                            error_payload: Some(json!({ "message": error.to_string() })),
                            metrics_payload: json!({}),
                            debug_payload: json!({}),
                            finished_at: OffsetDateTime::now_utc(),
                        })
                        .await?;
                }
                live_debug_run::fail_flow_run(self, command.application_id, flow_run.id, &error)
                    .await?;
                return Err(error);
            }
        };
        let node_run = match lifecycle.prepared_node_runs()?.remove(&command.node_id) {
            Some(node_run) => node_run,
            None => {
                self.repository
                    .create_node_run(&build_node_run_input(
                        flow_run.id,
                        &compiled_plan,
                        &command.node_id,
                        &preview,
                        started_at,
                    )?)
                    .await?
            }
        };
        let events =
            persist_preview_events(&self.repository, &flow_run, &node_run, &preview).await?;
        let finished_at = OffsetDateTime::now_utc();
        ensure_node_run_transition(
            node_run.status,
            if preview.is_failed() {
                domain::NodeRunStatus::Failed
            } else {
                domain::NodeRunStatus::Succeeded
            },
            "complete_node_debug_preview",
        )?;
        let node_run = self
            .repository
            .complete_node_run(&build_complete_node_run_input(
                &node_run,
                &preview,
                finished_at,
            ))
            .await?;
        ensure_flow_run_transition(
            flow_run.status,
            if preview.is_failed() {
                domain::FlowRunStatus::Failed
            } else {
                domain::FlowRunStatus::Succeeded
            },
            "complete_flow_debug_preview",
        )?;
        let completion = build_complete_flow_run_input(&flow_run, &preview, finished_at);
        let terminal_event = if completion.status == domain::FlowRunStatus::Failed {
            debug_stream_events::flow_failed(
                flow_run.id,
                completion
                    .error_payload
                    .clone()
                    .unwrap_or_else(|| json!({ "message": "node preview failed" })),
            )
        } else {
            debug_stream_events::flow_finished(flow_run.id, completion.output_payload.clone())
        };
        let result = if completion.status == domain::FlowRunStatus::Failed {
            CommitFlowRunTerminalResult::Failed {
                output_payload: completion.output_payload,
                error_payload: completion
                    .error_payload
                    .unwrap_or_else(|| json!({ "message": "node preview failed" })),
            }
        } else {
            CommitFlowRunTerminalResult::Succeeded {
                output_payload: completion.output_payload,
            }
        };
        let flow_run_event_payload = result
            .error_payload()
            .cloned()
            .unwrap_or_else(|| result.output_payload().clone());
        let receipt = self
            .repository
            .commit_flow_run_terminal(&CommitFlowRunTerminalInput {
                flow_run_id: flow_run.id,
                expected_status: flow_run.status,
                result,
                flow_run_event_payload,
                terminal_event_payload: terminal_event.payload,
                finished_at,
            })
            .await?;
        let flow_run = match stream_terminal_recovery::resolve_terminal_commit(
            &self.repository,
            command.application_id,
            flow_run.id,
            receipt,
        )
        .await?
        {
            stream_terminal_recovery::TerminalCommitResolution::Winner(flow_run)
            | stream_terminal_recovery::TerminalCommitResolution::Loser(flow_run) => flow_run,
        };
        let mut variable_cache = command
            .input_payload
            .as_object()
            .cloned()
            .unwrap_or_default();
        variable_cache.insert(node_run.node_id.clone(), node_run.output_payload.clone());
        let variable_cache = public_node_variable_cache(&compiled_plan, &variable_cache);
        persist_debug_variable_cache_entries(
            &self.repository,
            application.workspace_id,
            &flow_run,
            &variable_cache,
        )
        .await?;

        Ok(domain::NodeDebugPreviewResult {
            flow_run,
            node_run,
            events,
            preview_payload: preview.as_payload(),
        })
    }

}
