mod semantic;
pub use semantic::{correlate_semantic_responses_callback, VerifiedResponsesContinuation};
use std::collections::BTreeSet;

use anyhow::Result;
use control_plane_contracts::application_public_runtime::ApplicationPublishedRunControlRepository;
use domain::{CallbackTaskRecord, CallbackTaskStatus, FlowRunStatus};
use serde_json::{json, Value};

use super::api_keys::ApplicationApiKeyActor;
use super::compat::openai::responses_index::native_tool_results;
use super::compat::openai::OpenAiResponsesEnvelope;
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
    let Ok(envelope) = OpenAiResponsesEnvelope::capture(request.clone()) else {
        return Ok(None);
    };
    correlate_native_responses_callback_from_envelope(repository, actor, &envelope).await
}

pub async fn correlate_native_responses_callback_from_envelope<R>(
    repository: &R,
    actor: &ApplicationApiKeyActor,
    envelope: &OpenAiResponsesEnvelope,
) -> Result<Option<(CallbackTaskRecord, Value)>>
where
    R: ApplicationPublishedRunControlRepository,
{
    // Structural protocol errors remain owned by the Responses translator. Correlation runs
    // before translation and must only claim a request when a bounded index can be built.
    let request = envelope.raw_body();
    let request_index = envelope.index();
    let input_value = &request["input"];
    let index = request_index.input();
    let Some(input) = input_value.as_array() else {
        return Ok(None);
    };
    let output_positions = index.current_tool_output_positions();
    if output_positions.is_empty() {
        return Ok(None);
    }

    let tool_results =
        native_tool_results(output_positions.iter().map(|position| &input[*position]))?;
    let call_ids: BTreeSet<String> = tool_results
        .iter()
        .map(|result| {
            result["tool_call_id"]
                .as_str()
                .expect("validated output ID")
                .to_owned()
        })
        .collect();
    debug_assert!(call_ids.iter().all(|call_id| {
        index
            .outputs_by_call_id()
            .get(call_id)
            .is_some_and(|positions| !positions.is_empty())
    }));
    let previous_response_id = request_index.previous_response_id();
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
    let Some(callback) = candidates.next() else {
        // A self-contained history can come from another session/key or provider.
        // Its past outputs are not submissions to an owned callback round.
        if previous_response_id.is_none() && index.has_assistant_message_after_tool_outputs() {
            return Ok(None);
        }
        let compact_trigger_is_final = input
            .last()
            .and_then(|item| item.get("type"))
            .and_then(Value::as_str)
            == Some("compaction_trigger");
        if compact_trigger_is_final {
            return Ok(None);
        }
        return Err(ControlPlaneError::Conflict("native_tool_output_unknown").into());
    };
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
    if is_independent_session_history(envelope, &flow_run, &call_id_list) {
        return Ok(None);
    }
    // Fork/new-turn sampling replays completed tool history with fresh context and
    // configuration. It must not mutate or recover the historical callback receipt.
    // Pending rounds and explicit response cursors retain the resume checks below.
    if previous_response_id.is_none()
        && callback.status == CallbackTaskStatus::Completed
        && index.has_assistant_message_after_tool_outputs()
    {
        return Ok(None);
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
    // A response cursor proves delta causality. Without one, a request carrying
    // history/context must prove the predecessor prefix; comparing user messages
    // conflates a complete history with a delta and rejects valid context updates.
    // Output-only submissions retain scoped call-id correlation.
    let full_context_required =
        previous_response_id.is_none() && input.len() > output_positions.len();
    let full_context_proven = full_context_required
        && super::compat::openai::history::prove_full_context_input(
            input_value,
            &metadata["history"],
            &call_id_list,
        )
        .is_ok();
    let digest = super::native::NativeExecutionModelParameters::
        configuration_digest_with_published_reasoning_default(&flow_run.input_payload, request)?;
    if metadata.get("configuration_digest").and_then(Value::as_str) != Some(digest.as_str()) {
        // A full next sampling request may refresh tools or generation options. It must
        // still prove this round's complete history and retain the frozen model route.
        // A pending tool result may arrive with newly available tools and appended
        // context in the same sampling request. Its consumption still belongs to the
        // callback owner; completed receipts and exact inference replay are classified there.
        let frozen_model = flow_run
            .input_payload
            .pointer("/sys/requested_model_id")
            .and_then(Value::as_str)
            .filter(|model| !model.is_empty());
        let full_request_proven = previous_response_id.is_none()
            && frozen_model.is_some()
            && frozen_model == request.get("model").and_then(Value::as_str)
            && full_context_proven;
        if !full_request_proven {
            return Err(
                ControlPlaneError::Conflict("native_tool_output_configuration_mismatch").into(),
            );
        }
    }
    if full_context_required && !full_context_proven {
        return Err(ControlPlaneError::Conflict("native_tool_output_history_mismatch").into());
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
            | FlowRunStatus::Incomplete
            | FlowRunStatus::Failed
            | FlowRunStatus::Cancelled
    ) {
        return Err(ControlPlaneError::Conflict("native_tool_output_round_not_pending").into());
    }
    Ok(Some((callback, json!({"tool_results": tool_results}))))
}

/// A self-contained request in another authenticated session starts new sampling.
/// It does not claim the old callback, even if the fork copied a pending round.
/// Codex rebuilds prompt-only item IDs per thread; the predecessor digest belongs
/// exclusively to the original session's recovery path and remains unchanged.
fn is_independent_session_history(
    envelope: &OpenAiResponsesEnvelope,
    flow_run: &domain::FlowRunRecord,
    owned_call_ids: &[String],
) -> bool {
    if envelope.previous_response_id().is_some() {
        return false;
    }
    let identities = envelope.session_identity().zip(
        flow_run
            .input_payload
            .pointer("/sys/provider_session_identity")
            .and_then(Value::as_str),
    );
    if !identities.is_some_and(|(current, original)| {
        !current.is_empty() && !original.is_empty() && current != original
    }) {
        return false;
    }
    let Some(input) = envelope.raw_body()["input"].as_array() else {
        return false;
    };
    let positions = envelope.index().input().current_tool_output_positions();
    let expected: BTreeSet<&str> = owned_call_ids.iter().map(String::as_str).collect();
    if expected.is_empty() || expected.len() != owned_call_ids.len() {
        return false;
    }
    let supplied: BTreeSet<&str> = positions
        .iter()
        .filter_map(|position| {
            let item = &input[*position];
            item.get("output")?;
            item.get("call_id").and_then(Value::as_str)
        })
        .collect();
    if supplied != expected {
        return false;
    }
    let mut remaining = expected;
    for item in &input[..positions[0]] {
        if matches!(
            item.get("type").and_then(Value::as_str),
            Some("function_call" | "custom_tool_call")
        ) {
            if let Some(id) = item.get("call_id").and_then(Value::as_str) {
                remaining.remove(id);
            }
        }
    }
    remaining.is_empty()
}

#[cfg(test)]
#[path = "../_tests/application_public_api/native_tool_resume.rs"]
mod tests;
