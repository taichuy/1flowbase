//! Host-only admission proof for a complete Anthropic callback request.
use anyhow::Result;
use domain::orchestration::AnthropicContinuation;
use plugin_framework::provider_contract::ProtocolContextEnvelope;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use uuid::Uuid;

use super::run_service::ApplicationPublishedRunControlRepository;
use super::{
    api_keys::ApplicationApiKeyActor, callback_tool_ids::decode_anthropic_callback_tool_use_id,
};
use crate::errors::ControlPlaneError;

#[derive(Debug, Clone, PartialEq)]
pub struct VerifiedAnthropicContinuation {
    application_id: Uuid,
    actor_user_id: Uuid,
    api_key_id: Uuid,
    callback_task_id: Uuid,
    payload: Value,
    continuation: AnthropicContinuation,
}

impl VerifiedAnthropicContinuation {
    pub fn payload(&self) -> &Value {
        &self.payload
    }
    pub(crate) fn continuation(&self) -> &AnthropicContinuation {
        &self.continuation
    }
    pub(crate) fn validate(
        &self,
        application_id: Uuid,
        actor_user_id: Uuid,
        callback_task_id: Uuid,
        payload: &Value,
    ) -> Result<()> {
        if self.application_id != application_id
            || self.actor_user_id != actor_user_id
            || self.callback_task_id != callback_task_id
            || &self.payload != payload
        {
            return Err(
                ControlPlaneError::Conflict("anthropic_continuation_grant_mismatch").into(),
            );
        }
        Ok(())
    }
    pub(crate) fn validate_api_key(&self, api_key_id: Uuid) -> Result<()> {
        if self.api_key_id != api_key_id {
            return Err(ControlPlaneError::PermissionDenied("anthropic_continuation_owner").into());
        }
        Ok(())
    }
}

/// Remove only the adapter-owned callback prefixes at protocol identity fields.
/// Nested tool payload data is opaque and must not be rewritten.
pub fn restore_anthropic_provider_call_ids(request: &Value) -> Value {
    let mut restored = request.clone();
    if let Some(messages) = restored.get_mut("messages").and_then(Value::as_array_mut) {
        for message in messages {
            if let Some(blocks) = message.get_mut("content").and_then(Value::as_array_mut) {
                for block in blocks {
                    let key = match block.get("type").and_then(Value::as_str) {
                        Some("tool_use") => "id",
                        Some("tool_result") => "tool_use_id",
                        _ => continue,
                    };
                    if let Some((_, original)) = block
                        .get(key)
                        .and_then(Value::as_str)
                        .and_then(decode_anthropic_callback_tool_use_id)
                    {
                        block[key] = Value::String(original);
                    }
                }
            }
        }
    }
    restored
}

