//! Native inference recovery is separate from the already consumed tool receipt.
use super::*;
use extension_contracts::provider_contract::{
    CommitLevel, ProviderRecoveryDirective, ProviderRecoveryReceipt, RecoveryDisposition,
    RecoveryReason,
};

/// Host-created, non-deserializable admission. Public request metadata cannot mint this grant.
#[derive(Clone, PartialEq, Eq)]
pub struct NativeInferenceRecoveryGrant {
    pub(crate) callback_task_id: Uuid,
    pub(crate) failed_flow_run_id: Uuid,
    pub(crate) publication_version_id: Uuid,
    pub(crate) remaining_attempts: u32,
    pub(crate) absolute_deadline_unix_ms: i64,
    pub(crate) provider_instance_id: String,
    pub(crate) node_id: String,
    pub(crate) node_run_id: Uuid,
    pub(crate) binding: Value,
    pub(crate) history: Value,
    pub(crate) frozen_input_payload: Value,
}

impl std::fmt::Debug for NativeInferenceRecoveryGrant {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NativeInferenceRecoveryGrant")
            .field("callback_task_id", &self.callback_task_id)
            .field("failed_flow_run_id", &self.failed_flow_run_id)
            .field("node_id", &self.node_id)
            .field("remaining_attempts", &self.remaining_attempts)
            .finish_non_exhaustive()
    }
}

pub(crate) fn recovery_key(callback_task_id: Uuid) -> String {
    format!("native-inference-recovery:{callback_task_id}")
}

impl NativeInferenceRecoveryGrant {
    pub(crate) fn durable_value(&self) -> Value {
        json!({"callback_task_id":self.callback_task_id,"failed_flow_run_id":self.failed_flow_run_id,
            "remaining_attempts":self.remaining_attempts,"absolute_deadline_unix_ms":self.absolute_deadline_unix_ms,
            "provider_instance_id":self.provider_instance_id, "node_id":self.node_id,
            "node_run_id":self.node_run_id, "binding":self.binding, "history":self.history})
    }
}

/// Recovery alone reads the identity-scoped Native record. Keep the public callback
/// projection unchanged: it intentionally excludes provider state and private history.
pub(super) async fn load_owned_evidence<R: ApplicationPublishedRunControlRepository>(
    repository: &R,
    actor: &super::super::api_keys::ApplicationApiKeyActor,
    callback: &domain::CallbackTaskRecord,
) -> Result<domain::CallbackTaskRecord> {
    let ids = callback_call_ids(callback)?;
    repository
        .find_native_responses_callbacks_by_call_ids(
            actor.workspace_id,
            actor.application_id,
            actor.api_key_id,
            actor.creator_user_id,
            &ids,
        )
        .await?
        .into_iter()
        .find(|owned| {
            owned.id == callback.id
                && owned.flow_run_id == callback.flow_run_id
                && owned.node_run_id == callback.node_run_id
                && owned.status == callback.status
        })
        .ok_or_else(|| ControlPlaneError::Conflict("native_recovery_history_missing").into())
}

fn callback_call_ids(callback: &domain::CallbackTaskRecord) -> Result<Vec<String>> {
    let reject = || ControlPlaneError::Conflict("native_recovery_history_missing");
    callback.request_payload["tool_calls"]
        .as_array()
        .ok_or_else(reject)?
        .iter()
        .map(|call| {
            call.get("call_id")
                .or_else(|| call.get("id"))
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| reject().into())
        })
        .collect()
}

