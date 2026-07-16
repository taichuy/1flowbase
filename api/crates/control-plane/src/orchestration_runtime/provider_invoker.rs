use super::*;
use plugin_framework::provider_contract::ProviderMessageRole;

const VISIBLE_INTERNAL_LLM_MEDIA_TOOLS_CONTEXT_KEY: &str = "visible_internal_llm_media_tools";

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
    async fn invoke_llm(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        mut input: ProviderInvocationInput,
    ) -> Result<orchestration_runtime::execution_engine::ProviderInvocationOutput> {
        let provider_resolve_started = std::time::Instant::now();
        let instance = self.resolve_llm_instance(runtime).await?;
        tracing::debug!(
            provider_resolve_ms = provider_resolve_started.elapsed().as_millis() as u64,
            "provider resolve finished"
        );

        let installation_reconcile_started = std::time::Instant::now();
        let installation = self.ready_installation(instance.installation_id).await?;
        tracing::debug!(
            installation_reconcile_ms = installation_reconcile_started.elapsed().as_millis() as u64,
            "installation reconcile finished"
        );
        let assigned = self
            .repository
            .list_assignments(self.workspace_id)
            .await?
            .into_iter()
            .any(|assignment| assignment.installation_id == installation.id);
        if !assigned
            || matches!(
                installation.desired_state,
                domain::PluginDesiredState::Disabled
            )
        {
            return Err(ControlPlaneError::InvalidInput("provider_code").into());
        }
        if installation.availability_status != domain::PluginAvailabilityStatus::Available {
            return Err(ControlPlaneError::Conflict("plugin_installation_unavailable").into());
        }

        let package_load_started = std::time::Instant::now();
        let package = load_provider_package(&installation.installed_path)?;
        tracing::debug!(
            package_load_ms = package_load_started.elapsed().as_millis() as u64,
            "package load finished"
        );
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
            &self.provider_secret_master_key,
            &package,
            &instance,
        )
        .await?;
        tracing::debug!(
            runtime_config_ms = runtime_config_started.elapsed().as_millis() as u64,
            "runtime config finished"
        );

        let canonical_tool_registry = input.tools.clone();
        let provider_invoke_started_at = OffsetDateTime::now_utc();
        let provider_invoke_started = std::time::Instant::now();
        let first_token_timing = Arc::new(Mutex::new(None::<FirstTokenTiming>));
        let mut live_forward_handle = None;
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
        let live_provider_events = if let Some(RuntimeActiveNode {
            node_id,
            node_run_id,
        }) = active_node
        {
            let live_sender = self.live_provider_events.clone();
            let runtime_event_stream = self.runtime_event_stream.clone();
            let answer_presentation = self.answer_presentation.clone();
            let repository = self.repository.clone();
            let flow_run_id = self.flow_run_id;
            let first_token_timing_for_task = first_token_timing.clone();
            let canonical_tool_registry_for_task = canonical_tool_registry.clone();
            let (provider_sender, mut provider_receiver) =
                mpsc::unbounded_channel::<ProviderStreamEvent>();
            live_forward_handle = Some(tokio::spawn(async move {
                let mut think_tag_splitter = ThinkTagStreamSplitter::default();
                while let Some(mut event) = provider_receiver.recv().await {
                    orchestration_runtime::execution_engine::canonicalize_provider_stream_event_tool_call_name(
                            &mut event,
                            &canonical_tool_registry_for_task,
                        );
                    record_first_token_timing(
                        &first_token_timing_for_task,
                        &event,
                        provider_invoke_started_at,
                        provider_invoke_started,
                    );
                    if let Some(sender) = &live_sender {
                        let _ = sender.send(LiveProviderStreamEvent {
                            node_id: node_id.clone(),
                            node_run_id,
                            event: event.clone(),
                        });
                    }
                    if let (Some(stream), Some(flow_run_id)) = (&runtime_event_stream, flow_run_id)
                    {
                        let runtime_events = match &event {
                            ProviderStreamEvent::TextDelta { delta } => {
                                let mut runtime_events = Vec::new();
                                let parts = think_tag_splitter.split(delta);
                                for part in parts {
                                    let provider_event = match part.kind {
                                        DebugDeltaKind::Text => {
                                            runtime_events.push(debug_stream_events::text_delta(
                                                &node_id,
                                                node_run_id,
                                                part.text.clone(),
                                            ));
                                            ProviderStreamEvent::TextDelta { delta: part.text }
                                        }
                                        DebugDeltaKind::Reasoning => {
                                            runtime_events.push(
                                                debug_stream_events::reasoning_delta(
                                                    &node_id,
                                                    node_run_id,
                                                    part.text.clone(),
                                                ),
                                            );
                                            ProviderStreamEvent::ReasoningDelta { delta: part.text }
                                        }
                                    };
                                    if let Some(answer_presentation) = &answer_presentation {
                                        runtime_events.extend(
                                            answer_presentation.lock().await.push_provider_event(
                                                &node_id,
                                                node_run_id,
                                                &provider_event,
                                            ),
                                        );
                                    }
                                }
                                runtime_events
                            }
                            ProviderStreamEvent::ReasoningDelta { delta } => {
                                let mut runtime_events =
                                    vec![debug_stream_events::reasoning_delta(
                                        &node_id,
                                        node_run_id,
                                        delta.clone(),
                                    )];
                                if let Some(answer_presentation) = &answer_presentation {
                                    runtime_events.extend(
                                        answer_presentation.lock().await.push_provider_event(
                                            &node_id,
                                            node_run_id,
                                            &event,
                                        ),
                                    );
                                }
                                runtime_events
                            }
                            _ => Vec::new(),
                        };
                        if runtime_events.is_empty() {
                            continue;
                        };
                        for runtime_event in runtime_events {
                            let is_answer_presentation =
                                debug_stream_events::is_answer_presentation_delta_payload(
                                    &runtime_event.payload,
                                );
                            if is_answer_presentation {
                                if let Err(error) =
                                    runtime_event_persister::persist_runtime_event_payload(
                                        &repository,
                                        flow_run_id,
                                        &runtime_event,
                                    )
                                    .await
                                {
                                    tracing::warn!(
                                        flow_run_id = %flow_run_id,
                                        event_type = %runtime_event.event_type,
                                        error = %error,
                                        "failed to persist answer presentation runtime event"
                                    );
                                }
                            }
                            let event_type = runtime_event.event_type.clone();
                            let source = runtime_event.source;
                            let mut stream_event = runtime_event;
                            if is_answer_presentation {
                                stream_event.persist_required = false;
                            }
                            if let Err(error) = stream.append(flow_run_id, stream_event).await {
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
            }));
            Some(provider_sender)
        } else {
            None
        };

        let invocation_result = self
            .runtime
            .invoke_stream_with_live_events(&installation, input, live_provider_events)
            .await;
        tracing::debug!(
            provider_invoke_ms = provider_invoke_started.elapsed().as_millis() as u64,
            "provider invoke finished"
        );
        if let Some(handle) = live_forward_handle {
            if let Err(error) = handle.await {
                tracing::warn!(
                    error = %error,
                    "provider live event forwarding task panicked"
                );
            }
        }
        let invocation_output = invocation_result?;
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

fn record_first_token_timing(
    first_token_timing: &Arc<Mutex<Option<FirstTokenTiming>>>,
    event: &ProviderStreamEvent,
    provider_invoke_started_at: OffsetDateTime,
    provider_invoke_started: std::time::Instant,
) {
    if !matches!(
        event,
        ProviderStreamEvent::TextDelta { .. } | ProviderStreamEvent::ReasoningDelta { .. }
    ) {
        return;
    }

    let Ok(mut timing) = first_token_timing.lock() else {
        return;
    };
    if timing.is_some() {
        return;
    }
    let elapsed = provider_invoke_started.elapsed();
    *timing = Some(FirstTokenTiming {
        first_token_at: provider_invoke_started_at + elapsed,
        time_to_first_token_ms: elapsed.as_millis() as u64,
    });
}

pub(super) fn is_expected_runtime_event_stream_closed_error(error: &anyhow::Error) -> bool {
    let message = error.to_string();
    message.contains("runtime event stream is closed")
        || message.contains("runtime event stream is not open")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DebugDeltaKind {
    Text,
    Reasoning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DebugDeltaPart {
    pub(super) kind: DebugDeltaKind,
    pub(super) text: String,
}

#[derive(Debug, Default)]
pub(super) struct ThinkTagStreamSplitter {
    inside_think: bool,
    pending: String,
}

impl ThinkTagStreamSplitter {
    pub(super) fn split(&mut self, delta: &str) -> Vec<DebugDeltaPart> {
        self.pending.push_str(delta);
        let mut parts = Vec::new();

        loop {
            let tag = if self.inside_think {
                "</think>"
            } else {
                "<think>"
            };

            if let Some(tag_index) = self.pending.find(tag) {
                let text = self.pending[..tag_index].to_string();
                push_debug_delta_part(
                    &mut parts,
                    if self.inside_think {
                        DebugDeltaKind::Reasoning
                    } else {
                        DebugDeltaKind::Text
                    },
                    text,
                );
                self.pending.drain(..tag_index + tag.len());
                self.inside_think = !self.inside_think;
                continue;
            }

            let keep_len = partial_tag_prefix_len(&self.pending, tag);
            let emit_len = self.pending.len().saturating_sub(keep_len);
            if emit_len > 0 {
                let text = self.pending[..emit_len].to_string();
                self.pending.drain(..emit_len);
                push_debug_delta_part(
                    &mut parts,
                    if self.inside_think {
                        DebugDeltaKind::Reasoning
                    } else {
                        DebugDeltaKind::Text
                    },
                    text,
                );
            }
            break;
        }

        parts
    }

    pub(super) fn finish(&mut self) -> Vec<DebugDeltaPart> {
        let text = std::mem::take(&mut self.pending);
        let mut parts = Vec::new();
        push_debug_delta_part(
            &mut parts,
            if self.inside_think {
                DebugDeltaKind::Reasoning
            } else {
                DebugDeltaKind::Text
            },
            text,
        );
        parts
    }
}

fn push_debug_delta_part(parts: &mut Vec<DebugDeltaPart>, kind: DebugDeltaKind, text: String) {
    if text.is_empty() {
        return;
    }

    if let Some(previous) = parts.last_mut().filter(|part| part.kind == kind) {
        previous.text.push_str(&text);
        return;
    }

    parts.push(DebugDeltaPart { kind, text });
}

fn partial_tag_prefix_len(buffer: &str, tag: &str) -> usize {
    let max_len = buffer.len().min(tag.len().saturating_sub(1));
    (1..=max_len)
        .rev()
        .find(|length| {
            let start = buffer.len() - length;
            buffer.is_char_boundary(start) && tag.starts_with(&buffer[start..])
        })
        .unwrap_or(0)
}

impl<R, H> RuntimeProviderInvoker<R, H>
where
    R: PluginRepository + Clone + Send + Sync,
    H: ProviderRuntimePort + Clone + Send + Sync,
{
    async fn ready_installation(
        &self,
        installation_id: Uuid,
    ) -> Result<domain::PluginInstallationRecord> {
        match (
            self.api_node_id.as_deref(),
            self.provider_install_root.as_deref(),
        ) {
            (Some(node_id), Some(install_root)) => {
                ready_current_node_plugin_installation(
                    &self.repository,
                    node_id,
                    install_root,
                    installation_id,
                )
                .await
            }
            _ => reconcile_installation_snapshot(&self.repository, installation_id).await,
        }
    }
}

impl<R, H> RuntimeProviderInvoker<R, H>
where
    R: ModelProviderRepository + PluginRepository + Clone + Send + Sync,
    H: ProviderRuntimePort + Clone + Send + Sync,
{
    pub(super) fn for_flow_run(&self, flow_run_id: Uuid) -> Self {
        Self {
            repository: self.repository.clone(),
            runtime: self.runtime.clone(),
            workspace_id: self.workspace_id,
            provider_secret_master_key: self.provider_secret_master_key.clone(),
            live_provider_events: self.live_provider_events.clone(),
            runtime_event_stream: self.runtime_event_stream.clone(),
            flow_run_id: Some(flow_run_id),
            active_node_id: self.active_node_id.clone(),
            active_node_run_id: self.active_node_run_id,
            api_node_id: self.api_node_id.clone(),
            provider_install_root: self.provider_install_root.clone(),
            flow_execution_context: self.flow_execution_context.clone(),
            answer_presentation: self.answer_presentation.clone(),
        }
    }

    pub(super) fn with_flow_execution_context(
        &self,
        context: Arc<RuntimeFlowExecutionContext>,
    ) -> Self {
        let mut invoker = self.clone();
        invoker.flow_execution_context = Some(context);
        invoker
    }

    pub(super) fn with_answer_presentation(
        &self,
        answer_presentation: Arc<tokio::sync::Mutex<answer_presentation::AnswerPresentationCursor>>,
    ) -> Self {
        let mut invoker = self.clone();
        invoker.answer_presentation = Some(answer_presentation);
        invoker
    }

    pub(super) async fn resolve_llm_instance(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
    ) -> Result<domain::ModelProviderInstanceRecord> {
        let provider_instance_id = Uuid::parse_str(&runtime.provider_instance_id)
            .map_err(|_| ControlPlaneError::InvalidInput("source_instance_id"))?;
        let instance = self
            .repository
            .get_instance(self.workspace_id, provider_instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_provider_instance"))?;
        if instance.provider_code != runtime.provider_code {
            return Err(ControlPlaneError::InvalidInput("provider_code").into());
        }
        if instance.status != domain::ModelProviderInstanceStatus::Ready {
            return Err(ControlPlaneError::Conflict("provider_instance_not_ready").into());
        }
        if !instance.included_in_main {
            return Err(ControlPlaneError::Conflict("provider_instance_not_in_main").into());
        }
        let installation = self
            .repository
            .get_installation(instance.installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        let assigned = self
            .repository
            .list_assignments(self.workspace_id)
            .await?
            .into_iter()
            .any(|assignment| assignment.installation_id == installation.id);
        if !assigned {
            return Err(ControlPlaneError::Conflict("plugin_assignment_required").into());
        }
        if matches!(
            installation.desired_state,
            domain::PluginDesiredState::Disabled
        ) || installation.availability_status != domain::PluginAvailabilityStatus::Available
        {
            return Err(ControlPlaneError::Conflict("plugin_installation_unavailable").into());
        }
        if !instance.enabled_model_ids.is_empty()
            && !instance
                .enabled_model_ids
                .iter()
                .any(|model_id| model_id == &runtime.model)
        {
            return Err(ControlPlaneError::InvalidInput("model").into());
        }

        Ok(instance)
    }
}

#[async_trait]
impl<R, H> orchestration_runtime::execution_engine::CapabilityInvoker
    for RuntimeProviderInvoker<R, H>
where
    R: ModelDefinitionRepository
        + OrchestrationRuntimeRepository
        + PluginRepository
        + Clone
        + Send
        + Sync,
    H: ProviderRuntimePort + CapabilityPluginRuntimePort + Clone + Send + Sync,
{
    async fn invoke_capability_node(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledPluginRuntime,
        config_payload: Value,
        input_payload: Value,
    ) -> Result<orchestration_runtime::execution_engine::CapabilityInvocationOutput> {
        let installation = self.ready_installation(runtime.installation_id).await?;
        let assigned = self
            .repository
            .list_assignments(self.workspace_id)
            .await?
            .into_iter()
            .any(|assignment| assignment.installation_id == installation.id);
        if !assigned
            || matches!(
                installation.desired_state,
                domain::PluginDesiredState::Disabled
            )
        {
            return Err(ControlPlaneError::InvalidInput("installation_id").into());
        }
        if installation.availability_status != domain::PluginAvailabilityStatus::Available {
            return Err(ControlPlaneError::Conflict("plugin_installation_unavailable").into());
        }

        let output = self
            .runtime
            .execute_node(ExecuteCapabilityNodeInput {
                installation,
                contribution_code: runtime.contribution_code.clone(),
                config_payload,
                input_payload,
            })
            .await?;

        Ok(
            orchestration_runtime::execution_engine::CapabilityInvocationOutput {
                output_payload: output.output_payload,
            },
        )
    }

    async fn invoke_data_model_node(
        &self,
        node: &orchestration_runtime::compiled_plan::CompiledNode,
        resolved_inputs: &serde_json::Map<String, Value>,
    ) -> Result<orchestration_runtime::execution_engine::DataModelInvocationOutput> {
        let context = self
            .flow_execution_context
            .as_ref()
            .ok_or_else(|| anyhow!("data model flow execution context is not configured"))?;
        let active_node = context
            .active_node
            .lock()
            .map_err(|_| anyhow!("active runtime node lock is poisoned"))?
            .clone()
            .ok_or_else(|| anyhow!("data model active runtime node is not set"))?;
        if active_node.node_id != node.node_id {
            return Err(anyhow!(
                "data model active runtime node mismatch: expected {}, got {}",
                node.node_id,
                active_node.node_id
            ));
        }

        let execution = data_model_runtime::execute_data_model_node(
            self.repository.clone(),
            context.data_model.runtime_engine.clone(),
            &context.data_model.actor,
            node,
            resolved_inputs,
            &data_model_runtime::DataModelRunContext {
                workspace_id: self.workspace_id,
                application_id: context.data_model.application_id,
                draft_id: context.data_model.draft_id,
                flow_run_id: context.data_model.flow_run_id,
                node_run_id: active_node.node_run_id,
            },
        )
        .await;

        let (pending_callback, debug_payload) = match execution.waiting_confirmation {
            Some(confirmation) => {
                let request_payload = json!({
                    "kind": "data_model_side_effect_confirmation",
                    "actor_user_id": context.data_model.actor.user_id,
                    "node_id": node.node_id,
                    "run_id": context.data_model.flow_run_id,
                    "payload_hash": confirmation.payload_hash,
                    "idempotency_key": confirmation.idempotency_key,
                    "expires_at": confirmation.expires_at,
                    "request_payload": confirmation.request_payload,
                });
                (
                    Some(orchestration_runtime::execution_engine::DataModelCallback {
                        callback_kind: "data_model_side_effect_confirmation".to_string(),
                        request_payload,
                    }),
                    json!({
                        "side_effect_policy": "confirm_each_run",
                        "idempotency_key": confirmation.idempotency_key,
                        "payload_hash": confirmation.payload_hash,
                        "expires_at": confirmation.expires_at,
                    }),
                )
            }
            None => (None, json!({})),
        };

        Ok(
            orchestration_runtime::execution_engine::DataModelInvocationOutput {
                output_payload: execution.output_payload,
                error_payload: execution.error_payload,
                metrics_payload: execution.metrics_payload,
                debug_payload,
                pending_callback,
            },
        )
    }
}

#[async_trait]
impl<R, H> orchestration_runtime::execution_engine::CodeInvoker for RuntimeProviderInvoker<R, H>
where
    R: Clone + Send + Sync,
    H: Clone + Send + Sync,
{
    async fn invoke_code_node(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledCodeRuntime,
        config_payload: Value,
        input_payload: Value,
    ) -> Result<orchestration_runtime::execution_engine::CodeInvocationOutput> {
        orchestration_runtime::execution_engine::CodeInvoker::invoke_code_node(
            &orchestration_runtime::execution_engine::QuickJsCodeInvoker::default(),
            runtime,
            config_payload,
            input_payload,
        )
        .await
    }
}

async fn build_provider_runtime_config<R>(
    repository: &R,
    master_key: &str,
    package: &ProviderPackage,
    instance: &domain::ModelProviderInstanceRecord,
) -> Result<Value>
where
    R: ModelProviderRepository,
{
    let secret_json = repository
        .get_secret_json(instance.id, master_key)
        .await?
        .unwrap_or_else(empty_object);
    validate_required_fields(
        &package.provider.form_schema,
        &instance.config_json,
        &secret_json,
    )?;
    merge_json_object(&instance.config_json, &secret_json)
}

fn validate_required_fields(
    form_schema: &[ProviderConfigField],
    public_config: &Value,
    secret_config: &Value,
) -> Result<()> {
    let public_object = public_config
        .as_object()
        .ok_or(ControlPlaneError::InvalidInput("config_json"))?;
    let secret_object = secret_config
        .as_object()
        .ok_or(ControlPlaneError::InvalidInput("config_json"))?;
    for field in form_schema {
        if !field.required {
            continue;
        }
        let value = if is_secret_field(&field.field_type) {
            secret_object.get(&field.key)
        } else {
            public_object.get(&field.key)
        };
        if value.is_none()
            || value == Some(&Value::Null)
            || value == Some(&Value::String(String::new()))
        {
            return Err(ControlPlaneError::InvalidInput("config_json").into());
        }
    }
    Ok(())
}

fn merge_json_object(base: &Value, patch: &Value) -> Result<Value> {
    let mut merged = base
        .as_object()
        .cloned()
        .ok_or(ControlPlaneError::InvalidInput("config_json"))?;
    let patch_object = patch
        .as_object()
        .ok_or(ControlPlaneError::InvalidInput("config_json"))?;
    for (key, value) in patch_object {
        merged.insert(key.clone(), value.clone());
    }
    Ok(Value::Object(merged))
}

fn empty_object() -> Value {
    Value::Object(serde_json::Map::new())
}

fn is_secret_field(field_type: &str) -> bool {
    field_type.trim().eq_ignore_ascii_case("secret")
}

fn load_provider_package(path: &str) -> Result<ProviderPackage> {
    ProviderPackage::load_from_dir(path)
        .map_err(|_| ControlPlaneError::InvalidInput("provider_package").into())
}

async fn adapt_or_ensure_model_supports_content_blocks<R>(
    repository: &R,
    instance: &domain::ModelProviderInstanceRecord,
    package: &ProviderPackage,
    model_id: &str,
    input: &mut ProviderInvocationInput,
) -> Result<()>
where
    R: ModelProviderRepository,
{
    if !provider_input_has_media_content_blocks(input) {
        return Ok(());
    }

    if selected_model_supports_multimodal(repository, instance, package, model_id).await? {
        return Ok(());
    }

    textualize_media_content_blocks_for_text_model(input);
    Ok(())
}

fn provider_input_has_media_content_blocks(input: &ProviderInvocationInput) -> bool {
    input.messages.iter().any(|message| {
        message
            .content_blocks
            .as_ref()
            .is_some_and(content_blocks_have_media)
    })
}

fn content_blocks_have_media(content_blocks: &Value) -> bool {
    content_blocks.as_array().is_some_and(|blocks| {
        blocks.iter().any(|block| {
            block
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(is_media_block_type)
        })
    })
}

pub(super) fn textualize_media_content_blocks_for_text_model(input: &mut ProviderInvocationInput) {
    let routed_media_tools = routed_media_tool_context(input);
    for message in &mut input.messages {
        let Some(content_blocks) = message.content_blocks.take() else {
            continue;
        };
        let media_blocks = summarize_media_blocks(&content_blocks);
        if media_blocks.as_array().is_none_or(Vec::is_empty) {
            message.content_blocks = Some(content_blocks);
            continue;
        }
        let fallback = if let Some(routed_media_tools) = &routed_media_tools {
            routed_media_guidance_content(routed_media_tools, &media_blocks)
        } else {
            unsupported_media_content(&message.role, &media_blocks)
        };
        if message.content.trim().is_empty() {
            message.content = fallback;
        } else {
            message.content = format!("{}\n{}", message.content.trim_end(), fallback);
        }
        if let Some(remaining_content_blocks) =
            retain_non_text_media_content_blocks(&content_blocks)
        {
            message.content_blocks = Some(remaining_content_blocks);
        }
    }
}

fn routed_media_tool_context(input: &ProviderInvocationInput) -> Option<Value> {
    let tools = input
        .run_context
        .get(VISIBLE_INTERNAL_LLM_MEDIA_TOOLS_CONTEXT_KEY)?
        .as_array()?
        .iter()
        .filter_map(|tool| {
            let object = tool.as_object()?;
            let name = object
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|name| !name.is_empty())?;
            let media_kind = object
                .get("media_kind")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|kind| !kind.is_empty())
                .unwrap_or("image");
            Some(json!({
                "name": name,
                "media_kind": media_kind,
            }))
        })
        .collect::<Vec<_>>();

    (!tools.is_empty()).then(|| Value::Array(tools))
}

fn routed_media_guidance_content(routed_media_tools: &Value, media_blocks: &Value) -> String {
    json!({
        "event": "routed_media_content_available",
        "message": "Media content is available in conversation history for routed media tools. Call the routed media tool again with the same media path and task; do not ask the user to re-upload the image.",
        "media_tools": routed_media_tools,
        "media_blocks": media_blocks,
    })
    .to_string()
}

fn unsupported_media_content(role: &ProviderMessageRole, media_blocks: &Value) -> String {
    let (error_code, message_text) = if matches!(role, ProviderMessageRole::Tool) {
        (
            "tool_result_media_unsupported",
            "Tool result contained media blocks that were not injected into the selected text model context.",
        )
    } else {
        (
            "message_media_unsupported",
            "Message contained media blocks that were not injected into the selected text model context.",
        )
    };
    json!({
        "error_code": error_code,
        "message": message_text,
        "recoverable": true,
        "media_blocks": media_blocks,
    })
    .to_string()
}

fn summarize_media_blocks(content_blocks: &Value) -> Value {
    let Some(blocks) = content_blocks.as_array() else {
        return Value::Array(Vec::new());
    };
    Value::Array(blocks.iter().filter_map(summarize_media_block).collect())
}

fn summarize_media_block(block: &Value) -> Option<Value> {
    let block_type = block.get("type").and_then(Value::as_str)?;
    if !is_media_block_type(block_type) {
        return None;
    }
    let mut summary = serde_json::Map::new();
    summary.insert("type".to_string(), Value::String(block_type.to_string()));
    if let Some(source_type) = block
        .get("source")
        .and_then(|source| source.get("type"))
        .and_then(Value::as_str)
    {
        summary.insert(
            "source_type".to_string(),
            Value::String(source_type.to_string()),
        );
    }
    if let Some(media_type) = block
        .get("source")
        .and_then(|source| source.get("media_type"))
        .or_else(|| block.get("media_type"))
        .and_then(Value::as_str)
    {
        summary.insert(
            "media_type".to_string(),
            Value::String(media_type.to_string()),
        );
    }
    if let Some(url) = block
        .get("image_url")
        .and_then(|image_url| image_url.get("url"))
        .or_else(|| block.get("source").and_then(|source| source.get("url")))
        .and_then(Value::as_str)
    {
        summary.insert("url".to_string(), Value::String(summarized_media_url(url)));
    }
    Some(Value::Object(summary))
}

fn summarized_media_url(url: &str) -> String {
    let trimmed = url.trim();
    if !trimmed.starts_with("data:") {
        return trimmed.to_string();
    }
    let prefix = trimmed
        .split_once(',')
        .map(|(prefix, _)| prefix)
        .unwrap_or("data:[redacted]");
    format!("{prefix},[redacted]")
}

fn retain_non_text_media_content_blocks(content_blocks: &Value) -> Option<Value> {
    let blocks = content_blocks.as_array()?;
    let retained = blocks
        .iter()
        .filter(|block| {
            !block
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|block_type| block_type == "text" || is_media_block_type(block_type))
        })
        .cloned()
        .collect::<Vec<_>>();
    (!retained.is_empty()).then_some(Value::Array(retained))
}

fn is_media_block_type(block_type: &str) -> bool {
    matches!(
        block_type,
        "image" | "document" | "image_url" | "input_image"
    )
}

async fn selected_model_supports_multimodal<R>(
    repository: &R,
    instance: &domain::ModelProviderInstanceRecord,
    package: &ProviderPackage,
    model_id: &str,
) -> Result<bool>
where
    R: ModelProviderRepository,
{
    if let Some(supports_multimodal) = instance
        .configured_models
        .iter()
        .find(|model| model.enabled && model.model_id == model_id)
        .and_then(|model| model.supports_multimodal)
    {
        return Ok(supports_multimodal);
    }

    if let Some(cache) = repository.get_catalog_cache(instance.id).await? {
        let models: Vec<ProviderModelDescriptor> = serde_json::from_value(cache.models_json)?;
        if let Some(model) = models.iter().find(|model| model.model_id == model_id) {
            return Ok(model.supports_multimodal);
        }
    }

    if let Some(model) = package
        .predefined_models
        .iter()
        .find(|model| model.model_id == model_id)
    {
        return Ok(model.supports_multimodal);
    }

    Ok(false)
}

pub(super) async fn freeze_failover_queue_routes<R>(
    repository: &R,
    compiled_plan: &mut orchestration_runtime::compiled_plan::CompiledPlan,
) -> Result<()>
where
    R: ModelProviderRepository,
{
    for node in compiled_plan.nodes.values_mut() {
        let Some(runtime) = node.llm_runtime.as_mut() else {
            continue;
        };
        let Some(routing) = runtime.routing.as_mut() else {
            continue;
        };
        if routing.routing_mode
            != orchestration_runtime::compiled_plan::LlmRoutingMode::FailoverQueue
            || !routing.queue_targets.is_empty()
        {
            continue;
        }

        let queue_template_id = routing
            .queue_template_id
            .as_deref()
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or(ControlPlaneError::InvalidInput("queue_template_id"))?;
        let queue = repository
            .get_failover_queue_template(queue_template_id)
            .await?
            .ok_or(ControlPlaneError::InvalidInput("queue_template_id"))?;
        if queue.status != "active" {
            return Err(ControlPlaneError::InvalidInput("queue_template_id").into());
        }
        let items = repository
            .list_failover_queue_items(queue_template_id)
            .await?;
        let snapshot_items = items
            .iter()
            .cloned()
            .map(FailoverQueueSnapshotItem::from)
            .collect::<Vec<_>>();
        let snapshot = repository
            .create_failover_queue_snapshot(&crate::ports::CreateModelFailoverQueueSnapshotInput {
                snapshot_id: Uuid::now_v7(),
                queue_template_id,
                version: queue.version,
                items: freeze_queue_items(&snapshot_items),
            })
            .await?;
        routing.queue_snapshot_id = Some(snapshot.id.to_string());
        let provider_display_names = routing
            .queue_targets
            .iter()
            .map(|target| {
                (
                    target.provider_instance_id.clone(),
                    target.provider_instance_display_name.clone(),
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        routing.queue_targets = snapshot_items
            .into_iter()
            .filter(|item| item.enabled)
            .map(
                |item| orchestration_runtime::compiled_plan::CompiledLlmRouteTarget {
                    provider_instance_id: item.provider_instance_id.to_string(),
                    provider_instance_display_name: provider_display_names
                        .get(&item.provider_instance_id.to_string())
                        .cloned()
                        .unwrap_or_default(),
                    provider_code: item.provider_code,
                    protocol: item.protocol,
                    upstream_model_id: item.upstream_model_id,
                },
            )
            .collect();
        let Some(first_target) = routing.queue_targets.first() else {
            return Err(ControlPlaneError::InvalidInput("queue_template_id").into());
        };
        runtime.provider_instance_id = first_target.provider_instance_id.clone();
        runtime.provider_code = first_target.provider_code.clone();
        runtime.protocol = first_target.protocol.clone();
        runtime.model = first_target.upstream_model_id.clone();
    }

    Ok(())
}

#[cfg(test)]
#[path = "../_tests/orchestration_runtime/support.rs"]
pub(crate) mod test_support;
