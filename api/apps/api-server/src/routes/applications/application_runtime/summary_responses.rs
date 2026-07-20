fn format_time(value: time::OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .unwrap_or_else(|_| value.to_string())
}

fn format_optional_time(value: Option<time::OffsetDateTime>) -> Option<String> {
    value.map(format_time)
}

fn to_flow_run_summary_response(
    application: &domain::ApplicationRecord,
    summary: domain::ApplicationRunSummary,
    statistics: application_logs::ApplicationRunStatisticsResponse,
) -> FlowRunSummaryResponse {
    let application_type = application.application_type.as_str().to_string();
    let run_object_kind = application.sections.logs.run_object_kind.clone();
    let subject = application_logs::ApplicationRunSubjectResponse {
        kind: application_type.clone(),
        id: Some(application.id.to_string()),
        draft_id: None,
        target_node_id: summary.target_node_id.clone(),
    };
    let invocation_context = summary.run_mode.invocation_context(
        summary.created_by,
        summary.authorized_account.clone(),
        summary.api_key_id,
    );
    let correlation = application_logs::ApplicationRunCorrelationResponse {
        api_key_id: summary.api_key_id.map(|value| value.to_string()),
        publication_version_id: summary
            .publication_version_id
            .map(|value| value.to_string()),
        external_user: summary.user_id.clone(),
        external_conversation_id: summary.external_conversation_id.clone(),
        external_trace_id: summary.external_trace_id.clone(),
        compatibility_mode: summary.compatibility_mode.clone(),
        idempotency_key: summary.idempotency_key.clone(),
    };

    FlowRunSummaryResponse {
        id: summary.id.to_string(),
        application_id: application.id.to_string(),
        application_type,
        run_object_kind,
        run_kind: summary.run_mode.as_str().to_string(),
        run_mode: summary.run_mode.as_str().to_string(),
        status: summary.status.as_str().to_string(),
        target_node_id: summary.target_node_id,
        title: summary.title,
        expand_id: summary.user_id,
        authorized_account: summary.authorized_account,
        execution_stage: invocation_context.execution_stage.as_str().to_string(),
        invocation_source: invocation_context.invocation_source.as_str().to_string(),
        compatibility_mode: summary.compatibility_mode,
        subject,
        principal: application_logs::principal_response(invocation_context.principal),
        correlation,
        statistics,
        started_at: format_time(summary.started_at),
        finished_at: format_optional_time(summary.finished_at),
        created_at: format_time(summary.created_at),
        updated_at: format_time(summary.updated_at),
    }
}

fn usage_token_value(value: &serde_json::Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
}

fn usage_total_tokens(usage: &serde_json::Value) -> Option<i64> {
    if let Some(total_tokens) = usage.get("total_tokens").and_then(usage_token_value) {
        return Some(total_tokens);
    }

    let segments = ["input_tokens", "output_tokens", "reasoning_tokens"];
    let mut total = 0_i64;
    let mut has_segment = false;

    for segment in segments {
        if let Some(tokens) = usage.get(segment).and_then(usage_token_value) {
            total += tokens;
            has_segment = true;
        }
    }

    has_segment.then_some(total)
}

fn metrics_payload_total_tokens(metrics_payload: &serde_json::Value) -> Option<i64> {
    metrics_payload.get("usage").and_then(usage_total_tokens)
}

fn metrics_payload_usage_token(
    metrics_payload: &serde_json::Value,
    usage_field: &str,
) -> Option<i64> {
    metrics_payload
        .get("usage")
        .and_then(|usage| usage.get(usage_field))
        .and_then(usage_token_value)
}

fn metrics_payload_cache_hit_tokens(metrics_payload: &serde_json::Value) -> Option<i64> {
    metrics_payload_usage_token(metrics_payload, "input_cache_hit_tokens")
        .or_else(|| metrics_payload_usage_token(metrics_payload, "cache_read_tokens"))
        .or_else(|| metrics_payload_usage_token(metrics_payload, "cached_input_tokens"))
}

fn callback_task_tool_callback_count(task: &domain::CallbackTaskRecord) -> i64 {
    if task.callback_kind != "llm_tool_calls" {
        return 0;
    }

    task.request_payload
        .get("tool_calls")
        .and_then(serde_json::Value::as_array)
        .map(|tool_calls| tool_calls.len() as i64)
        .or_else(|| {
            task.request_payload
                .get("tool_calls")
                .and_then(|value| value.get("tool_call_count"))
                .and_then(serde_json::Value::as_i64)
        })
        .unwrap_or(0)
}