/// Ownership and the durable expected set, rather than encoded client IDs,
/// grant authority. The complete context participates in reservation equality.
pub async fn verify_anthropic_continuation<R: ApplicationPublishedRunControlRepository>(
    repository: &R,
    actor: &ApplicationApiKeyActor,
    callback_task_id: Uuid,
    tool_results: Value,
    source_body: Value,
    client_protocol_envelope: ProtocolContextEnvelope,
) -> Result<VerifiedAnthropicContinuation> {
    let callback = repository
        .get_published_callback_task(callback_task_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("callback_task"))?;
    let run = repository
        .get_published_flow_run(callback.flow_run_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("flow_run"))?;
    if run.application_id != actor.application_id
        || run.api_key_id != Some(actor.api_key_id)
        || run.created_by != actor.creator_user_id
        || run.run_mode != domain::FlowRunMode::PublishedApiRun
    {
        return Err(ControlPlaneError::PermissionDenied("anthropic_continuation_owner").into());
    }
    let expected = callback.request_payload["tool_calls"]
        .as_array()
        .ok_or(ControlPlaneError::Conflict(
            "anthropic_callback_round_invalid",
        ))?
        .iter()
        .filter(|call| !super::compat::openai::projection::llm_tool_call_is_internal(call))
        .filter_map(|call| call["id"].as_str())
        .collect::<BTreeSet<_>>();
    let submitted = tool_results
        .as_array()
        .ok_or(ControlPlaneError::InvalidInput("tool_results"))?;
    let submitted_ids = submitted
        .iter()
        .filter_map(|result| result["tool_call_id"].as_str())
        .collect::<BTreeSet<_>>();
    if callback.callback_kind != "llm_tool_calls"
        || expected.is_empty()
        || submitted_ids.len() != submitted.len()
        || submitted_ids != expected
    {
        return Err(ControlPlaneError::Conflict("anthropic_callback_call_set_mismatch").into());
    }
    // Validate the entire request, including fields ignored by callback matching.
    let translated = super::compat::anthropic::translate_messages_request(source_body.clone())
        .map_err(|_| ControlPlaneError::InvalidInput("anthropic_continuation_request"))?;
    // Use the same Native prompt shape as a newly translated request; the
    // supplier's guarded source projection can then restore exact wire order.
    let mut ordered_messages = translated.request.history.clone();
    if !translated.request.query.is_empty() {
        ordered_messages.push(json!({"role":"user","content":translated.request.query}));
    }
    let mut model_tool_context = serde_json::Map::new();
    model_tool_context.insert(
        "tools".into(),
        translated
            .request
            .inputs
            .get("tools")
            .cloned()
            .unwrap_or_else(|| json!([])),
    );
    if let Some(choice) = translated.request.inputs.get("tool_choice") {
        model_tool_context.insert("tool_choice".into(), choice.clone());
    }
    let continuation = AnthropicContinuation {
        ordered_messages,
        system: serde_json::to_value(translated.request.system)?,
        model_tool_context: Value::Object(model_tool_context),
        model_parameters: translated
            .request
            .execution
            .model_parameters()
            .map(|parameters| parameters.canonical_value())
            .unwrap_or_else(|| json!({})),
        client_protocol_envelope: serde_json::to_value(client_protocol_envelope)?,
    };
    let payload = json!({"tool_results":tool_results,"anthropic_continuation":continuation});
    Ok(VerifiedAnthropicContinuation {
        application_id: actor.application_id,
        actor_user_id: actor.creator_user_id,
        api_key_id: actor.api_key_id,
        callback_task_id,
        payload,
        continuation,
    })
}

