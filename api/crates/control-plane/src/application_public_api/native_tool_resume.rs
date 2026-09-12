use std::collections::BTreeSet;

use anyhow::Result;
use control_plane_contracts::{
    application_public_runtime::ApplicationPublishedRunControlRepository,
    ports::ProviderTransportPayload,
};
use domain::{CallbackTaskRecord, CallbackTaskStatus, FlowRunStatus};
use serde_json::{json, Value};

use super::api_keys::ApplicationApiKeyActor;
use crate::errors::ControlPlaneError;

/// Correlates a Responses tool-result suffix with one owned, still-waiting round.
/// Admission does not consume the callback; the resume owner claims it atomically.
pub async fn correlate_native_responses_callback<R>(
    repository: &R,
    actor: &ApplicationApiKeyActor,
    request: &Value,
) -> Result<Option<(CallbackTaskRecord, Value)>>
where
    R: ApplicationPublishedRunControlRepository,
{
    let Some(input) = request.get("input").and_then(Value::as_array) else {
        return Ok(None);
    };
    let suffix_start = input
        .iter()
        .rposition(|item| {
            !matches!(
                item.get("type").and_then(Value::as_str),
                Some("function_call_output" | "custom_tool_call_output")
            )
        })
        .map_or(0, |index| index + 1);
    let outputs = &input[suffix_start..];
    if outputs.is_empty() {
        let last_output = input.iter().rposition(|item| {
            matches!(
                item.get("type").and_then(Value::as_str),
                Some("function_call_output" | "custom_tool_call_output")
            )
        });
        if let Some(index) = last_output {
            if !input[index + 1..]
                .iter()
                .any(|item| item.get("role").and_then(Value::as_str) == Some("user"))
            {
                return Err(ControlPlaneError::Conflict("native_tool_output_not_tail").into());
            }
        }
        return Ok(None);
    }

    let mut call_ids = BTreeSet::new();
    let mut tool_results = Vec::with_capacity(outputs.len());
    for output in outputs {
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
    let candidates = repository
        .find_native_responses_callbacks_by_call_ids(
            actor.workspace_id,
            actor.application_id,
            actor.api_key_id,
            actor.creator_user_id,
            &call_ids.iter().cloned().collect::<Vec<_>>(),
        )
        .await?;
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
    if callback.status != CallbackTaskStatus::Pending
        || flow_run.status != FlowRunStatus::WaitingCallback
    {
        return Err(ControlPlaneError::Conflict("native_tool_output_round_not_pending").into());
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
    if let Some(previous) = request.get("previous_response_id") {
        if previous.as_str() != Some(response_id) {
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
    Ok(Some((callback, json!({"tool_results": tool_results}))))
}

#[cfg(test)]
#[path = "../_tests/application_public_api/native_tool_resume.rs"]
mod tests;
