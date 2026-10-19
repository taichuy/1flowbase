use anyhow::Result;
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::ports::{OrchestrationRuntimeRepository, UpsertDebugVariableCacheEntryInput};

pub(super) async fn persist_debug_variable_cache_entries<R>(
    repository: &R,
    workspace_id: Uuid,
    flow_run: &domain::FlowRunRecord,
    variable_cache: &Map<String, Value>,
) -> Result<()>
where
    R: OrchestrationRuntimeRepository,
{
    for (node_id, payload) in variable_cache {
        let Some(payload) = payload.as_object() else {
            continue;
        };

        for (variable_key, value) in payload {
            repository
                .upsert_debug_variable_cache_entry(&UpsertDebugVariableCacheEntryInput {
                    workspace_id,
                    application_id: flow_run.application_id,
                    draft_id: flow_run.draft_id,
                    actor_user_id: flow_run.created_by,
                    node_id: node_id.clone(),
                    variable_key: variable_key.clone(),
                    value: value.clone(),
                })
                .await?;
        }
    }

    Ok(())
}

pub(super) fn public_node_variable_cache(
    compiled_plan: &orchestration_runtime::compiled_plan::CompiledPlan,
    variable_pool: &Map<String, Value>,
) -> Map<String, Value> {
    variable_pool
        .iter()
        .filter_map(|(node_id, payload)| {
            let node = compiled_plan.nodes.get(node_id)?;
            let payload = payload.as_object()?;
            if node.node_type == "start" {
                return public_start_variable_cache(node_id, payload);
            }

            let public_payload = node
                .outputs
                .iter()
                .filter_map(|output| {
                    if output.key.is_empty() || output.key.starts_with("__") {
                        return None;
                    }
                    read_selector(payload, &output.selector)
                        .or_else(|| payload.get(&output.key))
                        .map(|value| (output.key.clone(), value.clone()))
                })
                .collect::<Map<_, _>>();

            if public_payload.is_empty() {
                return None;
            }

            Some((node_id.clone(), Value::Object(public_payload)))
        })
        .collect()
}

fn public_start_variable_cache(
    node_id: &str,
    payload: &Map<String, Value>,
) -> Option<(String, Value)> {
    let public_payload = payload
        .iter()
        .filter_map(|(key, value)| {
            if key.is_empty() || key.starts_with("__") {
                return None;
            }

            Some((key.clone(), value.clone()))
        })
        .collect::<Map<_, _>>();

    if public_payload.is_empty() {
        return None;
    }

    Some((node_id.to_string(), Value::Object(public_payload)))
}

fn read_selector<'a>(payload: &'a Map<String, Value>, selector: &[String]) -> Option<&'a Value> {
    let (first, rest) = selector.split_first()?;
    let mut current = payload.get(first)?;

    for segment in rest {
        current = current.as_object()?.get(segment)?;
    }

    Some(current)
}
