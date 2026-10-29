use super::*;

#[async_trait]
impl ProviderRuntimePort for ApiProviderRuntime {
    async fn reconcile_provider_worker_demand(
        &self,
        installation: &domain::PluginInstallationRecord,
        revision: u64,
        selectable: Option<bool>,
    ) -> anyhow::Result<()> {
        self.services
            .runtime_backend
            .reconcile_provider_worker_demand(&installation.plugin_id, revision, selectable)
            .await
            .map_err(map_runtime_backend_error)
    }

    async fn provider_distribution_registry_fingerprint(&self) -> anyhow::Result<String> {
        Ok(self.services.provider_distribution_fingerprint().await)
    }

    async fn select_provider_distribution(
        &self,
        rule_id: &str,
        invocation: extension_contracts::ProviderDistributionInvocation,
        context: ProviderRuntimeExecutionContext,
    ) -> anyhow::Result<extension_contracts::ProviderDistributionSelectionReceipt> {
        let snapshot = self.services.provider_distribution_snapshot.read().await;
        let plugin_id = snapshot.resolve_runtime(
            rule_id,
            &invocation.rule_version,
            &invocation.contract_version,
            &invocation.config,
            &invocation.registry_fingerprint,
        )?;
        let plugin_id = plugin_id.to_string();
        drop(snapshot);
        self.services
            .orchestration_backend
            .select_provider_distribution(
                runtime_core::runtime_backend::RuntimeProviderDistributionRequest {
                    request_id: RuntimeRequestId::new(Uuid::now_v7().to_string())?,
                    target: RuntimeTargetId::new(plugin_id)?,
                    invocation,
                    principal: runtime_core::runtime_backend::RuntimeExecutionPrincipal {
                        caller: None,
                        workspace_id: context.workspace_id.to_string(),
                        actor_id: context.actor_id.map(|id| id.to_string()),
                        deadline_unix_ms: context.deadline_unix_ms,
                    },
                },
            )
            .await
            .map_err(map_runtime_backend_error)
    }

    async fn prepare_managed_artifact_removal(
        &self,
        installation_ids: &[Uuid],
    ) -> anyhow::Result<Box<dyn Send + Sync>> {
        control_plane_contracts::ports::ManagedArtifactRemovalGuard::prepare_managed_artifact_removal(
            self.services.managed_composition()?,
            installation_ids,
        ).await
    }

    async fn guard_managed_artifact_removal(
        &self,
        installation_ids: &[Uuid],
    ) -> anyhow::Result<Box<dyn Send + Sync>> {
        control_plane_contracts::ports::ManagedArtifactRemovalGuard::guard_managed_artifact_removal(
            self.services.managed_composition()?.as_ref(),
            installation_ids,
        )
        .await
    }
    async fn switch_managed_installation(
        &self,
        workspace_id: Uuid,
        current: Uuid,
        target: Uuid,
        audit: domain::AuditLogRecord,
        task: domain::PluginTaskRecord,
    ) -> anyhow::Result<domain::PluginTaskRecord> {
        self.services
            .managed_composition()?
            .switch_installation(workspace_id, current, target, audit, task)
            .await
    }

    async fn activate_plugin(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
    ) -> anyhow::Result<()> {
        match installation.contract_version.as_str() {
            "1flowbase.extension-bus/v1" => {
                self.services
                    .managed_composition()?
                    .rebuild_installation(installation.id)
                    .await
            }
            plugin_framework::provider_contract::CURRENT_PROVIDER_CONTRACT => {
                let binding = self.resolve_model_provider_binding(installation)?;
                self.ensure_provider_loaded(&binding).await
            }
            "1flowbase.data_source/v1" => self.ensure_data_source_loaded(installation).await,
            "1flowbase.capability/v1" => self.ensure_capability_loaded(installation).await,
            // Network egress workers receive their private configuration only through `sync`.
            // Installation activation is intentionally deferred so no worker can start without it.
            plugin_framework::NETWORK_EGRESS_PROVIDER_CONTRACT => Ok(()),
            extension_contracts::PROVIDER_DISTRIBUTION_RULE_CONTRACT_V1 => {
                self.services
                    .activate_provider_distribution_runtime(installation)
                    .await
            }
            _ => Err(ControlPlaneError::InvalidInput("plugin_installation").into()),
        }
    }

