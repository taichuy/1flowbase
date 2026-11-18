use super::*;

pub fn compiled_plan_has_mcp_bindings(plan: &Value) -> Option<bool> {
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
            dependencies
                .runtime_invoker_factory
                .without_mcp(&actor.actor)
                .await
        }
        Ok(Some(true) | None) => dependencies.runtime_invoker_factory.for_actor(actor).await,
        Err(error) => {
            debug!(flow_run_id = %run.id, error = %error, "MCP plan inspection failed; building runtime MCP catalog");
            dependencies.runtime_invoker_factory.for_actor(actor).await
        }
    }
}

pub(crate) async fn public_mcp_runtime_invoker_for_actor(
    state: &Arc<ApiState>,
    actor: &control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
) -> Result<
    Arc<dyn orchestration_runtime::execution_engine::RuntimeInternalToolInvoker>,
    NativeApiError,
> {
    crate::generation_dispatch::GenerationDispatch::acquire(state)
        .map_err(|error| service_error(error.0))?
        .public_runtime_invoker(state, actor)
        .await
        .map_err(|error| service_error(error.0))
}
