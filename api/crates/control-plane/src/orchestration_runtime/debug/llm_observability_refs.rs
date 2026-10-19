use serde_json::{json, Value};

pub(super) struct LlmDebugObservabilityRefs {
    context_projection_ref: String,
    attempt_refs: Vec<String>,
    winner_attempt_ref: Option<String>,
    raw_response_ref: Option<String>,
}

impl LlmDebugObservabilityRefs {
    pub(super) fn from_records(
        projection: &domain::ContextProjectionRecord,
        attempts: &[domain::ModelFailoverAttemptLedgerRecord],
    ) -> Self {
        let winner_attempt = attempts
            .iter()
            .find(|attempt| attempt.status == "succeeded");

        Self {
            context_projection_ref: format!("runtime_context_projection:{}", projection.id),
            attempt_refs: attempts
                .iter()
                .map(|attempt| format!("model_failover_attempt:{}", attempt.id))
                .collect(),
            winner_attempt_ref: winner_attempt
                .map(|attempt| format!("model_failover_attempt:{}", attempt.id)),
            raw_response_ref: winner_attempt.and_then(|attempt| attempt.response_ref.clone()),
        }
    }
}

pub(super) fn apply_llm_debug_observability_refs(
    debug_payload: &mut Value,
    refs: &LlmDebugObservabilityRefs,
) {
    let Some(debug) = debug_payload.as_object_mut() else {
        return;
    };

    debug.insert(
        "context_projection_ref".to_string(),
        Value::String(refs.context_projection_ref.clone()),
    );
    debug.insert("attempt_refs".to_string(), json!(refs.attempt_refs));
    debug.insert(
        "winner_attempt_ref".to_string(),
        refs.winner_attempt_ref
            .as_ref()
            .map(|value| Value::String(value.clone()))
            .unwrap_or(Value::Null),
    );
    debug.insert(
        "raw_response_ref".to_string(),
        refs.raw_response_ref
            .as_ref()
            .map(|value| Value::String(value.clone()))
            .unwrap_or(Value::Null),
    );
}