    async fn deactivate_plugin(
        &self,
        installation: &domain::PluginInstallationRecord,
    ) -> anyhow::Result<()> {
        match installation.contract_version.as_str() {
            "1flowbase.extension-bus/v1" => {
                self.services
                    .managed_composition()?
                    .rebuild_installation(installation.id)
                    .await
            }
            plugin_framework::provider_contract::CURRENT_PROVIDER_CONTRACT => self
                .services
                .runtime_backend
                .deactivate_provider(&installation.plugin_id)
                .await
                .map_err(map_runtime_backend_error),
            "1flowbase.data_source/v1" => {
                self.services
                    .runtime_backend
                    .deactivate_data_source(&installation.plugin_id)
                    .await
                    .map_err(map_runtime_backend_error)?;
                Ok(())
            }
            "1flowbase.capability/v1" => {
                self.services
                    .runtime_backend
                    .deactivate_capability(&installation.plugin_id)
                    .await
                    .map_err(map_runtime_backend_error)?;
                Ok(())
            }
            // Network egress workers are provider-instance scoped. Artifact deactivation does not
            // identify an instance and therefore cannot own its worker lifecycle.
            plugin_framework::NETWORK_EGRESS_PROVIDER_CONTRACT => Ok(()),
            extension_contracts::PROVIDER_DISTRIBUTION_RULE_CONTRACT_V1 => {
                self.services
                    .deactivate_provider_distribution_runtime(&installation.plugin_id)
                    .await
            }
            _ => Err(ControlPlaneError::InvalidInput("plugin_installation").into()),
        }
    }

    async fn pipeline_provider_input(
        &self,
        input: ProviderInvocationInput,
    ) -> std::result::Result<
        orchestration_runtime::provider_input_pipeline::ProviderInputPipelineOutput,
        orchestration_runtime::provider_input_pipeline::ProviderInputPipelineError,
    > {
        match self.services.provider_input_pipeline.as_ref() {
            Some(pipeline) => pipeline.execute(input).await,
            None => Ok(
                orchestration_runtime::provider_input_pipeline::ProviderInputPipelineOutput::unchanged(
                    input,
                ),
            ),
        }
    }

    async fn ensure_loaded(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
    ) -> anyhow::Result<()> {
        let binding = self.resolve_model_provider_binding(installation)?;
        self.ensure_provider_loaded(&binding).await
    }

    async fn validate_provider(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        provider_config: Value,
    ) -> anyhow::Result<Value> {
        let binding = self.resolve_model_provider_binding(installation)?;
        self.ensure_provider_loaded(&binding).await?;
        self.services
            .runtime_backend
            .provider_validate(&binding.plugin_id, provider_config)
            .await
            .map_err(map_runtime_backend_error)
    }

    async fn authenticate_provider(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        provider_config: Value,
        operation: ProviderAuthOperation,
    ) -> anyhow::Result<ProviderAuthResult> {
        let binding = self.resolve_model_provider_binding(installation)?;
        self.ensure_provider_loaded(&binding).await?;
        self.services
            .runtime_backend
            .provider_authenticate(&binding.plugin_id, provider_config, operation)
            .await
            .map_err(map_runtime_backend_error)
    }

    async fn list_models(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        provider_config: Value,
    ) -> anyhow::Result<Vec<ProviderModelDescriptor>> {
        let binding = self.resolve_model_provider_binding(installation)?;
        self.ensure_provider_loaded(&binding).await?;
        self.services
            .runtime_backend
            .provider_list_models(&binding.plugin_id, provider_config)
            .await
            .map_err(map_runtime_backend_error)
    }

    async fn get_balance(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        provider_config: Value,
    ) -> anyhow::Result<ProviderBalanceResult> {
        let binding = self.resolve_model_provider_binding(installation)?;
        self.ensure_provider_loaded(&binding).await?;
        self.services
            .runtime_backend
            .provider_get_balance(&binding.plugin_id, provider_config)
            .await
            .map_err(map_runtime_backend_error)
    }

