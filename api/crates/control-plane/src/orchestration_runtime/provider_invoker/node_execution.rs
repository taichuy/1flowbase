use super::*;

#[async_trait]
impl<R, H> orchestration_runtime::execution_engine::CapabilityInvoker
    for RuntimeProviderInvoker<R, H>
where
    R: crate::ports::BillingRepository
        + ModelDefinitionRepository
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
        if installation.availability_status() != domain::PluginAvailabilityStatus::Available {
            return Err(ControlPlaneError::PluginUnavailable.into());
        }
        let plugin_id = installation.plugin_id.clone();
        let credit_commands_allowed = installation.trust_level == "verified_official";

        let mut output = self
            .runtime
            .execute_node(ExecuteCapabilityNodeInput {
                installation,
                contribution_code: runtime.contribution_code.clone(),
                config_payload,
                input_payload,
            })
            .await?;

        if let Some(command_value) = output
            .output_payload
            .as_object_mut()
            .and_then(|payload| payload.remove("_1flowbase_credit_command"))
        {
            if !credit_commands_allowed {
                return Err(ControlPlaneError::PermissionDenied(
                    "plugin_credit_command_requires_verified_official_plugin",
                )
                .into());
            }
            let request: crate::ports::PluginCreditCommandRequest =
                serde_json::from_value(command_value)
                    .map_err(|_| ControlPlaneError::InvalidInput("plugin_credit_command"))?;
            let result = crate::billing::CreditCommandService::new(self.repository.clone())
                .execute_plugin_request(
                    self.workspace_id,
                    &plugin_id,
                    &output.granted_credit_permissions,
                    request,
                )
                .await?;
            output
                .output_payload
                .as_object_mut()
                .expect("credit command was extracted from an object payload")
                .insert(
                    "_1flowbase_credit_result".to_string(),
                    serde_json::to_value(result)?,
                );
        }

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

    async fn invoke_notification_node(
        &self,
        node: &orchestration_runtime::compiled_plan::CompiledNode,
        resolved_inputs: &serde_json::Map<String, Value>,
    ) -> Result<orchestration_runtime::execution_engine::NativeNotificationInvocationOutput> {
        let context = self
            .flow_execution_context
            .as_ref()
            .ok_or_else(|| anyhow!("notification flow execution context is not configured"))?;
        let sender = context
            .notification_sender
            .as_ref()
            .ok_or_else(|| anyhow!("notification sender is not configured"))?;
        let instance_id = node
            .config
            .get("instance_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("notification node is missing instance_id"))?;
        let instance_id = Uuid::parse_str(instance_id)?;
        let input = resolved_inputs
            .get("input")
            .filter(|input| input.is_object())
            .ok_or_else(|| anyhow!("notification bindings.input must resolve to an object"))?;
        match sender
            .send(
                &context.data_model.actor,
                crate::notifications::NotificationSource::Workflow,
                instance_id,
                input.clone(),
            )
            .await
        {
            Ok(result) => Ok(
                orchestration_runtime::execution_engine::NativeNotificationInvocationOutput {
                    output_payload: serde_json::to_value(result)?,
                    error_payload: None,
                },
            ),
            Err(error) => Ok(
                orchestration_runtime::execution_engine::NativeNotificationInvocationOutput {
                    output_payload: json!({}),
                    error_payload: Some(json!({
                        "kind": "notification_send_failed",
                        "message": error.to_string(),
                        "retryable": false,
                    })),
                },
            ),
        }
    }

    async fn invoke_native_sql_node(
        &self,
        node: &orchestration_runtime::compiled_plan::CompiledNode,
        sql: &str,
    ) -> Result<orchestration_runtime::execution_engine::NativeSqlInvocationOutput> {
        let context = self
            .flow_execution_context
            .as_ref()
            .ok_or_else(|| anyhow!("native SQL flow execution context is not configured"))?;
        let data_source_instance_id = node
            .config
            .get("data_source_instance_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("SQL node is missing data_source_instance_id"))?;
        match context
            .data_model
            .runtime_engine
            .execute_native_sql(self.workspace_id, data_source_instance_id, sql)
            .await
        {
            Ok(output) => Ok(
                orchestration_runtime::execution_engine::NativeSqlInvocationOutput {
                    output_payload: serde_json::to_value(output)?,
                    error_payload: None,
                    metrics_payload: json!({}),
                    debug_payload: json!({
                        "data_source_instance_id": data_source_instance_id,
                        "sql": sql,
                    }),
                },
            ),
            Err(error) => Ok(
                orchestration_runtime::execution_engine::NativeSqlInvocationOutput {
                    output_payload: json!({ "results": [] }),
                    error_payload: Some(native_sql_error_payload(&error)),
                    metrics_payload: json!({}),
                    debug_payload: json!({
                        "data_source_instance_id": data_source_instance_id,
                        "sql": sql,
                    }),
                },
            ),
        }
    }
}

fn native_sql_error_payload(error: &anyhow::Error) -> Value {
    if let Some(runtime_error) = error.downcast_ref::<plugin_framework::ProviderRuntimeError>() {
        return native_sql_provider_error_payload(runtime_error);
    }
    if let Some(framework_error) = error.downcast_ref::<plugin_framework::PluginFrameworkError>() {
        return match framework_error {
            plugin_framework::PluginFrameworkError::RuntimeContract { error } => {
                native_sql_provider_error_payload(error)
            }
            plugin_framework::PluginFrameworkError::InvalidProviderContract { message } => {
                let code = if message.starts_with("data_source_capability_not_supported") {
                    "data_source_capability_not_supported"
                } else {
                    "invalid_native_sql_result_contract"
                };
                json!({
                    "kind": "contract_error",
                    "code": code,
                    "message": message,
                })
            }
            _ => json!({
                "kind": "transport_error",
                "code": "data_source_transport_error",
                "message": framework_error.to_string(),
            }),
        };
    }
    json!({
        "kind": "transport_error",
        "code": "data_source_transport_error",
        "message": error.to_string(),
    })
}

fn native_sql_provider_error_payload(error: &plugin_framework::ProviderRuntimeError) -> Value {
    let code = error
        .provider_details
        .as_ref()
        .and_then(|details| details.get("code"))
        .and_then(Value::as_str)
        .or(error.provider_summary.as_deref())
        .unwrap_or("data_source_error");
    let kind = if code == "outcome_unknown" {
        "outcome_unknown"
    } else if error.kind == plugin_framework::ProviderRuntimeErrorKind::ProviderUpstreamError {
        "source_error"
    } else {
        "transport_error"
    };
    let detail = error
        .provider_details
        .as_ref()
        .and_then(|details| details.get("detail"))
        .cloned()
        .or_else(|| error.provider_details.clone());
    json!({
        "kind": kind,
        "code": code,
        "message": error.message,
        "detail": detail,
    })
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
        let input_payload = self.expose_protocol_contexts_to_code(input_payload).await?;
        orchestration_runtime::execution_engine::CodeInvoker::invoke_code_node(
            &orchestration_runtime::execution_engine::QuickJsCodeInvoker::default(),
            runtime,
            config_payload,
            input_payload,
        )
        .await
    }

    async fn protect_protocol_context_output(
        &self,
        output: &mut orchestration_runtime::execution_engine::CodeInvocationOutput,
        selected_output_paths: &[Vec<String>],
    ) -> Result<()> {
        self.protect_code_protocol_context_output(output, selected_output_paths)
            .await
    }

    async fn protect_protocol_context_logs(
        &self,
        console_logs: &mut Vec<orchestration_runtime::execution_engine::ConsoleLogEntry>,
    ) -> Result<()> {
        self.protect_code_console_logs(console_logs).await
    }
}

