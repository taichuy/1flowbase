use super::*;

pub(super) fn billing_invocation_id(
    flow_run_id: Uuid,
    node_id: Option<&str>,
    input: &ProviderInvocationInput,
) -> Uuid {
    if let Some(invocation_id) = input
        .trace_context
        .get("provider_invocation_id")
        .and_then(|value| Uuid::parse_str(value).ok())
    {
        return invocation_id;
    }
    let attempt = input
        .trace_context
        .get("provider_attempt_index")
        .map(String::as_str)
        .unwrap_or("0");
    let digest = Sha256::digest(format!(
        "{flow_run_id}:{}:{}:{}:{attempt}",
        node_id.unwrap_or("unknown"),
        input.provider_instance_id,
        input.model
    ));
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    Uuid::from_bytes(bytes)
}

pub(super) fn collected_provider_usage(
    events: &[ProviderStreamEvent],
    result: &plugin_framework::provider_contract::ProviderUsage,
) -> plugin_framework::provider_contract::ProviderUsage {
    fn add(target: &mut Option<u64>, value: Option<u64>) {
        if let Some(value) = value {
            *target = Some(target.unwrap_or_default().saturating_add(value));
        }
    }
    fn delta(
        target: &mut plugin_framework::provider_contract::ProviderUsage,
        value: &plugin_framework::provider_contract::ProviderUsage,
    ) {
        add(&mut target.input_tokens, value.input_tokens);
        add(
            &mut target.input_cache_hit_tokens,
            value.input_cache_hit_tokens,
        );
        add(
            &mut target.input_cache_miss_tokens,
            value.input_cache_miss_tokens,
        );
        add(&mut target.output_tokens, value.output_tokens);
        add(&mut target.reasoning_tokens, value.reasoning_tokens);
        add(&mut target.cache_read_tokens, value.cache_read_tokens);
        add(&mut target.cache_write_tokens, value.cache_write_tokens);
        if let Some(buckets) = &value.cache_write_by_ttl_seconds {
            let totals = target
                .cache_write_by_ttl_seconds
                .get_or_insert_with(Default::default);
            for (ttl, quantity) in buckets {
                let total = totals.entry(ttl.clone()).or_default();
                *total = total.saturating_add(*quantity);
            }
        }
        add(&mut target.total_tokens, value.total_tokens);
    }
    fn overlay(
        target: &mut plugin_framework::provider_contract::ProviderUsage,
        snapshot: &plugin_framework::provider_contract::ProviderUsage,
    ) {
        macro_rules! field {
            ($name:ident) => {
                if snapshot.$name.is_some() {
                    target.$name = snapshot.$name.clone();
                }
            };
        }
        field!(input_tokens);
        field!(input_cache_hit_tokens);
        field!(input_cache_miss_tokens);
        field!(output_tokens);
        field!(reasoning_tokens);
        field!(cache_read_tokens);
        field!(cache_write_tokens);
        if let Some(buckets) = &snapshot.cache_write_by_ttl_seconds {
            target
                .cache_write_by_ttl_seconds
                .get_or_insert_with(Default::default)
                .extend(buckets.clone());
        }
        field!(total_tokens);
    }
    let mut usage = plugin_framework::provider_contract::ProviderUsage::default();
    for event in events {
        match event {
            ProviderStreamEvent::UsageSnapshot { usage: snapshot } => overlay(&mut usage, snapshot),
            ProviderStreamEvent::UsageDelta { usage: value } => delta(&mut usage, value),
            _ => {}
        }
    }
    // The result is an absolute final snapshot, not another delta. Preserve fields
    // omitted by partial snapshots, including TTL buckets captured in the stream.
    overlay(&mut usage, result);
    usage
}

/// Fail-closed billing conflict for the narrow case where the provider produced
/// billable output but reported no usage. Carries the stream evidence so the
/// surfaced failure stays diagnosable.
pub(super) fn provider_usage_unavailable_conflict(
    output: &crate::ports::ProviderRuntimeInvocationOutput,
) -> plugin_framework::PluginFrameworkError {
    plugin_framework::PluginFrameworkError::runtime(
        ProviderRuntimeError::new(
            ProviderRuntimeErrorKind::ProviderInvalidResponse,
            "provider_usage_unavailable",
        )
        .with_provider_details(json!({
            "reason": "billable provider output arrived without provider-reported usage",
            "billable_output": true,
            "finish_reason": serde_json::to_value(&output.result.finish_reason)
                .unwrap_or(Value::Null),
            "stream_termination": output
                .result
                .provider_metadata
                .get("stream_termination")
                .cloned()
                .unwrap_or(Value::Null),
        })),
    )
}

pub(super) fn provider_tool_structure_receipt(tools: &[Value]) -> Value {
    Value::Array(
        tools
            .iter()
            .enumerate()
            .map(|(index, tool)| {
                let function = tool.get("function");
                json!({
                    "index": index,
                    "type": tool.get("type").and_then(Value::as_str),
                    "name": function
                        .and_then(|value| value.get("name"))
                        .or_else(|| tool.get("name"))
                        .and_then(Value::as_str),
                    "has_parameters": function
                        .and_then(|value| value.get("parameters"))
                        .or_else(|| tool.get("input_schema"))
                        .or_else(|| tool.get("inputSchema"))
                        .is_some(),
                })
            })
            .collect(),
    )
}
