use super::*;

#[test]
fn summary_only_projects_to_attempt_and_preserves_supplier_key_collisions() {
    let summary = json!({"schema_version": 1, "event_count": 0});
    let upstream = json!({
        "response_id": "response-1",
        "_1flowbase_runtime_stream_timing_summary": {"supplier": true},
        "_1flowbase_runtime_stream_timing": "supplier-detail"
    });
    let mut result = ProviderInvocationResult {
        provider_metadata: json!({
            "_1flowbase_provider_observability_schema_version": 1,
            "_1flowbase_runtime_stream_timing_summary": summary,
            "_1flowbase_upstream_provider_metadata": upstream
        }),
        ..Default::default()
    };

    let observability = take_provider_observability_metadata(&mut result);
    let mut attempt = json!({"attempt_index": 0});
    attach_provider_stream_timing_summary(
        &mut attempt,
        observability.stream_timing_summary.as_ref(),
    );
    attach_provider_stream_timing(&mut attempt, observability.stream_timing.as_ref());

    assert_eq!(observability.stream_timing_summary, Some(summary.clone()));
    assert_eq!(observability.stream_timing, None);
    assert_eq!(attempt["provider_stream_timing_summary"], summary);
    assert!(attempt.get("provider_stream_timing").is_none());
    assert_eq!(result.provider_metadata, upstream);
}

#[test]
fn detailed_capture_survives_nested_billing_account_and_recovery_metadata() {
    let summary = json!({"schema_version": 1, "event_count": 1, "capture_mode": "detailed"});
    let details = json!([{
        "sequence": 1, "event_kind": "finish", "size_bytes": 32,
        "ingress_ms": 200, "runtime_append_ms": 201
    }]);
    let upstream = json!({
        "response_id": "response-1",
        "native_recovery": {"checkpoint": "retained"},
        "_1flowbase_runtime_stream_timing_summary": "supplier-summary"
    });
    let billing = json!({"total_cost": "0.00125", "currency_code": "USD"});
    let stages = json!({"schema_version": 1, "flow_ms": 11, "ingress_ms": 200, "flush_ms": 1});
    let mut result = ProviderInvocationResult {
        provider_metadata: json!({
            "_1flowbase_provider_observability_schema_version": 1,
            "_1flowbase_runtime_stream_timing_summary": summary,
            "_1flowbase_runtime_stream_timing": details,
            "_1flowbase_upstream_provider_metadata": {
                "_1flowbase_user_account": "billing-user",
                "_1flowbase_upstream_provider_metadata": {
                    "_1flowbase_billing": billing,
                    "1flowbase_gateway_provider_stages": stages,
                    "_1flowbase_upstream_provider_metadata": upstream
                }
            }
        }),
        ..Default::default()
    };

    let observability = take_provider_observability_metadata(&mut result);
    let mut attempt = json!({"attempt_index": 1});
    attach_provider_stream_timing_summary(
        &mut attempt,
        observability.stream_timing_summary.as_ref(),
    );
    attach_provider_stream_timing(&mut attempt, observability.stream_timing.as_ref());

    assert_eq!(attempt["provider_stream_timing_summary"], summary);
    assert_eq!(attempt["provider_stream_timing"], details);
    assert_eq!(observability.billing, Some(billing));
    assert_eq!(observability.user_account, Some(json!("billing-user")));
    assert_eq!(observability.gateway_stages, Some(stages));
    assert_eq!(result.provider_metadata, upstream);
}

#[test]
fn supplier_summary_without_host_wrapper_is_not_consumed() {
    let upstream = json!({"_1flowbase_runtime_stream_timing_summary": "supplier-summary"});
    let mut result = ProviderInvocationResult {
        provider_metadata: upstream.clone(),
        ..Default::default()
    };
    let observability = take_provider_observability_metadata(&mut result);
    assert_eq!(observability.stream_timing_summary, None);
    assert_eq!(result.provider_metadata, upstream);
}

#[test]
fn supplier_summary_and_upstream_keys_are_preserved_together() {
    let upstream = json!({
        "_1flowbase_runtime_stream_timing_summary": {"schema_version": 1, "event_count": 7},
        "_1flowbase_upstream_provider_metadata": {"supplier_nested": true}
    });
    // Both raw supplier input and input inside a genuine host envelope must survive.
    for wrapped in [false, true] {
        let host_summary = json!({"schema_version": 1, "event_count": 0});
        let mut result = ProviderInvocationResult {
            provider_metadata: if wrapped {
                json!({
                    "_1flowbase_provider_observability_schema_version": 1,
                    "_1flowbase_runtime_stream_timing_summary": host_summary,
                    "_1flowbase_upstream_provider_metadata": upstream
                })
            } else {
                upstream.clone()
            },
            ..Default::default()
        };
        let observability = take_provider_observability_metadata(&mut result);
        assert_eq!(result.provider_metadata, upstream);
        assert_eq!(
            observability.stream_timing_summary,
            wrapped.then_some(host_summary)
        );
    }
}
