use super::*;

pub(super) fn round_cache_hit_rate(value: f64) -> Option<f64> {
    value
        .is_finite()
        .then(|| (value * 10_000.0).round() / 10_000.0)
}

pub(super) fn application_log_cache_hit_rate_for_response(record: &Value) -> Option<f64> {
    // Application log projections already include cached reads and writes in input_tokens.
    // Output tokens are unrelated to input cache hits; legacy total_tokens may omit caches.
    let input_tokens = record.get("input_tokens").and_then(Value::as_f64);
    let input_cache_hit_tokens = record.get("input_cache_hit_tokens").and_then(Value::as_f64);

    if let (Some(input_tokens), Some(input_cache_hit_tokens)) =
        (input_tokens, input_cache_hit_tokens)
    {
        if input_tokens > 0.0 && input_tokens.is_finite() && input_cache_hit_tokens.is_finite() {
            return round_cache_hit_rate(input_cache_hit_tokens / input_tokens);
        }
    }

    None
}

pub(super) fn normalize_application_log_cache_hit_rate(record: &mut Value) {
    let value = application_log_cache_hit_rate_for_response(record)
        .map_or(serde_json::Value::Null, serde_json::Value::from);

    if let Some(object) = record.as_object_mut() {
        object.insert("input_cache_hit_rate".to_string(), value);
    }
}

pub(super) fn normalize_application_run_observability(record: &mut Value) {
    if record.get("source_kind").and_then(Value::as_str) == Some("imported") {
        let principal = serde_json::json!({"kind":"application_api_key","id":record.get("api_key_id").cloned().unwrap_or(Value::Null),"display_name":record.get("api_key_name_snapshot").cloned().unwrap_or(Value::Null)});
        if let Some(object) = record.as_object_mut() {
            object.insert("execution_stage".into(), Value::String("collected".into()));
            object.insert(
                "invocation_source".into(),
                Value::String("client_collector".into()),
            );
            object.insert("principal".into(), principal);
        }
        return;
    }
    let Some(run_mode) = record
        .get("run_mode")
        .cloned()
        .and_then(|value| serde_json::from_value::<domain::FlowRunMode>(value).ok())
    else {
        return;
    };
    let created_by = record
        .get("created_by")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok());
    let api_key_id = record
        .get("api_key_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok());
    let authorized_account = record
        .get("authorized_account")
        .and_then(Value::as_str)
        .map(str::to_string);
    let context = run_mode.invocation_context(created_by, authorized_account, api_key_id);

    if let Some(object) = record.as_object_mut() {
        object.insert(
            "execution_stage".to_string(),
            Value::String(context.execution_stage.as_str().to_string()),
        );
        object.insert(
            "invocation_source".to_string(),
            Value::String(context.invocation_source.as_str().to_string()),
        );
        object.insert(
            "principal".to_string(),
            serde_json::to_value(context.principal)
                .expect("FlowRunPrincipal serialization must remain infallible"),
        );
    }
}

pub(super) fn is_application_run_log_model(model_code: &str) -> bool {
    matches!(
        model_code,
        "application_run_log_summaries" | "application_run_log_tasks"
    )
}

pub(super) fn runtime_record_response(model_code: &str, mut record: Value) -> Value {
    if is_application_run_log_model(model_code) {
        normalize_application_log_cache_hit_rate(&mut record);
        normalize_application_run_observability(&mut record);
    }

    record
}

pub(super) fn runtime_list_response(
    model_code: &str,
    items: Vec<Value>,
    total: i64,
) -> RuntimeListResponse {
    RuntimeListResponse {
        items: items
            .into_iter()
            .map(|record| runtime_record_response(model_code, record))
            .collect(),
        total,
    }
}

pub(super) fn apply_application_run_count_tokens_results(
    records: &mut [Value],
    results: &[control_plane::ports::ApplicationRunCountTokensResult],
) {
    let results = results
        .iter()
        .map(|result| (result.flow_run_id, result.input_tokens))
        .collect::<std::collections::HashMap<_, _>>();
    for record in records {
        let flow_run_id = record
            .get("flow_run_id")
            .or_else(|| record.get("id"))
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok());
        if let Some(object) = record.as_object_mut() {
            object.insert(
                "count_tokens_input_tokens".to_string(),
                flow_run_id
                    .and_then(|flow_run_id| results.get(&flow_run_id).copied())
                    .map_or(Value::Null, Value::from),
            );
        }
    }
}

pub(super) async fn enrich_application_run_count_tokens_results(
    adapter: &RuntimeModelOperationAdapter,
    model_code: &str,
    records: &mut [Value],
) -> Result<(), ApiError> {
    if !is_application_run_log_model(model_code) {
        return Ok(());
    }
    let flow_run_ids = records
        .iter()
        .filter_map(|record| {
            record
                .get("flow_run_id")
                .or_else(|| record.get("id"))
                .and_then(Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok())
        })
        .collect::<Vec<_>>();
    let results = adapter
        .store
        .list_application_run_count_tokens_results(&flow_run_ids)
        .await?;
    apply_application_run_count_tokens_results(records, &results);
    Ok(())
}