/// Update external request state in the selected durable execution scope. The
/// top-level/root context is shared by the runtime; sibling container rounds
/// retain the exact snapshot from their own admission.
pub(crate) fn refresh_owned_anthropic_context(
    snapshot: &mut orchestration_runtime::execution_state::CheckpointSnapshot,
    start_node_ids: &[String],
    execution_id: Option<Uuid>,
    continuation: &AnthropicContinuation,
) -> Result<()> {
    let refresh_pool = |pool: &mut serde_json::Map<String, Value>| {
        pool.insert(
            plugin_framework::provider_contract::NATIVE_MODEL_TOOL_CONTEXT_PAYLOAD_KEY.into(),
            continuation.model_tool_context.clone(),
        );
        if let Some(sys) = pool
            .entry("sys")
            .or_insert_with(|| json!({}))
            .as_object_mut()
        {
            sys.insert(
                "model_parameters".into(),
                continuation.model_parameters.clone(),
            );
        }
        for node_id in start_node_ids {
            if let Some(start) = pool.get_mut(node_id).and_then(Value::as_object_mut) {
                start.insert(
                    "tools".into(),
                    continuation.model_tool_context["tools"].clone(),
                );
                start.insert(
                    "tool_choice".into(),
                    continuation
                        .model_tool_context
                        .get("tool_choice")
                        .cloned()
                        .unwrap_or(Value::Null),
                );
            }
        }
    };
    refresh_pool(&mut snapshot.variable_pool);
    if let Some(state) = &mut snapshot.container_state {
        refresh_pool(&mut state.root_variable_pool);
        if let Some(execution_id) = execution_id {
            if !state
                .root_nodes
                .values()
                .any(|node| node.identity.execution_id == execution_id)
            {
                let round = state
                    .frames
                    .values_mut()
                    .flat_map(|frame| frame.rounds.values_mut())
                    .find(|round| {
                        round
                            .nodes
                            .values()
                            .any(|node| node.identity.execution_id == execution_id)
                    })
                    .ok_or(ControlPlaneError::Conflict(
                        "anthropic_continuation_scope_missing",
                    ))?;
                refresh_pool(&mut round.variable_pool);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restores_provider_ids_without_mutating_opaque_tool_data() {
        let encoded = super::super::callback_tool_ids::encode_anthropic_callback_tool_use_id(
            Uuid::from_u128(1),
            "original",
        );
        let source = json!({"messages":[
            {"role":"assistant","content":[{"type":"tool_use","id":encoded,"name":"Read","input":{"id":encoded}}]},
            {"role":"user","content":[{"type":"tool_result","tool_use_id":encoded,"content":[{"type":"text","text":encoded}]}]}
        ]});
        let restored = restore_anthropic_provider_call_ids(&source);
        assert_eq!(restored["messages"][0]["content"][0]["id"], "original");
        assert_eq!(
            restored["messages"][1]["content"][0]["tool_use_id"],
            "original"
        );
        assert_eq!(
            restored["messages"][0]["content"][0]["input"]["id"],
            encoded
        );
        assert_eq!(
            restored["messages"][1]["content"][0]["content"][0]["text"],
            encoded
        );
    }
    #[test]
    fn nested_callback_refreshes_owned_round_and_root_without_touching_sibling() {
        let selected_id = Uuid::from_u128(2);
        let sibling_id = Uuid::from_u128(3);
        let frame_id = Uuid::from_u128(1);
        let identity = |id| json!({"execution_id":id,"node_id":"llm","parent_execution_id":frame_id,"round_path":[]});
        let round = |id| {
            json!({"execution_id":id,"round_index":0,"deadline_unix_ms":null,"deadline_owner_execution_id":null,
            "status":"waiting","next_node_index":0,"variable_pool":{"start":{"tools":["old"]},"sys":{"model_parameters":{"max_output_tokens":1}}},
            "active_node_ids":[],"nodes":{"llm":{"identity":identity(id),"input_payload":{},"terminal_trace":null}},"waiting":null,"error_payload":null})
        };
        let mut frames = serde_json::Map::new();
        frames.insert(frame_id.to_string(), json!({"identity":identity(frame_id),"frozen_limit":2,"started_rounds":2,"frozen_items":null,"variables":{},
            "rounds":{"0":round(selected_id),"1":round(sibling_id)},"results":{},"closed":false,"global_failure":null}));
        let mut snapshot: orchestration_runtime::execution_state::CheckpointSnapshot = serde_json::from_value(json!({
            "next_node_index":0,"variable_pool":{"start":{"tools":["old"]}},"active_node_ids":[],
            "container_state":{"version":1,"root_next_node_index":0,"root_variable_pool":{"start":{"tools":["old"]}},
                "root_active_node_ids":[],"frames":frames,"execution_budget":100}
        })).unwrap();
        let sibling_before =
            snapshot.container_state.as_ref().unwrap().frames[&frame_id].rounds[&1].clone();
        let continuation = AnthropicContinuation {
            ordered_messages: vec![],
            system: json!([]),
            model_tool_context: json!({"tools":[{"name":"current"}],"tool_choice":"auto"}),
            model_parameters: json!({"max_output_tokens":200}),
            client_protocol_envelope: json!({}),
        };
        refresh_owned_anthropic_context(
            &mut snapshot,
            &["start".into()],
            Some(selected_id),
            &continuation,
        )
        .unwrap();
        let state = snapshot.container_state.as_ref().unwrap();
        assert_eq!(state.frames[&frame_id].rounds[&1], sibling_before);
        let selected = &state.frames[&frame_id].rounds[&0].variable_pool;
        assert_eq!(
            selected["start"]["tools"],
            continuation.model_tool_context["tools"]
        );
        assert_eq!(
            selected["sys"]["model_parameters"],
            continuation.model_parameters
        );
        assert_eq!(
            state.root_variable_pool["start"]["tools"],
            continuation.model_tool_context["tools"]
        );
        assert_eq!(
            snapshot.variable_pool["start"]["tools"],
            continuation.model_tool_context["tools"]
        );
    }
}
