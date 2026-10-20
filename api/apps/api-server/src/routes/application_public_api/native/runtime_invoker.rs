use super::*;

pub(crate) async fn public_mcp_runtime_invoker_for_actor(
    state: &Arc<ApiState>,
    actor: &control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
) -> Result<Arc<virtual_ui::ApiMcpRuntimeToolInvoker>, NativeApiError> {
    Ok(Arc::new(
        virtual_ui::ApiMcpRuntimeToolInvoker::new(
            crate::runtime_internal_tool_invoker_factory(state, &actor.actor)
                .await
                .map_err(|error| service_error(error.0))?,
            HeaderMap::new(),
            actor.actor.clone(),
            Vec::new(),
        )
        .await
        .map_err(|error| service_error(error.0))?,
    ))
}

pub(crate) struct NativeRuntimeInvokerFactoryDependencies {
    pub(crate) runtime_dependencies: virtual_ui::RuntimeInternalToolInvokerDependencies,
    pub(crate) interface_catalog:
        crate::routes::mcp_management::interface_catalog::McpInterfaceCatalogDependencies,
    pub(crate) interface_registry:
        Arc<dyn crate::routes::mcp_management::interface_catalog::McpInterfaceRegistrySnapshotPort>,
    pub(crate) interface_dispatch: Arc<dyn virtual_ui::McpInterfaceDispatchPort>,
}

pub(super) struct NativeRuntimeInvokerFactoryAdapter(NativeRuntimeInvokerFactoryDependencies);

pub(super) struct NoMcpRuntimeInternalTools;

#[async_trait::async_trait]
impl orchestration_runtime::execution_engine::RuntimeInternalToolInvoker
    for NoMcpRuntimeInternalTools
{
    fn registrations_for_node(
        &self,
        _node: &orchestration_runtime::compiled_plan::CompiledNode,
    ) -> Vec<orchestration_runtime::execution_engine::RuntimeInternalToolRegistration> {
        Vec::new()
    }

    async fn invoke_runtime_internal_tool(
        &self,
        _node: &orchestration_runtime::compiled_plan::CompiledNode,
        _registration: &orchestration_runtime::execution_engine::RuntimeInternalToolRegistration,
        _arguments: Value,
    ) -> anyhow::Result<orchestration_runtime::execution_engine::RuntimeInternalToolOutput> {
        anyhow::bail!("published plan has no MCP runtime registrations")
    }
}

pub(super) fn compiled_plan_has_mcp_bindings(plan: &Value) -> Option<bool> {
    let nodes = plan.get("nodes")?.as_object()?;
    let mut found = false;
    for node in nodes.values() {
        let config = node.get("config")?.as_object()?;
        match config.get("mcp_instance_ids") {
            None | Some(Value::Null) => {}
            Some(Value::Array(ids)) if ids.iter().all(Value::is_string) => {
                found |= !ids.is_empty();
            }
            _ => return None,
        }
    }
    Some(found)
}

pub(super) async fn plan_has_mcp_bindings_for_run(
    dependencies: &ApplicationNativeRunDependencies,
    run: &NativeRunResult,
) -> anyhow::Result<Option<bool>> {
    let Some(flow_run) = dependencies
        .store
        .get_flow_run(run.application_id, run.id)
        .await?
    else {
        return Ok(None);
    };
    let Some(compiled_plan_id) = flow_run.compiled_plan_id else {
        return Ok(None);
    };
    let store = dependencies.store.clone();
    let Some(plan) = dependencies
        .published_plan_cache
        .get_or_load(
            compiled_plan_id,
            Box::new(move || {
                Box::pin(async move { store.get_compiled_plan(compiled_plan_id).await })
            }),
        )
        .await?
    else {
        return Ok(None);
    };
    Ok(compiled_plan_has_mcp_bindings(&plan.plan))
}

pub(crate) async fn runtime_internal_tool_invoker_for_run(
    dependencies: &ApplicationNativeRunDependencies,
    actor: &control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
    run: &NativeRunResult,
) -> Result<
    Arc<dyn orchestration_runtime::execution_engine::RuntimeInternalToolInvoker>,
    NativeApiError,
> {
    match plan_has_mcp_bindings_for_run(dependencies, run).await {
        Ok(Some(false)) => {
            debug!(flow_run_id = %run.id, "published plan has no MCP bindings; skipping runtime MCP catalog");
            Ok(Arc::new(NoMcpRuntimeInternalTools))
        }
        Ok(Some(true) | None) => dependencies.runtime_invoker_factory.for_actor(actor).await,
        Err(error) => {
            debug!(flow_run_id = %run.id, error = %error, "MCP plan inspection failed; building runtime MCP catalog");
            dependencies.runtime_invoker_factory.for_actor(actor).await
        }
    }
}

impl NativeRuntimeInvokerFactory for NativeRuntimeInvokerFactoryAdapter {
    fn for_actor<'a>(
        &'a self,
        actor: &'a control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
    ) -> RuntimeInvokerFuture<'a> {
        let actor = actor.clone();
        Box::pin(async move {
            let catalog_dependencies = self
                .0
                .interface_catalog
                .with_interface_registry_snapshot(self.0.interface_registry.snapshot());
            let interface_catalog =
                crate::routes::mcp_management::interface_catalog::mcp_interface_catalog_entries_with(
                    &catalog_dependencies,
                    &actor.actor,
                )
                .await
                .map_err(|error| service_error(error.0))?;
            let invoker = virtual_ui::ApiMcpRuntimeToolInvoker::new(
                virtual_ui::RuntimeInternalToolInvokerFactory::new(
                    self.0.runtime_dependencies.clone(),
                    Arc::new(virtual_ui::McpInterfaceCatalogSnapshot::new(
                        interface_catalog,
                    )),
                    Arc::clone(&self.0.interface_dispatch),
                ),
                HeaderMap::new(),
                actor.actor.clone(),
                // Public application runs select MCP only through compiled node bindings.
                Vec::new(),
            )
            .await
            .map_err(|error| service_error(error.0))?;
            Ok(Arc::new(invoker)
                as Arc<
                    dyn orchestration_runtime::execution_engine::RuntimeInternalToolInvoker,
                >)
        })
    }
}

pub(crate) fn native_runtime_invoker_factory(
    dependencies: NativeRuntimeInvokerFactoryDependencies,
) -> Arc<dyn NativeRuntimeInvokerFactory> {
    Arc::new(NativeRuntimeInvokerFactoryAdapter(dependencies))
}
