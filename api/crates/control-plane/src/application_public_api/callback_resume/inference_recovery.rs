//! Native inference recovery is separate from the already consumed tool receipt.
use super::*;
use extension_contracts::provider_contract::{
    CommitLevel, ProviderRecoveryDirective, ProviderRecoveryReceipt, RecoveryDisposition,
    RecoveryReason,
};

/// Host-created, non-deserializable admission. Public request metadata cannot mint this grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeInferenceRecoveryGrant {
    pub(crate) callback_task_id: Uuid,
    pub(crate) failed_flow_run_id: Uuid,
    pub(crate) publication_version_id: Uuid,
    pub(crate) remaining_attempts: u32,
    pub(crate) absolute_deadline_unix_ms: i64,
    pub(crate) provider_instance_id: String,
}

pub(crate) fn recovery_key(callback_task_id: Uuid) -> String {
    format!("native-inference-recovery:{callback_task_id}")
}

impl NativeInferenceRecoveryGrant {
    pub(crate) fn durable_value(&self) -> Value {
        json!({"callback_task_id":self.callback_task_id,"failed_flow_run_id":self.failed_flow_run_id,
            "remaining_attempts":self.remaining_attempts,"absolute_deadline_unix_ms":self.absolute_deadline_unix_ms,
            "provider_instance_id":self.provider_instance_id})
    }
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
        || receipt.reason != RecoveryReason::TransportDisconnected
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
    Ok(NativeInferenceRecoveryGrant {
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
    if metadata["configuration_digest"].as_str() != Some(sealed.configuration_digest()?.as_str()) {
        return Err(reject("native_recovery_configuration_mismatch"));
    }
    let calls = callback.request_payload["tool_calls"]
        .as_array()
        .ok_or_else(|| reject("native_recovery_history_missing"))?;
    let ids = calls
        .iter()
        .map(|call| {
            call.get("call_id")
                .or_else(|| call.get("id"))
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| reject("native_recovery_history_missing"))
        })
        .collect::<Result<Vec<_>>>()?;
    super::super::compat::openai::history::validate_full_retry_input(
        &transport.wire_body()["input"],
        &metadata["history"],
        &ids,
    )
    .map_err(|_| reject("native_recovery_history_mismatch"))?;
    Ok(())
}
