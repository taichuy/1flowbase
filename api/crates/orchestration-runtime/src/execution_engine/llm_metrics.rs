use super::*;

pub(crate) use super::llm_route_preflight::*;

const MAX_PROJECTION_LOCATORS: usize = 16;

pub(crate) fn bounded_generate_projection_receipt(
    receipt: &ProviderGenerateTranslationReceipt,
) -> Value {
    let provenance = receipt.provenance.as_ref().map(|provenance| {
        json!({
            "source": provenance.source,
            "preserved_count": provenance.preserved_block_count,
            "omitted_count": provenance.omitted_block_count,
            "locators_capped": provenance.capped,
            "preserved_blocks": provenance.preserved_blocks
                .iter()
                .take(MAX_PROJECTION_LOCATORS)
                .collect::<Vec<_>>(),
            "omitted_blocks": provenance.omitted_blocks
                .iter()
                .take(MAX_PROJECTION_LOCATORS)
                .collect::<Vec<_>>(),
        })
    });
    json!({
        "fidelity": receipt.fidelity,
        "loss_codes": &receipt.loss_codes,
        "error_code": receipt.error_code,
        "provenance": provenance,
    })
}

pub(super) struct AttemptMetricInput<'a> {
    pub(super) attempt_index: usize,
    pub(super) retry_reason: Option<&'a str>,
    pub(super) runtime: &'a CompiledLlmRuntime,
    pub(super) plugin_id: Option<&'a str>,
    pub(super) reasoning_effort: Option<&'a str>,
    pub(super) status: &'a str,
    pub(super) failed_after_first_token: bool,
    pub(super) error_payload: Option<&'a Value>,
    pub(super) generate_projection_receipt: Option<&'a ProviderGenerateTranslationReceipt>,
    pub(super) usage: &'a ProviderUsage,
    pub(super) event_count: usize,
    pub(super) started_at: OffsetDateTime,
    pub(super) first_token_at: Option<OffsetDateTime>,
    pub(super) finished_at: OffsetDateTime,
    pub(super) time_to_first_token_ms: Option<u64>,
}

pub(super) fn build_attempt_metric(input: AttemptMetricInput<'_>) -> Value {
    let mut attempt = json!({
        "attempt_index": input.attempt_index,
        "is_retry": input.attempt_index > 0,
        "retry_reason": input.retry_reason,
        "provider_instance_id": input.runtime.provider_instance_id,
        "provider_instance_display_name": input.runtime.provider_instance_display_name,
        "provider_code": input.runtime.provider_code,
        "plugin_id": input.plugin_id,
        "protocol": input.runtime.protocol,
        "upstream_model_id": input.runtime.model,
        "model": input.runtime.model,
        "reasoning_effort": input.reasoning_effort,
        "status": input.status,
        "failed_after_first_token": input.failed_after_first_token,
        "event_count": input.event_count,
        "started_at": offset_datetime_json(Some(input.started_at)),
        "first_token_at": offset_datetime_json(input.first_token_at),
        "finished_at": offset_datetime_json(Some(input.finished_at)),
        "time_to_first_token_ms": input.time_to_first_token_ms,
        "usage": serde_json::to_value(input.usage).unwrap_or(Value::Null),
        "error_code": input.error_payload
            .and_then(|payload| payload.get("error_code"))
            .cloned()
            .unwrap_or(Value::Null),
        "error_message_ref": input.error_payload
            .and_then(|payload| payload.get("message"))
            .and_then(Value::as_str)
            .map(|message| format!("runtime_artifact:inline:error:{message}"))
            .map(Value::String)
            .unwrap_or(Value::Null),
    });
    for key in [
        "1flowbase_provider_recovery_diagnostics",
        "1flowbase_provider_recovery",
    ] {
        if let Some(value) = input.error_payload.and_then(|payload| payload.get(key)) {
            // Already bounded by the provider-error projection owner.
            attempt[key] = value.clone();
        }
    }
    attach_generate_projection_receipt(&mut attempt, input.generate_projection_receipt);
    attempt
}

pub(crate) fn attach_generate_projection_receipt(
    attempt: &mut Value,
    receipt: Option<&ProviderGenerateTranslationReceipt>,
) {
    let Some(attempt) = attempt.as_object_mut() else {
        return;
    };
    attempt.insert(
        "provider_generate_projection".to_string(),
        receipt
            .map(bounded_generate_projection_receipt)
            .unwrap_or(Value::Null),
    );
}

