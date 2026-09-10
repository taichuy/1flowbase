use super::*;
use crate::orchestration_runtime::test_support::InMemoryOrchestrationRuntimeRepository as Repository;
use plugin_framework::provider_contract::ProviderInvocationResult;

fn invoker(repository: Repository) -> RuntimeProviderInvoker<Repository, ()> {
    RuntimeProviderInvoker {
        repository,
        runtime: (),
        workspace_id: Uuid::nil(),
        provider_secret_master_key: String::new(),
        live_provider_events: None,
        runtime_event_stream: None,
        flow_run_id: Some(Uuid::now_v7()),
        active_node_id: None,
        active_node_run_id: None,
        api_node_id: None,
        provider_install_root: None,
        flow_execution_context: None,
        answer_presentation: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
        model_pricing_cache_store: None,
    }
}
async fn pricing(repository: &Repository) -> PricingRule {
    repository.enable_model_billing();
    let mut rule = repository
        .model_billing_match_pricing_rules("fixture", "model", OffsetDateTime::now_utc())
        .await
        .unwrap()
        .remove(0);
    rule.input_token_unit_price = "10".parse().unwrap();
    rule.output_token_unit_price = "50".parse().unwrap();
    rule.cache_hit_token_unit_price = "0.25".parse().unwrap();
    rule.cache_write_token_unit_size = 1000000;
    rule.cache_write_token_unit_price = "12.5".parse().unwrap();
    rule.rules = json!([
        {"when":{"cache_write_ttl_seconds":300},"overrides":{"cache_write_token_unit_price":"12.5"}},
        {"when":{"cache_write_ttl_seconds":3600},"overrides":{"cache_write_token_unit_price":"20"}}
    ]);
    rule
}
fn reservation(repository: Repository, rule: PricingRule) -> FeeReservation<Repository> {
    FeeReservation::new(
        repository,
        rule,
        CreditReservation {
            billing_session_id: Uuid::now_v7(),
            account_id: Uuid::now_v7(),
            reserved_amount: "1".into(),
            charge_skipped: false,
            charge_skip_reason: None,
        },
        Uuid::now_v7(),
        Uuid::now_v7(),
        OffsetDateTime::now_utc(),
    )
}

#[tokio::test]
async fn v2_normalizes_ordinary_read_and_write_without_double_counting() {
    let repository = Repository::with_permissions(vec![]);
    let rule = pricing(&repository).await;
    let usage = ProviderUsage {
        input_tokens: Some(100),
        input_cache_miss_tokens: Some(100),
        cache_read_tokens: Some(200),
        cache_write_tokens: Some(5000),
        cache_write_by_ttl_seconds: Some([("300".into(), 3000), ("3600".into(), 2000)].into()),
        ..Default::default()
    };
    let normalized = normalized_token_usage(&rule, &usage).unwrap();
    assert_eq!(normalized.input_tokens, 5300);
    assert_eq!(normalized.input_cache_hit_tokens, 200);
    assert_eq!(
        crate::billing::rate_token_usage(&rule, &normalized)
            .unwrap()
            .ordinary_input_tokens,
        100
    );
    let write_only = ProviderUsage {
        input_tokens: None,
        input_cache_miss_tokens: None,
        cache_read_tokens: None,
        ..usage.clone()
    };
    let rated = crate::billing::rate_token_usage(
        &rule,
        &normalized_token_usage(&rule, &write_only).unwrap(),
    )
    .unwrap();
    assert_eq!(rated.total_cost.to_string(), "0.0775");
    let overflowing = ProviderUsage {
        input_cache_miss_tokens: Some(i64::MAX as u64),
        ..usage.clone()
    };
    assert!(normalized_token_usage(&rule, &overflowing).is_err());
    let mut defaults = rule;
    defaults.rules = json!([]);
    let defaults = normalized_token_usage(&defaults, &usage).unwrap();
    assert_eq!(defaults.input_tokens, 5300);
    assert_eq!(defaults.input_cache_hit_tokens, 200);
}

#[test]
fn stream_deltas_and_partial_final_snapshot_preserve_ttl_without_double_counting() {
    let delta = ProviderUsage {
        cache_write_tokens: Some(3000),
        cache_write_by_ttl_seconds: Some([("300".into(), 3000)].into()),
        ..Default::default()
    };
    let events = vec![
        ProviderStreamEvent::UsageDelta {
            usage: delta.clone(),
        },
        ProviderStreamEvent::UsageSnapshot {
            usage: ProviderUsage {
                output_tokens: Some(2),
                ..Default::default()
            },
        },
        ProviderStreamEvent::UsageDelta {
            usage: ProviderUsage {
                cache_write_tokens: Some(2000),
                cache_write_by_ttl_seconds: Some([("3600".into(), 2000)].into()),
                ..Default::default()
            },
        },
    ];
    let usage = collected_provider_usage(
        &events,
        &ProviderUsage {
            cache_write_tokens: Some(5000),
            output_tokens: Some(2),
            ..Default::default()
        },
    );
    assert_eq!(usage.cache_write_tokens, Some(5000));
    assert_eq!(
        usage.cache_write_by_ttl_seconds,
        Some([("300".into(), 3000), ("3600".into(), 2000)].into())
    );
    assert_eq!(
        collected_provider_usage(
            &[ProviderStreamEvent::UsageDelta {
                usage: delta.clone()
            }],
            &delta
        ),
        delta
    );
}

