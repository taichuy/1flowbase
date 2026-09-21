use crate::ports::OrchestrationRuntimeRepository;
use anyhow::{anyhow, ensure, Result};
use orchestration_runtime::{compiled_plan::CompiledPlan, execution_state::CheckpointSnapshot};
use serde_json::{json, Value};
use uuid::Uuid;

pub(super) async fn load_snapshot<R: OrchestrationRuntimeRepository>(
    repository: &R,
    successor: &domain::FlowRunRecord,
    plan: &mut CompiledPlan,
) -> Result<Option<CheckpointSnapshot>> {
    let Some(grant) = successor
        .input_payload
        .pointer("/sys/native_inference_recovery")
    else {
        return Ok(None);
    };
    let parse_id = |key: &str| -> Result<Uuid> {
        Ok(Uuid::parse_str(grant[key].as_str().ok_or_else(|| {
            anyhow!("native_recovery_grant_invalid")
        })?)?)
    };
    let callback_id = parse_id("callback_task_id")?;
    let failed_id = parse_id("failed_flow_run_id")?;
    let node_run_id = parse_id("node_run_id")?;
    let node_id = grant["node_id"]
        .as_str()
        .ok_or_else(|| anyhow!("native_recovery_grant_invalid"))?;
    ensure!(
        successor.run_mode == domain::FlowRunMode::PublishedApiRun
            && successor.idempotency_key.as_deref()
                == Some(format!("native-inference-recovery:{callback_id}").as_str()),
        "native_recovery_grant_invalid"
    );
    let context = repository
        .get_callback_resume_context(successor.application_id, callback_id)
        .await?
        .ok_or_else(|| anyhow!("native_recovery_checkpoint_missing"))?;
    ensure!(
        context.flow_run.id == failed_id
            && context.flow_run.status == domain::FlowRunStatus::Failed
            && context.flow_run.publication_version_id == successor.publication_version_id
            && context.flow_run.compiled_plan_id == successor.compiled_plan_id
            && context.flow_run.api_key_id == successor.api_key_id
            && context.flow_run.created_by == successor.created_by
            && context.callback_task.status == domain::CallbackTaskStatus::Completed
            && context.callback_task.node_run_id == node_run_id
            && context.waiting_node.id == node_run_id
            && context.waiting_node.status == domain::NodeRunStatus::Failed
            && context.checkpoint.node_run_id == Some(node_run_id)
            && context.checkpoint.locator_payload["node_id"].as_str() == Some(node_id),
        "native_recovery_checkpoint_mismatch"
    );
    ensure!(
        context
            .flow_run
            .error_payload
            .as_ref()
            .and_then(|error| error.get("native_inference_binding"))
            == grant.get("binding"),
        "native_recovery_configuration_mismatch"
    );
    let mut snapshot =
        crate::orchestration_runtime::persistence::checkpoint_snapshot_from_record_with_context(
            repository,
            &context.checkpoint,
            successor.id,
        )
        .await?;
    let metadata = snapshot
        .variable_pool
        .get(node_id)
        .and_then(|node| node.pointer("/__llm_tool_callback/provider_metadata/native_response"))
        .ok_or_else(|| anyhow!("native_recovery_checkpoint_mismatch"))?;
    ensure!(
        metadata.get("history") == grant.get("history")
            && metadata.get("binding") == grant.get("binding"),
        "native_recovery_checkpoint_mismatch"
    );
    // The predecessor sys/env and every upstream value come from its persisted checkpoint.
    // Only host recovery authorization is overlaid; current mutable environment is never read.
    if !snapshot
        .variable_pool
        .get("sys")
        .is_some_and(Value::is_object)
    {
        snapshot.variable_pool.insert("sys".into(), json!({}));
    }
    let system_variables = snapshot
        .variable_pool
        .get_mut("sys")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| anyhow!("native_recovery_checkpoint_mismatch"))?;
    system_variables.insert("native_inference_recovery".into(), grant.clone());
    let binding = &grant["binding"];
    let text = |key: &str| -> Result<String> {
        binding[key]
            .as_str()
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| anyhow!("native_recovery_binding_missing"))
    };
    orchestration_runtime::execution_engine::validate_native_inference_recovery_scope(
        plan, node_id,
    )?;
    let runtime = plan
        .nodes
        .get_mut(node_id)
        .and_then(|node| node.llm_runtime.as_mut())
        .ok_or_else(|| anyhow!("native_recovery_node_missing"))?;
    runtime.provider_instance_id = text("provider_instance_id")?;
    runtime.provider_code = text("provider_code")?;
    runtime.protocol = text("protocol")?;
    runtime.model = text("model")?;
    runtime.routing = None;
    Ok(Some(snapshot))
}