pub(super) fn attach_distribution_selection_receipt(
    attempt: &mut Value,
    receipt: Option<&extension_contracts::ProviderDistributionSelectionReceipt>,
) {
    let Some(attempt) = attempt.as_object_mut() else {
        return;
    };
    attempt.insert(
        "provider_distribution_selection".to_string(),
        receipt
            .and_then(|receipt| serde_json::to_value(receipt).ok())
            .unwrap_or(Value::Null),
    );
}

pub(super) fn attach_provider_stream_timing_summary(attempt: &mut Value, summary: Option<&Value>) {
    let (Some(attempt), Some(summary)) = (attempt.as_object_mut(), summary) else {
        return;
    };
    attempt.insert(
        "provider_stream_timing_summary".to_string(),
        summary.clone(),
    );
}

pub(super) fn attach_provider_stream_timing(attempt: &mut Value, timing: Option<&Value>) {
    let (Some(attempt), Some(timing)) = (attempt.as_object_mut(), timing) else {
        return;
    };
    attempt.insert("provider_stream_timing".to_string(), timing.clone());
}

const RUNTIME_PROVIDER_STAGE_TIMING_METADATA_KEY: &str = "1flowbase_runtime_provider_stages";
const GATEWAY_PROVIDER_STAGE_TIMING_METADATA_KEY: &str = "1flowbase_gateway_provider_stages";

fn bounded_stage_value(value: Option<&Value>, field: &str) -> Option<u64> {
    value
        .filter(|receipt| receipt.get("schema_version").and_then(Value::as_u64) == Some(1))
        .and_then(|receipt| receipt.get(field))
        .and_then(Value::as_u64)
        .filter(|duration| *duration <= 24 * 60 * 60 * 1_000)
}

fn stage_receipt(owner: &str, duration_ms: Option<u64>, unavailable: &str) -> Value {
    json!({
        "owner": owner,
        "duration_ms": duration_ms,
        "availability": if duration_ms.is_some() { "observed" } else { "unavailable" },
        "unavailable_reason": duration_ms.is_none().then_some(unavailable),
    })
}

pub(super) fn attach_provider_timing_receipt(
    attempt: &mut Value,
    observability: &ProviderObservabilityMetadata,
    attempt_index: usize,
    attempt_status: &str,
    error_payload: Option<&Value>,
) {
    let Some(attempt) = attempt.as_object_mut() else {
        return;
    };
    let runtime = observability.runtime_stages.as_ref();
    let gateway = observability.gateway_stages.as_ref();
    let provider = observability.provider_timing.as_ref();
    let connection = observability.transport_session.as_ref();
    let connect_ms = provider.and_then(|receipt| receipt.connect_ms);
    let termination_kind = provider
        .map(|receipt| match receipt.termination_kind {
            extension_contracts::ProviderInvocationTerminationKind::Completed => "completed",
            extension_contracts::ProviderInvocationTerminationKind::UpstreamError => {
                "upstream_error"
            }
            extension_contracts::ProviderInvocationTerminationKind::TransportError => {
                "transport_error"
            }
            extension_contracts::ProviderInvocationTerminationKind::Deadline => "deadline",
        })
        .unwrap_or_else(|| {
            match error_payload
                .and_then(|payload| payload.get("error_code"))
                .and_then(Value::as_str)
            {
                Some("provider_transport_unavailable" | "endpoint_unreachable") => {
                    "transport_error"
                }
                Some("deadline_exceeded" | "provider_invocation_timeout") => "deadline",
                Some("empty_response") => "empty_response",
                Some(_) => "upstream_error",
                None if attempt_status == "succeeded" => "completed",
                None => attempt_status,
            }
        });
    let connect_unavailable = if connection.is_some_and(|receipt| receipt.reused) {
        "reused_connection_no_connect"
    } else {
        "provider_receipt_unavailable"
    };
    let connection = connection.map(|receipt| {
        json!({
            "cold": !receipt.reused,
            "reused": receipt.reused,
            "generation": receipt.generation,
            "connection_age_ms": receipt.connection_age_ms,
            "ttl_remaining_ms": receipt.ttl_remaining_ms,
            "physical_state": receipt.physical_state,
            "close_reason": receipt.close_reason,
            "close_acknowledged": receipt.close_acknowledged,
        })
    });
    attempt.insert(
        "provider_timing_receipt".to_string(),
        json!({
            "schema_version": 1,
            "attempt_index": attempt_index,
            "stages": {
                "ingress": stage_receipt("ai_native", bounded_stage_value(gateway, "ingress_ms"), "no_provider_event_ingress_observed"),
                "mapping": stage_receipt("runtime_host", bounded_stage_value(runtime, "mapping_ms"), "runtime_receipt_unavailable"),
                "flow": stage_receipt("ai_native", bounded_stage_value(gateway, "flow_ms"), "gateway_receipt_unavailable"),
                "queue": stage_receipt("runtime_host", bounded_stage_value(runtime, "queue_ms"), "runtime_receipt_unavailable"),
                "connect": stage_receipt("provider", connect_ms, connect_unavailable),
                "upstream": stage_receipt("provider", provider.map(|receipt| receipt.upstream_ms), "provider_receipt_unavailable"),
                "flush": stage_receipt("ai_native", bounded_stage_value(gateway, "flush_ms"), "no_stream_event_flush_observed"),
            },
            "connection": connection,
            "termination_kind": termination_kind,
        }),
    );
}

