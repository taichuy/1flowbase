use super::*;
use domain::orchestration::ResponsesContinuation;
use uuid::Uuid;

/// Minted only after scoped lookup, complete call-set and history verification.
/// No Deserialize implementation: public callback JSON cannot mint this authority.
#[derive(Debug, Clone, PartialEq)]
pub struct VerifiedResponsesContinuation {
    application_id: Uuid,
    actor_user_id: Uuid,
    api_key_id: Uuid,
    callback_task_id: Uuid,
    payload: Value,
    input_history: Value,
}
impl VerifiedResponsesContinuation {
    pub(crate) fn input_history(&self) -> &Value {
        &self.input_history
    }
    pub fn payload(&self) -> &Value {
        &self.payload
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
                ControlPlaneError::Conflict("responses_continuation_grant_mismatch").into(),
            );
        }
        Ok(())
    }
    pub(crate) fn validate_api_key(&self, api_key_id: Uuid) -> Result<()> {
        if self.api_key_id != api_key_id {
            return Err(ControlPlaneError::PermissionDenied("responses_continuation_owner").into());
        }
        Ok(())
    }
    pub fn callback_task_id(&self) -> Uuid {
        self.callback_task_id
    }
}

pub async fn correlate_semantic_responses_callback<R: ApplicationPublishedRunControlRepository>(
    repository: &R,
    actor: &ApplicationApiKeyActor,
    envelope: &OpenAiResponsesEnvelope,
) -> Result<Option<VerifiedResponsesContinuation>> {
    let request = envelope.raw_body();
    let Some(input) = request.get("input").and_then(Value::as_array) else {
        return Ok(None);
    };
    let positions = envelope.index().input().current_tool_output_positions();
    if positions.is_empty() {
        return Ok(None);
    }
    let ids = positions
        .iter()
        .map(|position| {
            input[*position]
                .get("call_id")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or(ControlPlaneError::InvalidInput(
                    "responses_tool_output_call_id",
                ))
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let previous = envelope.previous_response_id();
    let mut candidates = if let Some(id) = previous {
        repository
            .find_semantic_responses_callbacks_by_response_id(
                actor.workspace_id,
                actor.application_id,
                actor.api_key_id,
                actor.creator_user_id,
                id,
            )
            .await?
    } else {
        Vec::new()
    };
    if candidates.is_empty() {
        candidates = repository
            .find_semantic_responses_callbacks_by_call_ids(
                actor.workspace_id,
                actor.application_id,
                actor.api_key_id,
                actor.creator_user_id,
                &ids,
            )
            .await?;
    }
    if candidates.is_empty() {
        return Ok(None);
    }
    if candidates.len() != 1 {
        return Err(ControlPlaneError::Conflict("responses_tool_output_ambiguous_round").into());
    }
    let callback = candidates.remove(0);
    let run = repository
        .get_published_flow_run(callback.flow_run_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("flow_run"))?;
    if run.application_id != actor.application_id
        || run.api_key_id != Some(actor.api_key_id)
        || run.created_by != actor.creator_user_id
    {
        return Err(ControlPlaneError::PermissionDenied("responses_continuation_owner").into());
    }
    let evidence: domain::orchestration::ResponsesRoundEvidence =
        serde_json::from_value(callback.request_payload["responses_round"].clone())
            .map_err(|_| ControlPlaneError::Conflict("responses_history_evidence_missing"))?;
    if previous.is_some_and(|id| id != evidence.response_id) {
        return Err(ControlPlaneError::Conflict("responses_tool_output_response_mismatch").into());
    }
    let expected = callback
        .request_payload
        .get("tool_calls")
        .and_then(Value::as_array)
        .ok_or(ControlPlaneError::Conflict(
            "responses_tool_output_round_invalid",
        ))?
        .iter()
        .filter(|call| !super::super::compat::openai::projection::llm_tool_call_is_internal(call))
        .map(|call| {
            call.get("id")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or(ControlPlaneError::Conflict(
                    "responses_tool_output_round_invalid",
                ))
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let ordered_input = if previous.is_none() && input.len() > expected.len() {
        super::super::compat::openai::history::prove_full_context_input(
            &request["input"],
            &evidence.history,
            &expected,
        )
        .map_err(|_| ControlPlaneError::Conflict("responses_tool_output_history_mismatch"))?
        .continuation()
        .ordered_input
    } else {
        input.clone()
    };
    let mut remaining = expected.iter().map(String::as_str).collect::<BTreeSet<_>>();
    if remaining.len() != expected.len() || remaining.is_empty() {
        return Err(ControlPlaneError::Conflict("responses_tool_output_round_invalid").into());
    }
    let mut results = Vec::new();
    for item in &ordered_input {
        match item.get("type").and_then(Value::as_str) {
            Some("function_call_output") => {
                let id = item.get("call_id").and_then(Value::as_str).ok_or(
                    ControlPlaneError::InvalidInput("responses_tool_output_call_id"),
                )?;
                if !remaining.remove(id) {
                    return Err(ControlPlaneError::Conflict(
                        "responses_tool_output_duplicate_or_unknown",
                    )
                    .into());
                }
                let content = item.get("output").ok_or(ControlPlaneError::InvalidInput(
                    "responses_tool_output_content",
                ))?;
                results.push(json!({"tool_call_id":id,"content":content}));
            }
            Some("function_call" | "custom_tool_call" | "custom_tool_call_output") => {
                return Err(
                    ControlPlaneError::Conflict("responses_tool_output_round_invalid").into(),
                )
            }
            _ => {}
        }
    }
    if !remaining.is_empty() {
        return Err(ControlPlaneError::Conflict("responses_tool_output_incomplete_round").into());
    }
    let digest =
        ProviderTransportPayload::openai_responses(request.clone())?.configuration_digest()?;
    if run
        .input_payload
        .pointer("/sys/responses_configuration_digest")
        .and_then(Value::as_str)
        != Some(digest.as_str())
    {
        return Err(ControlPlaneError::Conflict(
            "semantic_responses_configuration_change_unsupported",
        )
        .into());
    }
    let ordered_messages = super::super::compat::openai::continuation_messages(&ordered_input)
        .map_err(|_| {
            ControlPlaneError::InvalidInput("semantic_responses_continuation_item_unsupported")
        })?;
    let input_history =
        super::super::compat::openai::history::append_items(&evidence.history, &ordered_input)?;
    let continuation = ResponsesContinuation {
        ordered_input,
        ordered_messages,
    };
    let payload = json!({"tool_results":results,"responses_continuation":continuation});
    Ok(Some(VerifiedResponsesContinuation {
        application_id: actor.application_id,
        actor_user_id: actor.creator_user_id,
        api_key_id: actor.api_key_id,
        callback_task_id: callback.id,
        payload,
        input_history,
    }))
}
