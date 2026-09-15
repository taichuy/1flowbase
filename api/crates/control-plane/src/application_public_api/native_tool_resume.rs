use std::collections::BTreeSet;

use anyhow::Result;
use control_plane_contracts::{
    application_public_runtime::ApplicationPublishedRunControlRepository,
    ports::ProviderTransportPayload,
};
use domain::{CallbackTaskRecord, CallbackTaskStatus, FlowRunStatus};
use serde_json::{json, Value};

use super::api_keys::ApplicationApiKeyActor;
use super::compat::openai::responses_index::ResponsesInputIndex;
use crate::errors::ControlPlaneError;

/// Correlates a Responses tool-result segment with one owned callback round.
/// Admission does not consume the callback; the resume owner claims it atomically.
pub async fn correlate_native_responses_callback<R>(
    repository: &R,
    actor: &ApplicationApiKeyActor,
    request: &Value,
) -> Result<Option<(CallbackTaskRecord, Value)>>
where
    R: ApplicationPublishedRunControlRepository,
{
    let Some(input_value) = request.get("input") else {
        return Ok(None);
    };
    // Structural protocol errors remain owned by the Responses translator. Correlation runs
    // before translation and must only claim a request when a bounded index can be built.
    let Ok(index) = ResponsesInputIndex::build(input_value) else {
        return Ok(None);
    };
    let Some(input) = input_value.as_array() else {
        return Ok(None);
    };
    let output_positions = index.current_tool_output_positions();
    if output_positions.is_empty() {
        return Ok(None);
    }

    let mut call_ids = BTreeSet::new();
    let mut tool_results = Vec::with_capacity(output_positions.len());
    for position in output_positions {
        let output = &input[position];
        let call_id = output
            .get("call_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or(ControlPlaneError::InvalidInput(
                "native_tool_output_call_id",
            ))?;
        let content = output.get("output").ok_or(ControlPlaneError::InvalidInput(
            "native_tool_output_content",
        ))?;
        if !call_ids.insert(call_id.to_owned()) {
            return Err(ControlPlaneError::Conflict("native_tool_output_duplicate").into());
        }
        tool_results.push(json!({"tool_call_id": call_id, "content": content}));
    }
    debug_assert!(call_ids.iter().all(|call_id| {
        index
            .outputs_by_call_id()
            .get(call_id)
            .is_some_and(|positions| !positions.is_empty())
    }));
    let previous_response_id = match request.get("previous_response_id") {
        Some(Value::String(value)) if !value.is_empty() => Some(value.as_str()),
        Some(_) => {
            return Err(ControlPlaneError::Conflict("native_tool_output_response_mismatch").into())
        }
        None => None,
    };
    let call_id_list = call_ids.iter().cloned().collect::<Vec<_>>();
    let candidates = if let Some(response_id) = previous_response_id {
        let response_candidates = repository
            .find_native_responses_callbacks_by_response_id(
                actor.workspace_id,
                actor.application_id,
                actor.api_key_id,
                actor.creator_user_id,
                response_id,
            )
            .await?;
        if response_candidates.is_empty() {
            // Preserve a precise mismatch result when the call set identifies an owned round;
            // this diagnostic fallback never overrides a response-id match.
            repository
                .find_native_responses_callbacks_by_call_ids(
                    actor.workspace_id,
                    actor.application_id,
                    actor.api_key_id,
                    actor.creator_user_id,
                    &call_id_list,
                )
                .await?
        } else {
            response_candidates
        }
    } else {
        repository
            .find_native_responses_callbacks_by_call_ids(
                actor.workspace_id,
                actor.application_id,
                actor.api_key_id,
                actor.creator_user_id,
                &call_id_list,
            )
            .await?
    };
    let mut candidates = candidates.into_iter();
    let callback = candidates
        .next()
        .ok_or(ControlPlaneError::Conflict("native_tool_output_unknown"))?;
    if candidates.next().is_some() {
        return Err(ControlPlaneError::Conflict("native_tool_output_ambiguous_round").into());
    }
    let flow_run = repository
        .get_published_flow_run(callback.flow_run_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("flow_run"))?;
    if flow_run.application_id != actor.application_id
        || flow_run.api_key_id != Some(actor.api_key_id)
        || flow_run.created_by != actor.creator_user_id
    {
        return Err(ControlPlaneError::PermissionDenied("native_tool_output_owner").into());
    }
    let expected_calls = callback
        .request_payload
        .get("tool_calls")
        .and_then(Value::as_array)
        .ok_or(ControlPlaneError::Conflict(
            "native_tool_output_round_invalid",
        ))?;
    let mut expected_ids = BTreeSet::new();
    for call in expected_calls {
        let id = call
            .get("call_id")
            .or_else(|| call.get("id"))
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or(ControlPlaneError::Conflict(
                "native_tool_output_round_invalid",
            ))?;
        if !expected_ids.insert(id.to_owned()) {
            return Err(ControlPlaneError::Conflict("native_tool_output_round_invalid").into());
        }
    }
    if expected_ids != call_ids {
        return Err(ControlPlaneError::Conflict("native_tool_output_incomplete_round").into());
    }
    let metadata = callback
        .request_payload
        .pointer("/provider_metadata/native_response")
        .ok_or(ControlPlaneError::Conflict(
            "native_tool_output_round_invalid",
        ))?;
    let response_id = metadata
        .get("response_id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or(ControlPlaneError::Conflict(
            "native_tool_output_round_invalid",
        ))?;
    if let Some(previous) = previous_response_id {
        if previous != response_id {
            return Err(ControlPlaneError::Conflict("native_tool_output_response_mismatch").into());
        }
    }
    let transport = ProviderTransportPayload::openai_responses(request.clone())?;
    if let Some(expected) = metadata.get("user_messages_digest").and_then(Value::as_str) {
        if let Some(actual) = transport.user_messages_digest()? {
            if actual != expected {
                return Err(
                    ControlPlaneError::Conflict("native_tool_output_user_turn_mismatch").into(),
                );
            }
        }
    }
    let digest = transport.configuration_digest()?;
    if metadata.get("configuration_digest").and_then(Value::as_str) != Some(digest.as_str()) {
        return Err(
            ControlPlaneError::Conflict("native_tool_output_configuration_mismatch").into(),
        );
    }
    // State admission belongs to the callback-resume owner. A completed callback can be an
    // exact transport replay after the original terminal was lost; rejecting it here would hide
    // the durable resume attempt behind `round_not_pending` before idempotency can identify it.
    if !matches!(
        callback.status,
        CallbackTaskStatus::Pending | CallbackTaskStatus::Completed
    ) || !matches!(
        flow_run.status,
        FlowRunStatus::WaitingCallback
            | FlowRunStatus::Running
            | FlowRunStatus::Succeeded
            | FlowRunStatus::Failed
            | FlowRunStatus::Cancelled
    ) {
        return Err(ControlPlaneError::Conflict("native_tool_output_round_not_pending").into());
    }
    Ok(Some((callback, json!({"tool_results": tool_results}))))
}

#[cfg(test)]
#[path = "../_tests/application_public_api/native_tool_resume.rs"]
mod tests;