#[derive(Default)]
pub(super) struct ProviderObservabilityMetadata {
    pub(super) user_account: Option<Value>,
    pub(super) stream_timing: Option<Value>,
    pub(super) stream_timing_summary: Option<Value>,
    pub(super) billing: Option<Value>,
    pub(super) provider_timing: Option<extension_contracts::ProviderInvocationTimingReceipt>,
    pub(super) transport_session: Option<extension_contracts::ProviderTransportSessionReceipt>,
    pub(super) runtime_stages: Option<Value>,
    pub(super) gateway_stages: Option<Value>,
}

pub(super) fn attach_provider_billing(attempt: &mut Value, billing: Option<&Value>) {
    let (Some(attempt), Some(billing)) = (attempt.as_object_mut(), billing) else {
        return;
    };
    attempt.insert("billing".to_string(), billing.clone());
}

pub(super) fn take_provider_observability_metadata(
    result: &mut ProviderInvocationResult,
) -> ProviderObservabilityMetadata {
    let mut extracted = ProviderObservabilityMetadata::default();
    while let Some(metadata) = result.provider_metadata.as_object_mut() {
        // Only the explicitly tagged host envelope owns the new summary key.
        // Once consumed, a supplier's same-name keys remain supplier metadata.
        let is_summary_wrapper = extracted.stream_timing_summary.is_none()
            && metadata
                .get("_1flowbase_provider_observability_schema_version")
                .and_then(Value::as_u64)
                == Some(1)
            && metadata.contains_key("_1flowbase_runtime_stream_timing_summary");
        let is_wrapper = metadata.contains_key("_1flowbase_upstream_provider_metadata")
            && (metadata.contains_key(
                extension_contracts::PROVIDER_INVOCATION_TIMING_RECEIPT_METADATA_KEY,
            ) || metadata.contains_key(
                extension_contracts::PROVIDER_TRANSPORT_SESSION_RECEIPT_METADATA_KEY,
            ) || metadata.contains_key(RUNTIME_PROVIDER_STAGE_TIMING_METADATA_KEY)
                || metadata.contains_key(GATEWAY_PROVIDER_STAGE_TIMING_METADATA_KEY)
                || metadata.contains_key("_1flowbase_runtime_stream_timing")
                || is_summary_wrapper
                || metadata.contains_key("_1flowbase_billing")
                || metadata.contains_key("_1flowbase_user_account"));
        if let Some(value) =
            metadata.remove(extension_contracts::PROVIDER_INVOCATION_TIMING_RECEIPT_METADATA_KEY)
        {
            extracted.provider_timing = serde_json::from_value(value)
                .ok()
                .filter(
                    |receipt: &extension_contracts::ProviderInvocationTimingReceipt| {
                        receipt.validate().is_ok()
                    },
                )
                .or(extracted.provider_timing);
        }
        if let Some(value) =
            metadata.remove(extension_contracts::PROVIDER_TRANSPORT_SESSION_RECEIPT_METADATA_KEY)
        {
            extracted.transport_session = serde_json::from_value(value)
                .ok()
                .filter(
                    |receipt: &extension_contracts::ProviderTransportSessionReceipt| {
                        receipt.validate().is_ok()
                    },
                )
                .or(extracted.transport_session);
        }
        extracted.runtime_stages = metadata
            .remove(RUNTIME_PROVIDER_STAGE_TIMING_METADATA_KEY)
            .or(extracted.runtime_stages);
        extracted.gateway_stages = metadata
            .remove(GATEWAY_PROVIDER_STAGE_TIMING_METADATA_KEY)
            .or(extracted.gateway_stages);
        if !is_wrapper {
            break;
        }
        if is_summary_wrapper {
            extracted.stream_timing_summary =
                metadata.remove("_1flowbase_runtime_stream_timing_summary");
        }
        extracted.stream_timing = metadata
            .remove("_1flowbase_runtime_stream_timing")
            .or(extracted.stream_timing);
        extracted.billing = metadata.remove("_1flowbase_billing").or(extracted.billing);
        extracted.user_account = metadata
            .remove("_1flowbase_user_account")
            .or(extracted.user_account);
        let Some(upstream_metadata) = metadata.remove("_1flowbase_upstream_provider_metadata")
        else {
            break;
        };
        result.provider_metadata = upstream_metadata;
    }
    extracted
}