pub(super) fn qualify(
    flow: &domain::FlowRunRecord,
    callback: &domain::CallbackTaskRecord,
    command: &ResumePublishedCallbackCommand,
    now_ms: i64,
) -> Result<NativeInferenceRecoveryGrant> {
    let reject = |code| anyhow::Error::from(ControlPlaneError::Conflict(code));
    if flow.status != domain::FlowRunStatus::Failed
        || callback.status != domain::CallbackTaskStatus::Completed
        || callback.callback_kind != "llm_tool_calls"
    {
        return Err(reject("native_recovery_not_failed_inference"));
    }
    let error = flow
        .error_payload
        .as_ref()
        .ok_or_else(|| reject("native_recovery_missing_receipt"))?;
    let audit = error
        .get("ai_native_recovery")
        .ok_or_else(|| reject("native_recovery_missing_receipt"))?;
    // NonReproducible is emitted only after the Native semantic barrier and provider receipt
    // validation. A false first-token flag alone does not authorize replay.
    if audit["decision"] != "non_reproducible"
        || error["failed_after_first_token"] != false
        || audit["provider_final_commit"] != "lifecycle_only"
    {
        return Err(reject("native_recovery_semantic_or_untrusted_failure"));
    }
    let directive: ProviderRecoveryDirective =
        serde_json::from_value(audit["provider_directive"].clone())
            .map_err(|_| reject("native_recovery_invalid_receipt"))?;
    let receipt: ProviderRecoveryReceipt =
        serde_json::from_value(audit["provider_inner_receipt"].clone())
            .map_err(|_| reject("native_recovery_invalid_receipt"))?;
    receipt
        .validate_against(&directive)
        .map_err(|_| reject("native_recovery_invalid_receipt"))?;
    if receipt.commit_level != CommitLevel::LifecycleOnly
        || receipt.disposition != RecoveryDisposition::LogicalInvocationRetry
        || !matches!(
            receipt.reason,
            RecoveryReason::TransportDisconnected | RecoveryReason::BudgetExhausted
        )
    {
        return Err(reject("native_recovery_not_transport_failure"));
    }
    let total = audit["total_attempt_budget"]
        .as_u64()
        .filter(|v| *v <= u32::MAX as u64)
        .ok_or_else(|| reject("native_recovery_invalid_budget"))?;
    let charged = audit["total_attempts_charged"]
        .as_u64()
        .ok_or_else(|| reject("native_recovery_invalid_budget"))?;
    if charged == 0 || charged >= total {
        return Err(reject("native_recovery_budget_exhausted"));
    }
    let deadline = directive.policy.budget().absolute_deadline_unix_ms;
    if now_ms >= deadline {
        return Err(reject("native_recovery_deadline_exceeded"));
    }
    validate_context(flow, callback, command)?;
    let binding = error
        .get("native_inference_binding")
        .filter(|value| value.is_object())
        .ok_or_else(|| reject("native_recovery_binding_missing"))?;
    if callback
        .request_payload
        .pointer("/provider_metadata/native_response/binding")
        != Some(binding)
    {
        return Err(reject("native_recovery_configuration_mismatch"));
    }
    let node_id = binding
        .get("node_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| reject("native_recovery_binding_missing"))?;
    Ok(NativeInferenceRecoveryGrant {
        node_id: node_id.to_owned(),
        node_run_id: callback.node_run_id,
        binding: binding.clone(),
        history: callback.request_payload["provider_metadata"]["native_response"]["history"]
            .clone(),
        frozen_input_payload: flow.input_payload.clone(),
        callback_task_id: callback.id,
        failed_flow_run_id: flow.id,
        publication_version_id: flow
            .publication_version_id
            .ok_or_else(|| reject("native_recovery_publication_missing"))?,
        remaining_attempts: (total - charged) as u32,
        absolute_deadline_unix_ms: deadline,
        provider_instance_id: error["provider_instance_id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| reject("native_recovery_provider_missing"))?
            .to_owned(),
    })
}

/// A consumed receipt followed by new context starts a normal invocation. Failure
/// recovery budgets and configuration identity apply only to replay of the old input.
/// The caller has already checked ownership, source and exact accepted tool outputs.
pub(super) fn is_full_context_continuation(
    flow: &domain::FlowRunRecord,
    callback: &domain::CallbackTaskRecord,
    command: &ResumePublishedCallbackCommand,
) -> Result<bool> {
    let Some(transport) = command.native_transport.as_ref() else {
        return Ok(false);
    };
    let body = transport.wire_body();
    if body.get("previous_response_id").is_some() {
        return Ok(false);
    }
    let ids = callback_call_ids(callback)?;
    let Some(metadata) = callback
        .request_payload
        .pointer("/provider_metadata/native_response")
    else {
        return Ok(false);
    };
    let Ok(proof) = super::super::compat::openai::history::prove_full_context_input(
        &body["input"],
        &metadata["history"],
        &ids,
    ) else {
        return Ok(false);
    };
    if proof.context.is_empty() {
        return Ok(false);
    }
    let accepted = command.response_payload["tool_results"].as_array();
    let outputs_match = accepted.is_some_and(|results| {
        results.len() == ids.len()
            && proof.tool_outputs.iter().all(|output| {
                results.iter().any(|result| {
                    result["tool_call_id"] == output["call_id"]
                        && result.get("content") == output.get("output")
                })
            })
    });
    if !outputs_match {
        return Err(ControlPlaneError::Conflict("callback_resume_payload_conflict").into());
    }
    // Full continuations may refresh tools/options, but may not change the model
    // route attached to this receipt. Legacy records require the same configuration.
    let sealed =
        super::super::native::NativeExecutionModelParameters::seal_published_reasoning_default(
            &flow.input_payload,
            transport.clone(),
        )?;
    let model_matches = flow
        .input_payload
        .pointer("/sys/requested_model_id")
        .and_then(Value::as_str)
        .filter(|model| !model.is_empty())
        .map(|model| body.get("model").and_then(Value::as_str) == Some(model));
    if model_matches == Some(false)
        || (model_matches.is_none()
            && metadata["configuration_digest"].as_str()
                != Some(sealed.configuration_digest()?.as_str()))
    {
        return Err(
            ControlPlaneError::Conflict("native_tool_output_configuration_mismatch").into(),
        );
    }
    Ok(true)
}

/// A terminal Responses response cannot be resumed. A later create request may
/// sample again from the same proven input without replaying the consumed tool
/// callback or reopening the failed provider attempt. This is a protocol-level
/// new response, not an AI Native recovery disposition.
pub(super) fn is_terminal_transport_reissue(
    flow: &domain::FlowRunRecord,
    callback: &domain::CallbackTaskRecord,
    command: &ResumePublishedCallbackCommand,
) -> Result<bool> {
    if flow.status != domain::FlowRunStatus::Failed
        || callback.status != domain::CallbackTaskStatus::Completed
        || callback.callback_kind != "llm_tool_calls"
    {
        return Ok(false);
    }
    let Some(error) = flow.error_payload.as_ref() else {
        return Ok(false);
    };
    let audit = &error["ai_native_recovery"];
    if error["error_code"] != "provider_transport_unavailable"
        || audit["decision"] != "semantic_terminal"
        || audit["provider_final_commit"] != "terminal"
    {
        return Ok(false);
    }
    let reject = || {
        anyhow::Error::from(ControlPlaneError::Conflict(
            "native_recovery_invalid_receipt",
        ))
    };
    let directive: ProviderRecoveryDirective =
        serde_json::from_value(audit["provider_directive"].clone()).map_err(|_| reject())?;
    let receipt: ProviderRecoveryReceipt =
        serde_json::from_value(audit["provider_inner_receipt"].clone()).map_err(|_| reject())?;
    receipt.validate_against(&directive).map_err(|_| reject())?;
    if receipt.commit_level != CommitLevel::Terminal
        || receipt.disposition != RecoveryDisposition::TerminalInterruption
        || receipt.reason != RecoveryReason::SemanticFailed
    {
        return Ok(false);
    }
    validate_context(flow, callback, command)?;
    Ok(true)
}

/// Duplicates retain their original budget, but must still match the admitted context.
pub(super) fn validate_context(
    flow: &domain::FlowRunRecord,
    callback: &domain::CallbackTaskRecord,
    command: &ResumePublishedCallbackCommand,
) -> Result<()> {
    let reject = |code| anyhow::Error::from(ControlPlaneError::Conflict(code));
    let transport = command
        .native_transport
        .as_ref()
        .ok_or_else(|| reject("native_recovery_full_context_required"))?;
    if transport.wire_body().get("previous_response_id").is_some() {
        return Err(reject("native_recovery_full_context_required"));
    }
    let metadata = callback
        .request_payload
        .pointer("/provider_metadata/native_response")
        .ok_or_else(|| reject("native_recovery_history_missing"))?;
    let sealed =
        super::super::native::NativeExecutionModelParameters::seal_published_reasoning_default(
            &flow.input_payload,
            transport.clone(),
        )?;
    // A normal full-context continuation may change request configuration. Recovery
    // must replay the failed invocation exactly, not the preceding successful round.
    // Older host records have no invocation digest and retain their original strict proof.
    let expected_configuration = match flow
        .error_payload
        .as_ref()
        .and_then(|error| error.get("native_inference_configuration_digest"))
    {
        Some(digest) => digest.as_str(),
        None => metadata["configuration_digest"].as_str(),
    };
    if expected_configuration != Some(sealed.configuration_digest()?.as_str()) {
        return Err(reject("native_recovery_configuration_mismatch"));
    }
    let ids = callback_call_ids(callback)?;
    super::super::compat::openai::history::validate_full_retry_input(
        &transport.wire_body()["input"],
        &metadata["history"],
        &ids,
    )
    .map_err(|error| {
        reject(match error.to_string().as_str() {
            "native_history_evidence_missing" => "native_recovery_history_missing",
            "native_history_evidence_invalid" | "native_history_item_invalid" => {
                "native_recovery_history_invalid"
            }
            "native_history_version_unsupported" => "native_recovery_history_version_unsupported",
            "native_history_item_count_mismatch" => "native_recovery_history_item_count_mismatch",
            "native_history_mismatch" => "native_recovery_history_mismatch",
            "native_history_tool_outputs_invalid" => "native_recovery_tool_output_mismatch",
            _ => "native_recovery_history_invalid",
        })
    })?;
    Ok(())
}