    async fn get_usage_windows(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        provider_config: Value,
    ) -> anyhow::Result<ProviderUsageWindowsResult> {
        let binding = self.resolve_model_provider_binding(installation)?;
        self.ensure_provider_loaded(&binding).await?;
        self.services
            .runtime_backend
            .provider_get_usage_windows(&binding.plugin_id, provider_config)
            .await
            .map_err(map_runtime_backend_error)
    }

    async fn reset_credit(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        provider_config: Value,
        reset_credit_operation: ProviderResetCreditOperation,
    ) -> anyhow::Result<ProviderResetCreditResult> {
        let binding = self.resolve_model_provider_binding(installation)?;
        self.ensure_provider_loaded(&binding).await?;
        self.services
            .runtime_backend
            .provider_reset_credit(&binding.plugin_id, provider_config, reset_credit_operation)
            .await
            .map_err(map_runtime_backend_error)
    }

    async fn count_tokens(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: ProviderCountTokensInput,
    ) -> anyhow::Result<ProviderCountTokensResult> {
        let mut invocation = input.into_invocation();
        transport_session_lifecycle::take_transport_connection_scope(&mut invocation);
        let input = ProviderCountTokensInput::from_invocation(invocation);
        let binding = self.resolve_model_provider_binding(installation)?;
        binding.require_provider_code(&input.as_invocation().provider_code)?;
        let activity = self.start_runtime_activity(ApplicationActivityKind::ModelRequest);
        let operation_name = "count_tokens";
        trace_provider_operation_boundary(&binding, operation_name, "start", "started");
        let result = async {
            self.ensure_provider_loaded(&binding).await?;
            self.services
                .runtime_backend
                .provider_count_tokens(&binding.plugin_id, input)
                .await
                .map_err(map_runtime_backend_error)
        }
        .await;
        trace_provider_operation_boundary(
            &binding,
            operation_name,
            "end",
            if result.is_ok() {
                "succeeded"
            } else {
                "failed"
            },
        );
        finish_runtime_activity(activity, &result);
        result
    }

    async fn compact(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        mut input: ProviderInvocationInput,
    ) -> anyhow::Result<ProviderCompactResult> {
        transport_session_lifecycle::take_transport_connection_scope(&mut input);
        let binding = self.resolve_model_provider_binding(installation)?;
        binding.require_provider_code(&input.provider_code)?;
        let activity = self.start_runtime_activity(ApplicationActivityKind::ModelRequest);
        self.ensure_provider_loaded(&binding).await?;
        let result = self
            .services
            .runtime_backend
            .provider_compact(&binding.plugin_id, input)
            .await
            .map_err(map_runtime_backend_error);
        finish_runtime_activity(activity, &result);
        result
    }

    async fn invoke_stream(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        mut input: ProviderInvocationInput,
    ) -> anyhow::Result<ProviderRuntimeInvocationOutput> {
        transport_session_lifecycle::take_transport_connection_scope(&mut input);
        let binding = self.resolve_model_provider_binding(installation)?;
        binding.require_provider_code(&input.provider_code)?;
        let activity = self.start_runtime_activity(ApplicationActivityKind::ModelRequest);
        let operation_name = "invoke_stream";
        trace_provider_operation_boundary(&binding, operation_name, "start", "started");
        let result = async {
            self.ensure_provider_loaded(&binding).await?;
            self.services
                .orchestration_backend
                .execute(runtime_execution_request(&binding, input, None)?)
                .await
                .map(|output| ProviderRuntimeInvocationOutput {
                    events: output.events,
                    result: output.result,
                })
                .map_err(map_runtime_backend_error)
                .and_then(validate_provider_invocation_output)
        }
        .await;
        trace_provider_operation_boundary(
            &binding,
            operation_name,
            "end",
            if result.is_ok() {
                "succeeded"
            } else {
                "failed"
            },
        );
        finish_runtime_activity(activity, &result);
        result
    }