pub(super) fn build_llm_metrics_payload(
    runtime: &CompiledLlmRuntime,
    usage: ProviderUsage,
    finish_reason: Option<ProviderFinishReason>,
    event_count: usize,
    attempts: Vec<Value>,
    first_token_at: Option<OffsetDateTime>,
    time_to_first_token_ms: Option<u64>,
) -> Value {
    json!({
        "provider_instance_id": runtime.provider_instance_id,
        "provider_instance_display_name": runtime.provider_instance_display_name,
        "provider_code": runtime.provider_code,
        "protocol": runtime.protocol,
        "model": runtime.model,
        "event_count": event_count,
        "first_token_at": offset_datetime_json(first_token_at),
        "time_to_first_token_ms": time_to_first_token_ms,
        "route": build_llm_route_payload(runtime),
        "usage": serde_json::to_value(&usage).unwrap_or(Value::Null),
        "finish_reason": finish_reason
            .as_ref()
            .map(|reason| serde_json::to_value(reason).unwrap_or(Value::Null))
            .unwrap_or(Value::Null),
        "queue_snapshot_id": runtime
            .routing
            .as_ref()
            .and_then(|routing| routing.queue_snapshot_id.clone())
            .map(Value::String)
            .unwrap_or(Value::Null),
        "attempts": attempts,
    })
}

pub(super) fn offset_datetime_json(value: Option<OffsetDateTime>) -> Value {
    value
        .and_then(|datetime| datetime.format(&Rfc3339).ok())
        .map(Value::String)
        .unwrap_or(Value::Null)
}

pub(super) fn build_llm_route_payload(runtime: &CompiledLlmRuntime) -> Value {
    match runtime.routing.as_ref() {
        Some(routing) => json!({
            "routing_mode": routing.routing_mode,
            "fixed_model_target": routing.fixed_model_target,
            "queue_template_id": routing.queue_template_id,
            "distribution_rule": routing.distribution_rule,
            "provider_instance_id": runtime.provider_instance_id,
            "provider_code": runtime.provider_code,
            "upstream_model_id": runtime.model,
            "protocol": runtime.protocol,
        }),
        None => json!({
            "routing_mode": "fixed_model",
            "provider_instance_id": runtime.provider_instance_id,
            "provider_code": runtime.provider_code,
            "upstream_model_id": runtime.model,
            "protocol": runtime.protocol,
        }),
    }
}

#[cfg(test)]
mod provider_stream_timing_tests {
    use super::*;

    #[test]
    fn timing_receipt_contains_only_safe_event_facts() {
        let timing = json!([{
            "sequence": 1,
            "event_kind": "text_delta",
            "size_bytes": 42,
            "ingress_ms": 120,
            "runtime_append_ms": 121
        }]);
        let mut attempt = json!({ "attempt_index": 0 });

        attach_provider_stream_timing(&mut attempt, Some(&timing));

        assert_eq!(attempt["provider_stream_timing"], timing);
        assert!(!attempt.to_string().contains("sensitive answer text"));
    }

    #[test]
    fn timing_wrapper_restores_upstream_provider_metadata_exactly() {
        let upstream = json!({
            "response_id": "response-1",
            "_1flowbase_runtime_stream_timing": "upstream-owned-value"
        });
        let timing = json!([{
            "sequence": 1,
            "event_kind": "finish",
            "size_bytes": 32,
            "ingress_ms": 200,
            "runtime_append_ms": 201
        }]);
        let mut result = ProviderInvocationResult {
            provider_metadata: json!({
                "_1flowbase_runtime_stream_timing": timing,
                "_1flowbase_upstream_provider_metadata": upstream
            }),
            ..ProviderInvocationResult::default()
        };

        let receipt = take_provider_observability_metadata(&mut result).stream_timing;

        assert_eq!(receipt, Some(timing));
        assert_eq!(result.provider_metadata, upstream);
    }

