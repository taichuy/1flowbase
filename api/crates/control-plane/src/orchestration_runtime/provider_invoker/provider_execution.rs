use super::*;

#[async_trait]
impl<R, H> orchestration_runtime::execution_engine::ProviderInvoker for RuntimeProviderInvoker<R, H>
where
    R: ModelProviderRepository
        + OrchestrationRuntimeRepository
        + PluginRepository
        + Clone
        + Send
        + Sync
        + 'static,
    H: ProviderRuntimePort + Clone + Send + Sync,
{
    async fn provider_distribution_registry_fingerprint(&self) -> Result<String> {
        self.runtime
            .provider_distribution_registry_fingerprint()
            .await
    }

    async fn select_provider_distribution(
        &self,
        plugin_id: &str,
        invocation: extension_contracts::ProviderDistributionInvocation,
    ) -> Result<extension_contracts::ProviderDistributionSelectionReceipt> {
        self.runtime
            .select_provider_distribution(
                plugin_id,
                invocation,
                crate::ports::ProviderRuntimeExecutionContext {
                    workspace_id: self.workspace_id,
                    actor_id: None,
                    deadline_unix_ms: time::OffsetDateTime::now_utc().unix_timestamp() * 1000
                        + 300_000,
                },
            )
            .await
    }

    async fn pipeline_provider_input(
        &self,
        mut input: ProviderInvocationInput,
    ) -> std::result::Result<
        orchestration_runtime::provider_input_pipeline::ProviderInputPipelineOutput,
        orchestration_runtime::provider_input_pipeline::ProviderInputPipelineError,
    > {
        if self.continuation_affinity().is_some() {
            input.required_capabilities.insert(
                plugin_framework::provider_contract::ProviderInvocationCapability::NativeContinuationSupported,
            );
        }
        self.runtime.pipeline_provider_input(input).await
    }

    async fn acquire_http_node_client(
        &self,
        timeout: std::time::Duration,
        verify_ssl: bool,
    ) -> Result<Option<orchestration_runtime::execution_engine::HttpRequestClientLease>> {
        self.runtime
            .acquire_http_node_client(self.workspace_id, timeout, verify_ssl)
            .await
    }

    async fn compact(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        mut input: ProviderInvocationInput,
    ) -> Result<plugin_framework::provider_contract::ProviderCompactResult> {
        self.apply_provider_transport(runtime, &mut input)?;
        let instance = self.resolve_llm_instance(runtime).await?;
        let installation = self.ready_installation(instance.installation_id).await?;
        let package = load_installed_provider_package(&installation)?;
        input.provider_config = build_provider_runtime_config(
            &self.repository,
            &self.runtime,
            &self.provider_secret_master_key,
            &package,
            &installation,
            &instance,
        )
        .await?;

        if let Some(scope) = &self.transport_connection_scope_override {
            apply_transport_connection_scope_override(&mut input, scope.as_deref());
        }
        let active_node = self
            .flow_execution_context
            .as_ref()
            .and_then(|context| context.active_node.lock().ok()?.clone())
            .or_else(|| {
                Some(RuntimeActiveNode {
                    node_id: self.active_node_id.clone()?,
                    node_run_id: self.active_node_run_id?,
                })
            });
        let capture = native_trajectory::start(
            self.repository.clone(),
            self.flow_run_id,
            active_node.map(|node| (node.node_id, node.node_run_id)),
            &input,
            self.observation_context.as_ref(),
        )
        .await;
        let result = self.runtime.compact(&installation, input).await;
        capture
            .finish_compact(result.as_ref().ok(), result.is_err())
            .await;
        result
    }

    async fn count_tokens(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        mut input: plugin_framework::provider_contract::ProviderCountTokensInput,
    ) -> Result<plugin_framework::provider_contract::ProviderCountTokensResult> {
        let attempted = async {
            let instance = self.resolve_llm_instance(runtime).await?;
            let installation = self.ready_installation(instance.installation_id).await?;
            let package = load_installed_provider_package(&installation)?;
            input.set_provider_config(
                build_provider_runtime_config(
                    &self.repository,
                    &self.runtime,
                    &self.provider_secret_master_key,
                    &package,
                    &installation,
                    &instance,
                )
                .await?,
            );
            self.runtime
                .count_tokens(&installation, input.clone())
                .await
        }
        .await;

        match attempted {
            Ok(result) => Ok(result),
            Err(_) => Ok(match estimate_provider_count_tokens(input.as_invocation()) {
                Ok(mut result) => {
                    result.fallback_reason =
                        Some(ProviderCountTokensFallbackReason::PluginUnavailable);
                    result
                }
                Err(_) => plugin_framework::provider_contract::ProviderCountTokensResult::fallback_zero(),
            }),
        }
    }

    async fn resolve_protocol_context_locator(&self, locator: &Value) -> Result<Option<Value>> {
        self.open_protocol_context_locator_value(locator).await
    }

    async fn get_session_llm_provider(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        node_id: &str,
    ) -> Result<Option<String>> {
        self.session_provider_owner(runtime, node_id).await
    }

    async fn bind_session_llm_provider(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        node_id: &str,
        selected: &str,
    ) -> Result<String> {
        let Some(scope) = self.session_provider_scope(runtime, node_id) else {
            return Ok(selected.to_owned());
        };
        let selected = Uuid::parse_str(selected)
            .map_err(|_| ControlPlaneError::InvalidInput("provider_instance_id"))?;
        Ok(self
            .repository
            .bind_model_provider_session(self.workspace_id, &scope, selected)
            .await?
            .to_string())
    }

    async fn resolve_main_llm_routing(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        node_id: &str,
    ) -> Result<Option<orchestration_runtime::execution_engine::ResolvedMainLlmRouting>> {
        if !runtime.targets_main_instance() {
            return Ok(None);
        }
        let mut routing = self
            .resolve_current_main_llm_routing(runtime, node_id)
            .await?;
        if self.continuation_affinity().is_some() {
            routing.candidates.retain(|candidate| {
                self.ensure_continuation_route(
                    &candidate.runtime,
                    &candidate.route.runtime_capabilities,
                )
                .is_ok()
            });
            if routing.candidates.is_empty() {
                return Err(provider_transport_error(
                    ProviderRuntimeErrorKind::ProviderAffinityMismatch,
                    "main Provider routing has no legal continuation owner",
                ));
            }
        }
        Ok(Some(routing))
    }

    async fn resolve_llm_route(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
    ) -> Result<orchestration_runtime::execution_engine::ResolvedProviderRoute> {
        let route = self.resolve_registered_llm_route(runtime).await?;
        if let Some(affinity) = self.continuation_affinity() {
            self.ensure_provider_affinity(runtime, affinity)?;
        }
        Ok(route)
    }

    async fn invoke_llm(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        input: ProviderInvocationInput,
    ) -> Result<orchestration_runtime::execution_engine::ProviderInvocationOutput> {
        let resolved_route = self.resolve_llm_route(runtime).await?;
        self.invoke_resolved_llm(runtime, resolved_route, input)
            .await
    }

    fn prepare_selected_llm_input(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        route: &orchestration_runtime::execution_engine::ResolvedProviderRoute,
        input: &mut ProviderInvocationInput,
    ) -> Result<orchestration_runtime::execution_engine::ai_native_recovery::RecoveryInputMode>
    {
        self.ensure_continuation_route(runtime, &route.runtime_capabilities)?;
        self.apply_selected_provider_transport(runtime, &route.runtime_capabilities, input)?;
        Ok(if input.native_transport.is_some() {
            orchestration_runtime::execution_engine::ai_native_recovery::RecoveryInputMode::NativeOpaque
        } else {
            orchestration_runtime::execution_engine::ai_native_recovery::RecoveryInputMode::SemanticMapped
        })
    }

    async fn invoke_resolved_llm(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        resolved_route: orchestration_runtime::execution_engine::ResolvedProviderRoute,
        mut input: ProviderInvocationInput,
    ) -> Result<orchestration_runtime::execution_engine::ProviderInvocationOutput> {
        let selected_mode =
            self.prepare_selected_llm_input(runtime, &resolved_route, &mut input)?;
        let selected_native_transport = selected_mode == orchestration_runtime::execution_engine::ai_native_recovery::RecoveryInputMode::NativeOpaque;
        let pin = resolved_route
            .invocation_pin::<RuntimeProviderInvocationPin>()
            .ok_or(ControlPlaneError::InvalidInput("resolved_provider_route"))?
            .clone();
        let instance = pin.instance;
        if let Some(recovery) = input.run_context.get("native_inference_recovery") {
            let expected = recovery.get("provider_instance_id").and_then(Value::as_str);
            if expected != Some(instance.id.to_string().as_str()) {
                return Err(anyhow!("provider_affinity_mismatch"));
            }
        }
        let installation = pin.installation;
        let package = pin.package;
        adapt_or_ensure_model_supports_content_blocks(
            &self.repository,
            &instance,
            &package,
            &runtime.model,
            &mut input,
        )
        .await?;

        let runtime_config_started = std::time::Instant::now();
        input.provider_config = build_provider_runtime_config(
            &self.repository,
            &self.runtime,
            &self.provider_secret_master_key,
            &package,
            &installation,
            &instance,
        )
        .await?;
        tracing::debug!(
            runtime_config_ms = runtime_config_started.elapsed().as_millis() as u64,
            "runtime config finished"
        );

        // Host seals exact effective provider configuration without persisting credentials.
        let native_inference_binding = json!({
            "provider_configuration_digest": format!("sha256:{:x}", base16ct::HexDisplay(&Sha256::digest(
                serde_json::to_vec(&input.provider_config)?))),
            "provider_instance_id": instance.id.to_string(),
            "node_id": input.trace_context.get("node_id"),
            "provider_code": input.provider_code,
            "protocol": input.protocol,
            "model": input.model,
            "installation_id": installation.id,
            "plugin_version": installation.plugin_version,
            "artifact_checksum": installation.artifact.local_checksum,
        });
        if let Some(recovery) = input.run_context.get("native_inference_recovery") {
            if recovery.get("binding") != Some(&native_inference_binding) {
                return Err(anyhow!("native_recovery_provider_configuration_mismatch"));
            }
        }

        let canonical_tool_registry = input.tools.clone();
        let actual_provider_code = input.provider_code.clone();
        let tool_structure = provider_tool_structure_receipt(&input.tools);
        tracing::debug!(
            flow_run_id = ?self.flow_run_id,
            provider_instance_id = %instance.id,
            provider_code = %input.provider_code,
            protocol = %input.protocol,
            model = %input.model,
            tool_count = input.tools.len(),
            tool_structure = %tool_structure,
            "AI Gateway provider tool structure"
        );
        let configured_model = instance
            .configured_models
            .iter()
            .find(|model| model.model_id == runtime.model);
        let effective_context_window = input
            .model_parameters
            .get("requested_context_window")
            .and_then(Value::as_u64)
            .or_else(|| configured_model.and_then(|model| model.context_window_override_tokens))
            .or_else(|| {
                package
                    .predefined_models
                    .iter()
                    .find(|model| model.model_id == runtime.model)
                    .and_then(|model| model.context_window)
            });
        let provider_invoke_started_at = OffsetDateTime::now_utc();
        let provider_invoke_started = std::time::Instant::now();
        let first_token_timing = Arc::new(Mutex::new(None::<FirstTokenTiming>));
        // Freeze diagnostics before this invocation starts; clients cannot enable it.
        let capture_stream_timing = std::env::var("FLOWBASE_PROVIDER_STREAM_TIMING_CAPTURE")
            .is_ok_and(|value| value == "1");
        let provider_stream_timing =
            Arc::new(Mutex::new(ProviderStreamTiming::new(capture_stream_timing)));
        let native_output_items = Arc::new(Mutex::new(std::collections::BTreeMap::<
            usize,
            Option<Value>,
        >::new()));
        let mut required_forward_handle = None;
        let mut diagnostic_forward_handle = None;
        let active_node = self
            .flow_execution_context
            .as_ref()
            .and_then(|context| context.active_node.lock().ok()?.clone());
        let active_node = active_node.or_else(|| {
            Some(RuntimeActiveNode {
                node_id: self.active_node_id.clone()?,
                node_run_id: self.active_node_run_id?,
            })
        });
        let billing_node_id = active_node.as_ref().map(|node| node.node_id.clone());
        let provider_generation_writer = match (&self.runtime_event_stream, self.flow_run_id) {
            (Some(stream), Some(run_id)) => stream.terminal_writer(run_id).await.ok(),
            _ => None,
        };
        // This host fact owns downstream protocol projection. Unlike the context
        // estimate, failure to commit it prevents the provider from producing output.
        if let (Some(node), Some(run_id)) = (active_node.as_ref(), self.flow_run_id) {
            let selection = crate::ports::LlmInputSelection {
                node_id: node.node_id.clone(),
                node_run_id: node.node_run_id,
                representation: if selected_native_transport {
                    crate::ports::LlmInputRepresentation::NativeOpaque
                } else {
                    crate::ports::LlmInputRepresentation::SemanticMapped
                },
            }
            .runtime_event();
            // Selection is a durable host fact even when a callback continues
            // after the waiting presentation stream has closed. Its commit is
            // required; delivery to that stream is a separate projection.
            runtime_event_persister::persist_runtime_event_payload_for_generation(
                &self.repository,
                run_id,
                &selection,
                provider_generation_writer.as_deref(),
            )
            .await?;
            if let Some(stream) = self.runtime_event_stream.as_ref() {
                let mut live_selection = selection;
                live_selection.persist_required = false;
                append_provider_runtime_event(stream, run_id, live_selection).await;
            }
        }
        if let (Some(active_node), Some(stream), Some(flow_run_id)) = (
            active_node.as_ref(),
            self.runtime_event_stream.as_ref(),
            self.flow_run_id,
        ) {
            match estimate_provider_count_tokens(&input) {
                Ok(estimate) => {
                    let mut context_snapshot = debug_stream_events::context_snapshot(
                        &active_node.node_id,
                        active_node.node_run_id,
                        &estimate,
                        effective_context_window,
                    );
                    match runtime_event_persister::persist_runtime_event_payload_for_generation(
                        &self.repository,
                        flow_run_id,
                        &context_snapshot,
                        provider_generation_writer.as_deref(),
                    )
                    .await
                    {
                        Ok(()) => {
                            context_snapshot.persist_required = false;
                            context_snapshot.durability = RuntimeEventDurability::Ephemeral;
                        }
                        Err(error) => {
                            tracing::warn!(
                                flow_run_id = %flow_run_id,
                                node_id = %active_node.node_id,
                                error = %error,
                                "failed to persist AI Gateway context snapshot"
                            );
                        }
                    }
                    append_provider_runtime_event(stream, flow_run_id, context_snapshot).await;
                }
                Err(error) => {
                    tracing::warn!(
                        flow_run_id = %flow_run_id,
                        node_id = %active_node.node_id,
                        error = %error,
                        "AI Gateway context estimate unavailable"
                    );
                }
            }
        }
        let presentation_source_node_id = input.trace_context.get("node_id").cloned();
        let native_capture = native_trajectory::start(
            self.repository.clone(),
            self.flow_run_id,
            active_node
                .as_ref()
                .map(|node| (node.node_id.clone(), node.node_run_id)),
            &input,
            self.observation_context.as_ref(),
        )
        .await;
        let native_observer = native_capture.observer();
        let (protocol_observation, protocol_completion) =
            if std::env::var("FLOWBASE_PROVIDER_PROTOCOL_CAPTURE").as_deref() == Ok("1") {
                protocol_observation::start(
                    self.repository.clone(),
                    self.flow_run_id,
                    protocol_observation::Capture::new(
                        active_node
                            .as_ref()
                            .map(|node| (node.node_id.clone(), node.node_run_id)),
                        &input,
                    ),
                )
            } else {
                (None, protocol_observation::Completion::default())
            };
        let live_provider_events = if let Some(RuntimeActiveNode {
            node_id,
            node_run_id,
        }) = active_node
        {
            let live_sender = self.live_provider_events.clone();
            let runtime_event_stream = self.runtime_event_stream.clone();
            let flow_run_id = self.flow_run_id;
            let first_token_timing_for_task = first_token_timing.clone();
            let provider_stream_timing_for_task = provider_stream_timing.clone();
            let canonical_tool_registry_for_task = canonical_tool_registry.clone();
            let answer_presentation = answer_presentation_source_is_active(
                presentation_source_node_id.as_deref(),
                &node_id,
            )
            .then(|| self.answer_presentation.clone())
            .flatten();
            let (required_sender, mut required_receiver) =
                mpsc::channel::<ProviderStreamEvent>(PROVIDER_LIVE_EVENT_LANE_CAPACITY);
            let (diagnostic_sender, mut diagnostic_receiver) =
                mpsc::channel::<ProviderStreamEvent>(PROVIDER_LIVE_EVENT_LANE_CAPACITY);
            let diagnostic_node_id = node_id.clone();
            let flow_execution_context_for_task = self.flow_execution_context.clone();
            let repository_for_events = self.repository.clone();
            let generation_writer_for_task = provider_generation_writer.clone();
            let native_output_items_for_task = native_output_items.clone();
            let capture_native_history = selected_native_transport;
            let response_round_id_for_task = self.response_round_id.or(self.flow_run_id);
            required_forward_handle = Some(tokio::spawn(async move {
                let mut canonical_writer = RuntimeCanonicalStreamWriter::new(node_id.clone());
                let mut ingress_sequence = 0_u64;
                while let Some(mut event) = required_receiver.recv().await {
                    ingress_sequence += 1;
                    let ingress_ms = provider_invoke_started.elapsed().as_millis() as u64;
                    let event_kind = provider_stream_event_kind(&event);
                    let size_bytes = serialized_frame_size(&event);
                    orchestration_runtime::execution_engine::canonicalize_provider_stream_event_tool_call_name(
                            &mut event,
                            &canonical_tool_registry_for_task,
                        );
                    native_observer.observe(&event);
                    record_first_token_timing(
                        &first_token_timing_for_task,
                        &event,
                        provider_invoke_started_at,
                        provider_invoke_started,
                    );
                    let canonical_deltas = canonical_writer.write(&event)?;
                    if capture_native_history {
                        if let ProviderStreamEvent::OutputItem {
                            phase,
                            output_index,
                            item,
                        } = &event
                        {
                            let mut items = native_output_items_for_task
                                .lock()
                                .map_err(|_| anyhow!("native output history lock poisoned"))?;
                            // The canonical writer above owns phase/identity validation.
                            // Keep open slots so Added without Done cannot look like empty prewarm.
                            items.insert(
                                *output_index,
                                match phase {
                                    ProviderOutputItemPhase::Added => None,
                                    ProviderOutputItemPhase::Done => Some(item.clone()),
                                },
                            );
                        }
                    }
                    let tool_delivery =
                        committed_tool_delivery_candidate(&node_id, node_run_id, &event)?;
                    if let Some(delivery) = &tool_delivery {
                        let context =
                            flow_execution_context_for_task.as_ref().ok_or_else(|| {
                                anyhow!("tool delivery requires a flow execution context")
                            })?;
                        context
                            .tool_delivery_events
                            .lock()
                            .map_err(|_| anyhow!("tool delivery buffer lock is poisoned"))?
                            .push(delivery.clone());
                    }
                    if let (
                        true,
                        None,
                        Some(flow_run_id),
                        ProviderStreamEvent::OutputItem {
                            phase: ProviderOutputItemPhase::Done,
                            output_index,
                            item,
                        },
                    ) = (
                        capture_native_history,
                        tool_delivery.as_ref(),
                        flow_run_id,
                        &event,
                    ) {
                        let mut fact = debug_stream_events::provider_output_item_done(
                            &node_id,
                            node_run_id,
                            *output_index,
                            item.clone(),
                        );
                        fact.payload["response_round_id"] = json!(response_round_id_for_task);
                        runtime_event_persister::persist_runtime_event_payload_for_generation(
                            &repository_for_events,
                            flow_run_id,
                            &fact,
                            generation_writer_for_task.as_deref(),
                        )
                        .await?;
                    }
                    project_canonical_provider_deltas(
                        runtime_event_stream.as_ref(),
                        flow_run_id,
                        answer_presentation.as_ref(),
                        &node_id,
                        node_run_id,
                        &canonical_deltas,
                    )
                    .await;
                    if let (Some(stream), Some(flow_run_id)) = (&runtime_event_stream, flow_run_id)
                    {
                        let runtime_events = match &event {
                            ProviderStreamEvent::TextDelta { .. }
                            | ProviderStreamEvent::ReasoningDelta { .. }
                            | ProviderStreamEvent::Finish { .. }
                            | ProviderStreamEvent::Error { .. } => Vec::new(),
                            ProviderStreamEvent::ReasoningSignatureDelta { signature } => {
                                vec![debug_stream_events::reasoning_signature_delta(
                                    &node_id,
                                    node_run_id,
                                    signature.clone(),
                                )]
                            }
                            ProviderStreamEvent::ResponsesOutputDelta { event } => {
                                vec![debug_stream_events::provider_responses_output_delta(
                                    &node_id,
                                    node_run_id,
                                    event.clone(),
                                )]
                            }
                            ProviderStreamEvent::OutputItem {
                                phase,
                                output_index,
                                item,
                            } if tool_delivery.is_none() => vec![match phase {
                                ProviderOutputItemPhase::Added => {
                                    debug_stream_events::provider_output_item_added(
                                        &node_id,
                                        node_run_id,
                                        *output_index,
                                        item.clone(),
                                    )
                                }
                                ProviderOutputItemPhase::Done => {
                                    debug_stream_events::provider_output_item_done(
                                        &node_id,
                                        node_run_id,
                                        *output_index,
                                        item.clone(),
                                    )
                                }
                            }],
                            ProviderStreamEvent::OutputItem { .. } => Vec::new(),
                            ProviderStreamEvent::UsageSnapshot { usage } => {
                                vec![debug_stream_events::usage_snapshot(
                                    &node_id,
                                    node_run_id,
                                    usage,
                                )]
                            }
                            _ => Vec::new(),
                        };
                        for runtime_event in runtime_events {
                            let event_type = runtime_event.event_type.clone();
                            let source = runtime_event.source;
                            let mut stream_event = runtime_event;
                            if capture_native_history
                                && stream_event.event_type == "provider_output_item_done"
                            {
                                stream_event.payload["response_round_id"] =
                                    json!(response_round_id_for_task);
                            }
                            if debug_stream_events::is_answer_presentation_delta_payload(
                                &stream_event.payload,
                            ) {
                                stream_event.persist_required = false;
                            }
                            let durable_log_fact =
                                stream_event.event_type == "provider_output_item_done";
                            if durable_log_fact {
                                // Non-tool native items were committed above; executable
                                // items are committed with the callback waiting state.
                                stream_event.persist_required = false;
                            }
                            match runtime_events::forward_required_runtime_event(
                                &repository_for_events,
                                stream.as_ref(),
                                flow_run_id,
                                stream_event,
                                generation_writer_for_task.as_deref(),
                            )
                            .await
                            {
                                Ok(_) => {}
                                Err(error) => {
                                    if durable_log_fact {
                                        return Err(error);
                                    }
                                    if is_expected_runtime_event_stream_closed_error(&error) {
                                        tracing::debug!(
                                            flow_run_id = %flow_run_id,
                                            event_type = %event_type,
                                            source = ?source,
                                            error = %error,
                                            "provider runtime event append skipped because stream is already closed"
                                        );
                                    } else {
                                        tracing::warn!(
                                            flow_run_id = %flow_run_id,
                                            event_type = %event_type,
                                            source = ?source,
                                            error = %error,
                                            "failed to append provider runtime event"
                                        );
                                    }
                                }
                            }
                        }
                    }
                    provider_stream_timing_for_task
                        .lock()
                        .map_err(|_| anyhow!("provider stream timing lock is poisoned"))?
                        .observe(
                            ingress_sequence,
                            event_kind,
                            size_bytes,
                            ingress_ms,
                            provider_invoke_started.elapsed().as_millis() as u64,
                        )?;
                    if tool_delivery.is_none() {
                        if let Some(sender) = &live_sender {
                            sender
                                .send(LiveProviderStreamEvent {
                                    node_id: node_id.clone(),
                                    node_run_id,
                                    event: event.clone(),
                                })
                                .await
                                .map_err(|_| {
                                    anyhow!("required live provider event writer closed")
                                })?;
                        }
                    }
                }
                let completion_deltas = canonical_writer.complete()?;
                project_canonical_provider_deltas(
                    runtime_event_stream.as_ref(),
                    flow_run_id,
                    answer_presentation.as_ref(),
                    &node_id,
                    node_run_id,
                    &completion_deltas,
                )
                .await;
                Ok::<_, anyhow::Error>(canonical_writer.into_state())
            }));
            let diagnostic_stream = self.runtime_event_stream.clone();
            let diagnostic_flow_run_id = self.flow_run_id;
            diagnostic_forward_handle = Some(tokio::spawn(async move {
                while let Some(event) = diagnostic_receiver.recv().await {
                    let ProviderStreamEvent::NativeEvent { protocol, event } = event else {
                        continue;
                    };
                    if let (Some(stream), Some(flow_run_id)) =
                        (&diagnostic_stream, diagnostic_flow_run_id)
                    {
                        let _ = stream
                            .append(
                                flow_run_id,
                                debug_stream_events::provider_native_event(
                                    &diagnostic_node_id,
                                    node_run_id,
                                    protocol,
                                    event,
                                ),
                            )
                            .await;
                    }
                }
            }));
            Some(crate::ports::ProviderLiveEventSenders {
                protocol_observation,
                required: required_sender,
                diagnostic: diagnostic_sender,
            })
        } else {
            None
        };

        let fee_lifecycle = fee_lifecycle::ProviderFeeLifecycle::new(self);
        let billing = fee_lifecycle
            .dispatch(fee_lifecycle::ProviderFeeEvent::BeforeInvocation {
                input: &input,
                pricing_provider_code: configured_model
                    .map(|model| model.pricing_provider_code.as_str())
                    .unwrap_or(domain::DEFAULT_MODEL_PRICING_PROVIDER_CODE),
                pricing_model_id: configured_model
                    .map(|model| model.pricing_model_id.as_str())
                    .unwrap_or(domain::DEFAULT_MODEL_PRICING_MODEL_ID),
                billing_node_id: billing_node_id.as_deref(),
            })
            .await?;

        let native_responses_passthrough = input.required_capabilities.contains(
            &plugin_framework::provider_contract::ProviderInvocationCapability::ResponsesNativePassthrough,
        );
        let deadline_unix_ms = provider_execution_deadline_unix_ms(
            &input,
            OffsetDateTime::now_utc(),
            self.transport_connection_scope_override
                .as_ref()
                .and_then(|scope| scope.as_deref()),
        );
        let flow_ms = bounded_timing_millis(provider_invoke_started.elapsed());
        if let Some(scope) = &self.transport_connection_scope_override {
            apply_transport_connection_scope_override(&mut input, scope.as_deref());
        }
        let sealed_session_identity = input
            .client_protocol_envelope
            .as_ref()
            .filter(|envelope| envelope.source_protocol == "openai_responses")
            .and_then(|envelope| envelope.headers.get("session-id"))
            .and_then(|values| match values.as_slice() {
                [identity]
                    if identity.len() == 64
                        && identity.bytes().all(|byte| byte.is_ascii_hexdigit()) =>
                {
                    Some(identity.clone())
                }
                _ => None,
            });
        let invocation_result = self
            .runtime
            .invoke_stream_with_execution_context(
                &installation,
                input,
                live_provider_events,
                crate::ports::ProviderRuntimeExecutionContext {
                    workspace_id: self.workspace_id,
                    actor_id: self
                        .flow_execution_context
                        .as_ref()
                        .map(|context| context.data_model.actor.user_id),
                    deadline_unix_ms,
                },
                domain::NetworkEgressConsumerSelector::ModelProviderInstance {
                    instance_id: instance.id,
                },
            )
            .await;
        tracing::debug!(
            provider_invoke_ms = provider_invoke_started.elapsed().as_millis() as u64,
            "provider invoke finished"
        );
        let (canonical_stream_state, forwarding_error) =
            if let Some(handle) = required_forward_handle {
                match handle.await {
                    Ok(Ok(state)) => (Some(state), None),
                    Ok(Err(error)) => (None, Some(error)),
                    Err(error) => (
                        None,
                        Some(anyhow!(
                            "provider live event forwarding task panicked: {error}"
                        )),
                    ),
                }
            } else {
                (None, None)
            };
        native_capture
            .finish(
                invocation_result.as_ref().ok().map(|output| &output.result),
                invocation_result
                    .as_ref()
                    .err()
                    .map(|_| "invocation failed"),
                forwarding_error.is_none(),
            )
            .await;
        protocol_completion.finish(invocation_result.is_ok() && forwarding_error.is_none());
        if let Some(handle) = diagnostic_forward_handle {
            if let Err(error) = handle.await {
                tracing::warn!(error = %error, "provider diagnostic event forwarding task panicked");
            }
        }
        let (mut invocation_output, mut invocation_error) = match invocation_result {
            Ok(output) => (Some(output), None),
            Err(error) => (None, Some(error)),
        };
        // Validate the provider-owned typed receipt before any host metadata wrapping.
        // Keep the validation result until settlement completes so invalid metadata does
        // not skip accounting for upstream usage that has already occurred.
        let recovery_receipt = match invocation_output.as_ref() {
            Some(output) => output.result.recovery_receipt().map_err(anyhow::Error::msg),
            None => Ok(None),
        };
        if let Some(error) = forwarding_error {
            invocation_error.get_or_insert(error);
        }
        if let (Some(transport), Some(flow_run_id), Some(output)) = (
            self.provider_transport_payload
                .as_ref()
                .filter(|_| selected_native_transport),
            self.flow_run_id,
            invocation_output.as_mut(),
        ) {
            let round_id = self.response_round_id.unwrap_or(flow_run_id);
            let history = {
                let items = native_output_items
                    .lock()
                    .map_err(|_| anyhow!("native output history lock poisoned"))?;
                // Every announced slot must close. Sparse provider indexes still
                // preserve output order in this BTreeMap.
                if invocation_error.is_none()
                    && matches!(output.result.finish_reason,
                        Some(plugin_framework::provider_contract::ProviderFinishReason::Stop
                        | plugin_framework::provider_contract::ProviderFinishReason::ToolCall
                        | plugin_framework::provider_contract::ProviderFinishReason::McpCall))
                {
                    if let Some(output_items) = completed_native_output_items(&items) {
                        crate::application_public_api::compat::openai::history::completed_history(
                            transport.wire_body(),
                            self.native_history.as_ref(),
                            &output_items,
                        )?
                    } else {
                        None
                    }
                } else {
                    None
                }
            };
            let metadata = &mut output.result.provider_metadata;
            if !metadata.is_object() {
                *metadata = json!({});
            }
            metadata["native_response"] = json!({
                "response_id": format!("resp_{round_id}"),
                "configuration_digest": transport.configuration_digest()?,
                "history": history,
                "binding": native_inference_binding,
                "user_messages_digest": transport.user_messages_digest()?.or_else(|| self.native_user_messages_digest.clone()),
            });
        }
        let continuation_history = invocation_output
            .as_ref()
            .and_then(|output| {
                output
                    .result
                    .provider_metadata
                    .pointer("/native_response/history")
            })
            .filter(|value| value.is_object())
            .cloned();
        if selected_native_transport {
            if let Some(error) = invocation_error.take() {
                let configuration_digest = self
                    .provider_transport_payload
                    .as_ref()
                    .map(|transport| transport.configuration_digest())
                    .transpose()?;
                invocation_error = Some(seal_native_failure_binding(
                    error,
                    &native_inference_binding,
                    configuration_digest.as_deref(),
                ));
            }
        }
        let billing_result = fee_lifecycle
            .dispatch(fee_lifecycle::ProviderFeeEvent::AfterUsage {
                reservation: Box::new(billing),
                outcome: Box::new(fee_lifecycle::ProviderFeeOutcome {
                    upstream_model_id: &runtime.model,
                    provider_instance_id: instance.id,
                    actual_provider_code: &actual_provider_code,
                    invocation_output: &mut invocation_output,
                    invocation_error: &mut invocation_error,
                    canonical_stream_state: canonical_stream_state.as_ref(),
                    native_responses_passthrough,
                }),
            })
            .await;
        // Preserve provider continuation even on late billing failure, but only successful
        // host completion may carry a complete history proof into the next response round.
        if let Some(output) = invocation_output
            .as_ref()
            .filter(|_| selected_native_transport)
        {
            let history = if billing_result.is_ok() && invocation_error.is_none() {
                continuation_history
            } else {
                None
            };
            if let Err(error) = self
                .stage_provider_continuation_with_history(
                    runtime,
                    output.result.response_id.as_deref(),
                    history,
                    sealed_session_identity.as_deref(),
                )
                .await
            {
                invocation_error.get_or_insert(error);
            }
        }
        billing_result?;
        if let Some(error) = invocation_error {
            return Err(error);
        }
        let mut invocation_output = invocation_output
            .ok_or_else(|| anyhow!("provider invocation completed without output or error"))?;
        let runtime_stream_timing = std::mem::replace(
            &mut *provider_stream_timing
                .lock()
                .map_err(|_| anyhow!("provider stream timing lock is poisoned"))?,
            ProviderStreamTiming::new(false),
        );
        attach_gateway_stage_timing(
            &mut invocation_output.result.provider_metadata,
            flow_ms,
            runtime_stream_timing.first_ingress_ms(),
            runtime_stream_timing.max_append_delay_ms(),
        )?;
        // Restore the validated receipt at the host-owned metadata surface after wrapping.
        // AI Native must not parse nested Provider Close/cursor payloads.
        let recovery_receipt = recovery_receipt?;
        if let Some(account) = self
            .flow_execution_context
            .as_ref()
            .and_then(|context| context.user_account.as_ref())
        {
            let upstream = std::mem::take(&mut invocation_output.result.provider_metadata);
            invocation_output.result.provider_metadata = json!({
                "_1flowbase_user_account": account,
                "_1flowbase_upstream_provider_metadata": upstream,
            });
        }
        let timing_summary = runtime_stream_timing.summary();
        let timing_details = runtime_stream_timing.into_details();
        let provider_metadata = std::mem::take(&mut invocation_output.result.provider_metadata);
        invocation_output.result.provider_metadata = json!({
            "_1flowbase_provider_observability_schema_version": 1,
            "_1flowbase_runtime_stream_timing_summary": timing_summary,
            "_1flowbase_upstream_provider_metadata": provider_metadata,
        });
        if let Some(details) = timing_details {
            invocation_output.result.provider_metadata["_1flowbase_runtime_stream_timing"] =
                details;
        }
        if let Some(receipt) = recovery_receipt {
            invocation_output
                .result
                .set_recovery_receipt(receipt)
                .map_err(anyhow::Error::msg)?;
        }
        let captured_first_token_timing = first_token_timing.lock().ok().and_then(|timing| *timing);
        let mut output = orchestration_runtime::execution_engine::ProviderInvocationOutput {
            events: invocation_output.events,
            result: invocation_output.result,
            first_token_at: captured_first_token_timing.map(|timing| timing.first_token_at),
            time_to_first_token_ms: captured_first_token_timing
                .map(|timing| timing.time_to_first_token_ms),
        };
        orchestration_runtime::execution_engine::canonicalize_provider_output_tool_call_names(
            &mut output,
            &canonical_tool_registry,
        );
        Ok(output)
    }
}