    async fn invoke_stream_with_live_events(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        mut input: ProviderInvocationInput,
        live_events: Option<ProviderLiveEventSenders>,
    ) -> anyhow::Result<ProviderRuntimeInvocationOutput> {
        transport_session_lifecycle::take_transport_connection_scope(&mut input);
        let binding = self.resolve_model_provider_binding(installation)?;
        binding.require_provider_code(&input.provider_code)?;
        let activity = self.start_runtime_activity(ApplicationActivityKind::ModelRequest);
        let operation_name = "invoke_stream_with_live_events";
        trace_provider_operation_boundary(&binding, operation_name, "start", "started");
        let result = async {
            self.ensure_provider_loaded(&binding).await?;
            let sinks = live_events
                .map(|senders| RuntimeStreamSinks {
                    protocol_observation: senders.protocol_observation,
                    required: Some(Arc::new(RuntimeEventChannelSink(senders.required))),
                    diagnostic: Some(Arc::new(RuntimeEventChannelSink(senders.diagnostic))),
                })
                .unwrap_or_default();
            self.services
                .orchestration_backend
                .execute_stream(runtime_execution_request(&binding, input, None)?, sinks)
                .await
                .map(|output| ProviderRuntimeInvocationOutput {
                    events: output.events,
                    result: output.result,
                })
                .map_err(map_runtime_backend_error)
                .and_then(validate_provider_invocation_output)
        }
        .await;
        trace_provider_operation_boundary(
            &binding,
            operation_name,
            "end",
            if result.is_ok() {
                "succeeded"
            } else {
                "failed"
            },
        );
        finish_runtime_activity(activity, &result);
        result
    }

    async fn invoke_stream_with_network_egress(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        mut input: ProviderInvocationInput,
        live_events: Option<ProviderLiveEventSenders>,
        workspace_id: Uuid,
        selector: domain::NetworkEgressConsumerSelector,
    ) -> anyhow::Result<ProviderRuntimeInvocationOutput> {
        let Some(network_egress) = self.network_egress.as_ref() else {
            return self
                .invoke_stream_with_live_events(installation, input, live_events)
                .await;
        };
        let Some(scope) = network_egress.acquire(workspace_id, selector).await? else {
            return self
                .invoke_stream_with_live_events(installation, input, live_events)
                .await;
        };

        input.set_network_egress_context(scope.provider_invocation_context());
        let invocation = self
            .invoke_stream_with_live_events(installation, input, live_events)
            .await;
        let release = scope.release().await;
        match (invocation, release) {
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
            (Ok(output), Ok(())) => Ok(output),
        }
    }