#[tokio::test]
async fn required_fee_admission_vetoes_before_a_reservation_without_actor() {
    let repository = Repository::with_permissions(vec![]);
    repository.enable_model_billing();
    let invoker = invoker(repository.clone());
    let lifecycle = ProviderFeeLifecycle::new(&invoker);
    let input: ProviderInvocationInput = serde_json::from_value(json!({"contract_version":"1flowbase.provider/v2", "provider_instance_id":"fixture", "provider_code":"fixture", "protocol":"openai", "model":"model"})).unwrap();
    let result = lifecycle
        .dispatch(ProviderFeeEvent::BeforeInvocation {
            input: &input,
            pricing_provider_code: "fixture",
            pricing_model_id: "model",
            billing_node_id: None,
        })
        .await;
    assert!(result
        .err()
        .unwrap()
        .to_string()
        .contains("billing_actor_context_required"));
    assert_eq!(repository.model_billing_reserved_session_count(), 0);
}

#[tokio::test]
async fn required_fee_outcome_releases_invalid_usage_and_finalize_failure() {
    for (usage, expected_error, finalizations) in [
        (
            ProviderUsage {
                cache_write_tokens: Some(5000),
                ..Default::default()
            },
            "provider_usage_rating_failed",
            0,
        ),
        (
            ProviderUsage {
                input_tokens: Some(1),
                ..Default::default()
            },
            "billing_finalize_failed",
            1,
        ),
    ] {
        let repository = Repository::with_permissions(vec![]);
        let guard = reservation(repository.clone(), pricing(&repository).await);
        let invoker = invoker(repository.clone());
        let lifecycle = ProviderFeeLifecycle::new(&invoker);
        let mut output = Some(crate::ports::ProviderRuntimeInvocationOutput {
            events: vec![],
            result: ProviderInvocationResult {
                usage,
                ..Default::default()
            },
        });
        let mut error = None;
        let failure = lifecycle
            .dispatch(ProviderFeeEvent::AfterUsage {
                reservation: Some(guard),
                outcome: ProviderFeeOutcome {
                    upstream_model_id: "model",
                    provider_instance_id: Uuid::nil(),
                    actual_provider_code: "fixture",
                    invocation_output: &mut output,
                    invocation_error: &mut error,
                    canonical_stream_state: None,
                    native_responses_passthrough: false,
                },
            })
            .await
            .err()
            .unwrap();
        assert!(failure.to_string().contains(expected_error), "{failure}");
        assert_eq!(
            repository.model_billing_finalize_attempt_count(),
            finalizations
        );
        let releases = repository.model_billing_credit_releases();
        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0].1, expected_error);
        assert!(output
            .unwrap()
            .result
            .provider_metadata
            .get("_1flowbase_billing")
            .is_none());
    }
}

#[tokio::test]
async fn interrupted_fee_reservation_keeps_a_cleanup_owner() {
    let repository = Repository::with_permissions(vec![]);
    let guard = reservation(repository.clone(), pricing(&repository).await);
    let session = guard.reservation.billing_session_id;
    drop(guard);
    tokio::task::yield_now().await;
    assert_eq!(
        repository.model_billing_credit_releases(),
        vec![(session, "provider_billing_interrupted".into())]
    );
}

// AC3: each snapshot replaces only the TTL buckets it actually reports.
#[tokio::test]
async fn partial_ttl_snapshots_preserve_other_buckets_and_rate_mixed_writes() {
    let events = vec![
        ProviderStreamEvent::UsageSnapshot {
            usage: ProviderUsage {
                cache_write_tokens: Some(3000),
                cache_write_by_ttl_seconds: Some([("300".into(), 3000)].into()),
                ..Default::default()
            },
        },
        ProviderStreamEvent::UsageSnapshot {
            usage: ProviderUsage {
                cache_write_tokens: Some(5000),
                cache_write_by_ttl_seconds: Some([("3600".into(), 2000)].into()),
                ..Default::default()
            },
        },
    ];
    let usage = collected_provider_usage(
        &events,
        &ProviderUsage {
            cache_write_tokens: Some(5000),
            cache_write_by_ttl_seconds: Some([("3600".into(), 2000)].into()),
            ..Default::default()
        },
    );
    assert_eq!(usage.cache_write_tokens, Some(5000));
    assert_eq!(
        usage.cache_write_by_ttl_seconds,
        Some([("300".into(), 3000), ("3600".into(), 2000)].into())
    );
    let repository = Repository::with_permissions(vec![]);
    let rule = pricing(&repository).await;
    let rated =
        crate::billing::rate_token_usage(&rule, &normalized_token_usage(&rule, &usage).unwrap())
            .unwrap();
    assert_eq!(rated.total_cost.to_string(), "0.0775");
}