    #[test]
    fn provider_observability_wrapper_extracts_billing_and_restores_upstream_metadata() {
        let upstream = json!({"response_id": "response-1"});
        let billing = json!({
            "pricing_provider_code": "openai",
            "pricing_model_id": "gpt-x",
            "total_cost": "0.00125",
            "currency_code": "USD"
        });
        let mut result = ProviderInvocationResult {
            provider_metadata: json!({
                "_1flowbase_billing": billing,
                "_1flowbase_user_account": "billing-user",
                "_1flowbase_upstream_provider_metadata": upstream
            }),
            ..ProviderInvocationResult::default()
        };

        let observability = take_provider_observability_metadata(&mut result);

        assert_eq!(observability.billing, Some(billing));
        assert_eq!(observability.user_account, Some(json!("billing-user")));
        assert_eq!(result.provider_metadata, upstream);
    }

    #[test]
    fn gateway_timing_wrapper_restores_non_object_upstream_metadata() {
        let timing = json!({
            "schema_version": 1,
            "ingress_ms": 37,
            "flow_ms": 11,
            "flush_ms": 3
        });
        let mut result = ProviderInvocationResult {
            provider_metadata: json!({
                "1flowbase_gateway_provider_stages": timing,
                "_1flowbase_upstream_provider_metadata": Value::Null
            }),
            ..ProviderInvocationResult::default()
        };

        let observability = take_provider_observability_metadata(&mut result);

        assert_eq!(observability.gateway_stages, Some(timing));
        assert_eq!(result.provider_metadata, Value::Null);
    }

    #[test]
    fn c2_clocked_receipts_project_safe_complete_stage_schema() {
        let mut result = ProviderInvocationResult {
            provider_metadata: json!({
                "1flowbase_gateway_provider_stages": {
                    "schema_version": 1,
                    "ingress_ms": 37,
                    "flow_ms": 11,
                    "flush_ms": 2
                },
                "1flowbase_runtime_provider_stages": {
                    "schema_version": 1,
                    "mapping_ms": 3,
                    "queue_ms": 5
                },
                "1flowbase_provider_invocation_timing": {
                    "schema_version": 1,
                    "connect_ms": 17,
                    "upstream_ms": 241,
                    "termination_kind": "completed"
                },
                "1flowbase_physical_transport_session": {
                    "generation": 7,
                    "reused": false,
                    "physical_state": "ready",
                    "connection_age_ms": 260,
                    "ttl_remaining_ms": 3_000,
                },
                "response_id": "must-remain-upstream-only"
            }),
            ..ProviderInvocationResult::default()
        };
        let observability = take_provider_observability_metadata(&mut result);
        let mut attempt = json!({"attempt_index": 2});

        attach_provider_timing_receipt(&mut attempt, &observability, 2, "succeeded", None);

        let receipt = &attempt["provider_timing_receipt"];
        assert_eq!(receipt["attempt_index"], 2);
        assert_eq!(receipt["stages"]["ingress"]["duration_ms"], 37);
        assert_eq!(receipt["stages"]["mapping"]["duration_ms"], 3);
        assert_eq!(receipt["stages"]["flow"]["duration_ms"], 11);
        assert_eq!(receipt["stages"]["queue"]["duration_ms"], 5);
        assert_eq!(receipt["stages"]["connect"]["duration_ms"], 17);
        assert_eq!(receipt["stages"]["upstream"]["duration_ms"], 241);
        assert_eq!(receipt["stages"]["flush"]["duration_ms"], 2);
        assert_eq!(receipt["connection"]["cold"], true);
        assert_eq!(receipt["connection"]["generation"], 7);
        assert_eq!(receipt["termination_kind"], "completed");
        let serialized = serde_json::to_string(receipt).unwrap();
        for forbidden in [
            "credential",
            "api_key",
            "prompt",
            "tool_output",
            "encrypted_content",
            "cursor",
            "response_id",
        ] {
            assert!(!serialized.contains(forbidden));
        }
        assert_eq!(
            result.provider_metadata["response_id"],
            "must-remain-upstream-only"
        );
    }

    #[test]
    fn c2_failed_attempt_derives_transport_termination_without_fabricating_stages() {
        let observability = ProviderObservabilityMetadata::default();
        let mut attempt = json!({"attempt_index": 1});

        attach_provider_timing_receipt(
            &mut attempt,
            &observability,
            1,
            "failed",
            Some(&json!({"error_code": "provider_transport_unavailable"})),
        );

        let receipt = &attempt["provider_timing_receipt"];
        assert_eq!(receipt["termination_kind"], "transport_error");
        assert_eq!(receipt["connection"], Value::Null);
        assert_eq!(receipt["stages"]["connect"]["duration_ms"], Value::Null);
        assert_eq!(receipt["stages"]["connect"]["availability"], "unavailable");
    }
}

#[cfg(test)]
#[path = "_tests/llm_route_preflight.rs"]
mod tests;

#[cfg(test)]
#[path = "_tests/provider_stream_timing_summary.rs"]
mod provider_stream_timing_summary_tests;