    #[tracing::instrument(
        name = "provider_invocation",
        skip_all,
        fields(
            provider_invocation_id = input.trace_context.get("provider_invocation_id").map(String::as_str),
            provider_instance_id = %input.provider_instance_id,
            model = %input.model,
            deadline_unix_ms = context.deadline_unix_ms,
        )
    )]
    async fn invoke_stream_with_execution_context(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        mut input: ProviderInvocationInput,
        live_events: Option<ProviderLiveEventSenders>,
        context: control_plane::ports::ProviderRuntimeExecutionContext,
        selector: domain::NetworkEgressConsumerSelector,
    ) -> anyhow::Result<ProviderRuntimeInvocationOutput> {
        let binding = self.resolve_model_provider_binding(installation)?;
        binding.require_provider_code(&input.provider_code)?;
        let activity = self.start_runtime_activity(ApplicationActivityKind::ModelRequest);
        tracing::debug!(stage = "provider_load", "provider invocation stage entered");
        self.ensure_provider_loaded(&binding).await?;
        tracing::debug!(
            stage = "network_egress",
            "provider invocation stage entered"
        );
        let scope = match self.network_egress.as_ref() {
            Some(network_egress) => {
                network_egress
                    .acquire(context.workspace_id, selector)
                    .await?
            }
            None => None,
        };
        if let Some(scope) = &scope {
            input.set_network_egress_context(scope.provider_invocation_context());
        }
        let sinks = live_events
            .map(|senders| RuntimeStreamSinks {
                protocol_observation: senders.protocol_observation,
                required: Some(Arc::new(RuntimeEventChannelSink(senders.required))),
                diagnostic: Some(Arc::new(RuntimeEventChannelSink(senders.diagnostic))),
            })
            .unwrap_or_default();
        let principal = runtime_core::runtime_backend::RuntimeExecutionPrincipal {
            caller: None,
            workspace_id: context.workspace_id.to_string(),
            actor_id: context.actor_id.map(|value| value.to_string()),
            deadline_unix_ms: context.deadline_unix_ms,
        };
        tracing::debug!(
            stage = "transport_admission",
            "provider invocation stage entered"
        );
        let transport_invocation = self
            .services
            .transport_sessions
            .prepare(&binding.plugin_id, &mut input, &context)
            .await?;
        let mut invocation = async {
            // Construct before the handoff. Every error still reaches lease finish;
            // dropping this future also retains the prepared reservation's owner.
            let request = runtime_execution_request(&binding, input, Some(principal))?;
            if let Some(prepared) = transport_invocation.as_ref() {
                tracing::debug!(
                    stage = "dispatch_claim",
                    "provider invocation stage entered"
                );
                self.services
                    .transport_sessions
                    .claim_dispatch(prepared)
                    .await?;
            }
            // No await between claiming dispatch and polling the runtime future.
            tracing::debug!(
                stage = "runtime_execution",
                "provider invocation stage entered"
            );
            let output = match (&self.communication_activity, context.flow_run_id) {
                (Some(activity), Some(run_id)) => {
                    activity
                        .execute_provider(
                            run_id,
                            &self.services.orchestration_backend,
                            request,
                            sinks,
                        )
                        .await
                }
                _ => {
                    self.services
                        .orchestration_backend
                        .execute_stream(request, sinks)
                        .await
                }
            };
            output
                .map(|output| ProviderRuntimeInvocationOutput {
                    events: output.events,
                    result: output.result,
                })
                .map_err(map_runtime_backend_error)
                .and_then(validate_provider_invocation_output)
        }
        .await;
        if let Some(prepared) = transport_invocation {
            tracing::debug!(
                stage = "transport_completion",
                "provider invocation stage entered"
            );
            let completion = self
                .services
                .transport_sessions
                .finish(prepared, &invocation)
                .await;
            invocation = preserve_provider_invocation_outcome(invocation, completion);
        }
        tracing::debug!(
            stage = "network_egress_release",
            "provider invocation stage entered"
        );
        let release = match scope {
            Some(scope) => scope.release().await,
            None => Ok(()),
        };
        let result = preserve_provider_invocation_outcome(invocation, release);
        finish_runtime_activity(activity, &result);
        tracing::debug!(
            stage = "completed",
            succeeded = result.is_ok(),
            "provider invocation stage entered"
        );
        result
    }

    async fn acquire_http_node_client(
        &self,
        network_egress: &domain::HttpNodeNetworkEgress,
        workspace_id: Uuid,
        timeout: std::time::Duration,
        verify_ssl: bool,
    ) -> anyhow::Result<Option<orchestration_runtime::execution_engine::HttpRequestClientLease>>
    {
        if matches!(network_egress, domain::HttpNodeNetworkEgress::Direct) {
            return Ok(None);
        }
        let Some(resolver) = self.network_egress.as_ref() else {
            anyhow::ensure!(
                !matches!(network_egress, domain::HttpNodeNetworkEgress::Proxy { .. }),
                "configured HTTP node proxy resolver is unavailable"
            );
            return Ok(None);
        };
        let scope = match network_egress {
            domain::HttpNodeNetworkEgress::Workspace => {
                resolver
                    .acquire(
                        workspace_id,
                        domain::NetworkEgressConsumerSelector::HttpNodeDefault,
                    )
                    .await?
            }
            domain::HttpNodeNetworkEgress::Proxy { pool_member_id } => {
                Some(resolver.acquire_pool_member(*pool_member_id).await?)
            }
            domain::HttpNodeNetworkEgress::Direct => unreachable!(),
        };
        let Some(scope) = scope else {
            return Ok(None);
        };
        scope
            .into_http_request_client_lease(timeout, verify_ssl)
            .await
            .map(Some)
    }
}