fn application_run_tool_callback_count(detail: &domain::ApplicationRunDetail) -> i64 {
    let debug_payloads = detail
        .node_runs
        .iter()
        .map(|node_run| node_run.debug_payload.clone())
        .collect::<Vec<_>>();
    let indexed_count =
        count_llm_tool_callback_trace_items(&debug_payloads, &detail.callback_tasks) as i64;
    let task_count = detail
        .callback_tasks
        .iter()
        .map(callback_task_tool_callback_count)
        .sum();

    indexed_count.max(task_count)
}

fn application_run_statistics(
    detail: &domain::ApplicationRunDetail,
) -> application_logs::ApplicationRunStatisticsResponse {
    let mut unique_node_ids = HashSet::new();
    let mut total_tokens = None;
    let mut input_tokens = None;
    let mut output_tokens = None;
    let mut input_cache_hit_tokens = None;

    for node_run in &detail.node_runs {
        unique_node_ids.insert(node_run.node_id.as_str());

        if let Some(node_tokens) = metrics_payload_total_tokens(&node_run.metrics_payload) {
            total_tokens = Some(total_tokens.unwrap_or(0) + node_tokens);
        }
        if let Some(node_tokens) =
            metrics_payload_usage_token(&node_run.metrics_payload, "input_tokens")
        {
            input_tokens = Some(input_tokens.unwrap_or(0) + node_tokens);
        }
        if let Some(node_tokens) =
            metrics_payload_usage_token(&node_run.metrics_payload, "output_tokens")
        {
            output_tokens = Some(output_tokens.unwrap_or(0) + node_tokens);
        }
        if let Some(node_tokens) = metrics_payload_cache_hit_tokens(&node_run.metrics_payload) {
            input_cache_hit_tokens = Some(input_cache_hit_tokens.unwrap_or(0) + node_tokens);
        }
    }

    application_logs::ApplicationRunStatisticsResponse {
        total_tokens,
        input_tokens,
        output_tokens,
        input_cache_hit_tokens,
        input_cache_hit_rate: application_logs::input_cache_hit_rate_for_response(
            total_tokens,
            input_cache_hit_tokens,
        ),
        unique_node_count: unique_node_ids.len() as i64,
        tool_callback_count: application_run_tool_callback_count(detail),
    }
}

fn application_runs_created_after(query: &ApplicationRunsQuery) -> Option<OffsetDateTime> {
    let days = query
        .time_range_days
        .filter(|days| *days > 0)
        .unwrap_or(APPLICATION_RUN_LOG_DEFAULT_TIME_RANGE_DAYS);

    Some(OffsetDateTime::now_utc() - Duration::days(days))
}

fn normalize_application_run_sort_by(input: Option<&str>) -> &'static str {
    match input.unwrap_or("created_at") {
        "created_at" => "created_at",
        "started_at" => "started_at",
        "finished_at" => "finished_at",
        "updated_at" => "updated_at",
        _ => "created_at",
    }
}

fn normalize_application_run_sort_order(input: Option<&str>) -> &'static str {
    match input.unwrap_or("desc").to_ascii_lowercase().as_str() {
        "asc" => "asc",
        _ => "desc",
    }
}

fn should_refresh_application_run_logs(cache_mode: Option<&str>) -> bool {
    matches!(cache_mode, Some("refresh"))
}

fn to_flow_run_response(run: domain::FlowRunRecord) -> FlowRunResponse {
    FlowRunResponse {
        id: run.id.to_string(),
        application_id: run.application_id.to_string(),
        flow_id: run.flow_id.to_string(),
        draft_id: run.draft_id.to_string(),
        compiled_plan_id: run.compiled_plan_id.map(|value| value.to_string()),
        run_mode: run.run_mode.as_str().to_string(),
        status: run.status.as_str().to_string(),
        target_node_id: run.target_node_id,
        title: run.title,
        expand_id: run.external_user,
        authorized_account: run.authorized_account,
        external_conversation_id: run.external_conversation_id,
        query: application_run_query(&run.input_payload),
        model: application_run_model(&run.input_payload),
        input_payload: run.input_payload,
        output_payload: run.output_payload,
        error_payload: run.error_payload,
        created_by: run.created_by.to_string(),
        started_at: format_time(run.started_at),
        finished_at: format_optional_time(run.finished_at),
        created_at: format_time(run.created_at),
        updated_at: format_time(run.updated_at),
    }
}
