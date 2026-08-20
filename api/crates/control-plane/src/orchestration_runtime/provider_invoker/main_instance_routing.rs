use super::*;

impl<R, H> RuntimeProviderInvoker<R, H>
where
    R: ModelProviderRepository + PluginRepository + Clone + Send + Sync,
    H: ProviderRuntimePort + Clone + Send + Sync,
{
    pub(super) async fn resolve_current_main_llm_routing(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
    ) -> Result<orchestration_runtime::execution_engine::ResolvedMainLlmRouting> {
        let main_instance = self
            .repository
            .get_main_instance(self.workspace_id, &runtime.provider_code)
            .await?;
        let routing_policy = main_instance.as_ref().and_then(|main| {
            main.model_routing_policies
                .iter()
                .find(|policy| policy.model_id == runtime.model)
        });
        let distribution_rule = map_distribution_rule(
            routing_policy
                .map(|policy| policy.distribution_rule)
                .unwrap_or_default(),
        );
        let excluded = routing_policy
            .map(|policy| {
                policy
                    .excluded_provider_instance_ids
                    .iter()
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>()
            })
            .unwrap_or_default();
        let positions = routing_policy
            .map(|policy| {
                policy
                    .provider_instance_ids
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(position, id)| (id, position))
                    .collect::<std::collections::HashMap<_, _>>()
            })
            .unwrap_or_default();
        let mut instances = self
            .repository
            .list_instances(self.workspace_id)
            .await?
            .into_iter()
            .filter(|instance| main_candidate_matches(instance, runtime, &excluded))
            .collect::<Vec<_>>();
        instances.sort_by(|left, right| {
            positions
                .get(&left.id)
                .copied()
                .unwrap_or(usize::MAX)
                .cmp(&positions.get(&right.id).copied().unwrap_or(usize::MAX))
                .then(left.id.cmp(&right.id))
        });

        let mut candidates = Vec::with_capacity(instances.len());
        for instance in instances {
            let selected_runtime = resolved_main_runtime(
                runtime,
                &instance,
                main_instance.as_ref().map(|main| main.revision),
                distribution_rule,
            );
            match self.resolve_registered_llm_route(&selected_runtime).await {
                Ok(route) => candidates.push(
                    orchestration_runtime::execution_engine::ResolvedMainLlmRouteCandidate {
                        runtime: selected_runtime,
                        route,
                    },
                ),
                Err(error) => tracing::debug!(
                    provider_instance_id = %instance.id,
                    provider_code = %runtime.provider_code,
                    model_id = %runtime.model,
                    error = %error,
                    "main Provider skipped a non-runnable registered instance"
                ),
            }
        }
        if candidates.is_empty() {
            return Err(no_runnable_main_instance(runtime));
        }

        let distribution_key = (distribution_rule
            == orchestration_runtime::compiled_plan::LlmDistributionRule::RoundRobin)
            .then(|| {
                format!(
                    "llm-main:workspace:{}:provider:{}:model:{}",
                    self.workspace_id, runtime.provider_code, runtime.model
                )
            });
        Ok(
            orchestration_runtime::execution_engine::ResolvedMainLlmRouting {
                candidates,
                distribution_rule,
                distribution_key,
            },
        )
    }

    pub(super) async fn resolve_registered_llm_route(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
    ) -> Result<orchestration_runtime::execution_engine::ResolvedProviderRoute> {
        let instance = self.resolve_llm_instance(runtime).await?;
        let installation = self.ready_installation(instance.installation_id).await?;
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
        if installation.availability_status() != domain::PluginAvailabilityStatus::Available {
            return Err(ControlPlaneError::PluginUnavailable.into());
        }
        let package = load_installed_provider_package(&installation)?;
        let runtime_capabilities = package
            .manifest
            .runtime
            .capabilities
            .iter()
            .cloned()
            .collect();
        let plugin_id = installation.plugin_id.clone();
        Ok(
            orchestration_runtime::execution_engine::ResolvedProviderRoute::new(
                runtime_capabilities,
                RuntimeProviderInvocationPin {
                    instance,
                    installation,
                    package,
                },
            )
            .with_runtime_plugin_id(plugin_id),
        )
    }
}

fn main_candidate_matches(
    instance: &domain::ModelProviderInstanceRecord,
    runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
    excluded: &std::collections::BTreeSet<Uuid>,
) -> bool {
    instance.provider_code == runtime.provider_code
        && instance.included_in_main
        && instance.status == domain::ModelProviderInstanceStatus::Ready
        && !excluded.contains(&instance.id)
        && (instance.enabled_model_ids.is_empty()
            || instance
                .enabled_model_ids
                .iter()
                .any(|model_id| model_id == &runtime.model))
}

fn resolved_main_runtime(
    runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
    instance: &domain::ModelProviderInstanceRecord,
    main_instance_revision: Option<i64>,
    distribution_rule: orchestration_runtime::compiled_plan::LlmDistributionRule,
) -> orchestration_runtime::compiled_plan::CompiledLlmRuntime {
    orchestration_runtime::compiled_plan::CompiledLlmRuntime {
        provider_instance_id: instance.id.to_string(),
        provider_instance_display_name: instance.display_name.clone(),
        provider_code: runtime.provider_code.clone(),
        protocol: instance.protocol.clone(),
        model: runtime.model.clone(),
        routing: Some(orchestration_runtime::compiled_plan::CompiledLlmRouting {
            routing_mode: orchestration_runtime::compiled_plan::LlmRoutingMode::FixedModel,
            fixed_model_target: Some(serde_json::json!({
                "routing_owner": "main_instance",
                "main_instance_revision": main_instance_revision,
            })),
            queue_template_id: None,
            queue_snapshot_id: None,
            queue_targets: Vec::new(),
            distribution_rule,
            distribution_key: None,
            context_policy: serde_json::json!({}),
            stream_policy: serde_json::json!({}),
        }),
    }
}

fn no_runnable_main_instance(
    runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
) -> anyhow::Error {
    plugin_framework::PluginFrameworkError::runtime(
        plugin_framework::provider_contract::ProviderRuntimeError::new(
            plugin_framework::provider_contract::ProviderRuntimeErrorKind::ProviderTransportUnavailable,
            "main Provider has no runnable registered instance for the selected model",
        )
        .with_provider_details(serde_json::json!({
            "provider_code": runtime.provider_code,
            "model_id": runtime.model,
        })),
    )
    .into()
}

fn map_distribution_rule(
    value: domain::ModelProviderDistributionRule,
) -> orchestration_runtime::compiled_plan::LlmDistributionRule {
    match value {
        domain::ModelProviderDistributionRule::None => {
            orchestration_runtime::compiled_plan::LlmDistributionRule::None
        }
        domain::ModelProviderDistributionRule::RoundRobin => {
            orchestration_runtime::compiled_plan::LlmDistributionRule::RoundRobin
        }
        domain::ModelProviderDistributionRule::RetryRoundRobin => {
            orchestration_runtime::compiled_plan::LlmDistributionRule::RetryRoundRobin
        }
    }
}
