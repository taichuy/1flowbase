use super::*;

#[test]
fn ac_002_current_provider_package_serializes_typed_input_without_projection() {
    let package = TempProviderPackage::new();
    let mut host = ProviderHost::default();
    let summary = host.load(package.path().to_str().unwrap()).unwrap();
    let mut input = invocation_input("fixture-model");
    input.system = vec![NativePromptBlock::text("current typed system")];
    input
        .model_parameters
        .insert("max_output_tokens".to_string(), json!(512));

    let loaded = host
        .loaded_package(&summary.plugin_id)
        .expect("current provider package should be loaded");
    let prepared = current_provider_wire_input(loaded, &input)
        .expect("current provider package should receive direct typed input");
    let wire_input = prepared.wire_value;

    assert_eq!(
        wire_input["contract_version"],
        json!("1flowbase.provider/v2")
    );
    assert_eq!(wire_input["system"][0]["type"], json!("text"));
    assert_eq!(
        wire_input["model_parameters"]["max_output_tokens"],
        json!(512)
    );
    assert!(wire_input["model_parameters"].get("max_tokens").is_none());
    assert!(
        wire_input.get("operation").is_none() && wire_input.get("profile").is_none(),
        "default Generate must retain the existing provider wire shape"
    );
}

#[tokio::test]
async fn wp_r1_generate_attaches_translation_receipt_to_provider_metadata() {
    let package = TempProviderPackage::new();
    package.write_generate_translation_runtime();
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    let mut input = invocation_input("fixture-model");
    input.system = vec![NativePromptBlock::Text {
        text: "system text".to_string(),
        cache_control: Some(NativePromptCacheControl {
            cache_type: NativePromptCacheControlType::Ephemeral,
            ttl: None,
        }),
    }];
    input.request_context = NativeModelRequestContext {
        end_user_reference: Some("external-user".to_string()),
    };
    input.client_protocol_envelope = Some(ProtocolContextEnvelope {
        source_protocol: "anthropic_messages".to_string(),
        query: BTreeMap::from([("preview".to_string(), vec!["one".to_string()])]),
        ..ProtocolContextEnvelope::default()
    });
    input
        .synchronize_required_capabilities()
        .expect("fixture input must use valid canonical message blocks");
    input
        .required_capabilities
        .insert(ProviderInvocationCapability::ProtocolContext);

    let output = host
        .invoke_stream(&plugin_id, input)
        .await
        .expect("optional Generate context should translate before provider spawn");

    assert_eq!(output.result.final_content.as_deref(), Some("translated"));
    assert_eq!(output.result.provider_metadata["provider"], "fixture");
    assert_eq!(
        output.result.provider_metadata[PROVIDER_GENERATE_TRANSLATION_RECEIPT_METADATA_KEY]
            ["decisions"],
        json!([
            "omitted_system_prompt_cache_control",
            "omitted_end_user_reference",
            "omitted_protocol_context_profile_mismatch"
        ])
    );
}

#[tokio::test]
async fn ac_002_legacy_provider_abi_is_rejected_before_spawn_side_effect() {
    let package = TempProviderPackage::new();
    let spawn_marker = package.path().join("spawn-side-effect-marker");
    package.write_spawn_side_effect_runtime(&spawn_marker);
    let mut host = ProviderHost::default();
    let summary = host.load(package.path().to_str().unwrap()).unwrap();
    host.loaded_packages
        .get_mut(&summary.plugin_id)
        .expect("loaded fixture should be mutable for the legacy ABI negative")
        .package
        .manifest
        .contract_version = "1flowbase.provider/v1".to_string();

    let error = host
        .invoke_stream(&summary.plugin_id, invocation_input("fixture-model"))
        .await
        .unwrap_err();

    assert!(error
        .to_string()
        .contains("unsupported provider package contract"));
    assert!(!spawn_marker.exists(), "legacy ABI must fail before spawn");
    assert!(host.active_stream_snapshot().await.streams.is_empty());
    assert!(lock_provider_worker_registry(&host.provider_workers)
        .expect("provider worker registry should be available")
        .workers
        .is_empty());
}

#[tokio::test]
async fn d1_p03_count_tokens_missing_capability_estimates_before_spawn_side_effect() {
    let package = TempProviderPackage::new();
    let spawn_marker = package.path().join("count-tokens-spawn-side-effect-marker");
    package.write_spawn_side_effect_runtime(&spawn_marker);
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;

    let output = host
        .count_tokens(&plugin_id, count_tokens_input("fixture-model"))
        .await
        .expect("missing optional capability must use the generic estimator");

    assert!(output.result.input_tokens > 0);
    assert_eq!(
        output.result.method,
        ProviderCountTokensMethod::GenericEstimate
    );
    assert_eq!(
        output.result.fallback_reason,
        Some(ProviderCountTokensFallbackReason::CapabilityUnavailable)
    );
    assert!(
        !spawn_marker.exists(),
        "missing CountTokens capability must fail before provider spawn"
    );
}

#[tokio::test]
async fn d1_p03_count_tokens_missing_plugin_returns_a_typed_generic_estimate() {
    let host = ProviderHost::default();

    let output = host
        .count_tokens("missing-plugin", count_tokens_input("fixture-model"))
        .await
        .expect("a missing CountTokens plugin must not escape the host boundary");

    assert!(output.result.input_tokens > 0);
    assert_eq!(
        output.result.method,
        ProviderCountTokensMethod::GenericEstimate
    );
    assert_eq!(
        output.result.fallback_reason,
        Some(ProviderCountTokensFallbackReason::PluginUnavailable)
    );
}

#[tokio::test]
async fn k2_compact_missing_capability_fails_before_spawn_side_effect() {
    let package = TempProviderPackage::new();
    let spawn_marker = package.path().join("compact-spawn-side-effect-marker");
    package.write_spawn_side_effect_runtime(&spawn_marker);
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;

    let error = host
        .compact(
            &plugin_id,
            compact_input("fixture-model", ProviderCompactProfile::ResponsesCompact),
        )
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        ProviderCompactError::Unsupported {
            profile: ProviderCompactProfile::ResponsesCompact,
            capabilities,
        } if capabilities == vec!["compact.responses_compact"]
    ));
    assert!(
        !spawn_marker.exists(),
        "missing Compact capability must fail before provider spawn"
    );
}

#[tokio::test]
async fn k2_compact_unclaimed_profile_capability_fails_before_spawn_side_effect() {
    let package = TempProviderPackage::new();
    package.declare_compact_capability("compact.responses_compact");
    let spawn_marker = package
        .path()
        .join("compact-unclaimed-spawn-side-effect-marker");
    package.write_spawn_side_effect_runtime(&spawn_marker);
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;

    let error = host
        .compact(
            &plugin_id,
            compact_input(
                "fixture-model",
                ProviderCompactProfile::ResponsesCompactionV2,
            ),
        )
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        ProviderCompactError::Unsupported {
            profile: ProviderCompactProfile::ResponsesCompactionV2,
            capabilities,
        } if capabilities == vec!["compact.responses_compaction_v2"]
    ));
    assert!(
        !spawn_marker.exists(),
        "a Compact profile must not claim another profile's manifest row"
    );
}

#[tokio::test]
async fn d1_p03_count_tokens_preserves_upstream_success_and_estimates_provider_failures() {
    let package = TempProviderPackage::new();
    package.declare_count_tokens_capability();
    package.write_count_tokens_response_runtime(
            r#"{"ok":false,"error":{"kind":"provider_upstream_error","message":"upstream CountTokens failed"}}"#,
        );
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;

    let upstream = host
        .count_tokens(&plugin_id, count_tokens_input("fixture-model"))
        .await
        .expect("upstream CountTokens failures must use the generic estimator");
    assert!(upstream.result.input_tokens > 0);
    assert_eq!(
        upstream.result.fallback_reason,
        Some(ProviderCountTokensFallbackReason::ProviderRuntimeFailure)
    );

    package.write_count_tokens_response_runtime(
        r#"{"ok":true,"result":{"operation":"count_tokens","input_tokens":37}}"#,
    );
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    let counted = host
        .count_tokens(&plugin_id, count_tokens_input("fixture-model"))
        .await
        .expect("tagged CountTokens result should project");
    assert_eq!(counted.result.input_tokens, 37);
    assert_eq!(
        counted.result.method,
        ProviderCountTokensMethod::UpstreamApi
    );

    package.write_count_tokens_response_runtime(
        r#"{"ok":true,"result":{"operation":"count_tokens","input_tokens":"not-a-number"}}"#,
    );
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    let malformed = host
        .count_tokens(&plugin_id, count_tokens_input("fixture-model"))
        .await
        .expect("malformed CountTokens results must use the generic estimator");
    assert!(malformed.result.input_tokens > 0);
    assert_eq!(
        malformed.result.fallback_reason,
        Some(ProviderCountTokensFallbackReason::MalformedProviderResult)
    );
}

#[tokio::test]
async fn active_invocation_lease_serializes_same_provider_pool() {
    let package = TempProviderPackage::new();
    package.write_slow_invoke_runtime();
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    let host = Arc::new(host);

    let first_host = Arc::clone(&host);
    let first_plugin_id = plugin_id.clone();
    let first = tokio::spawn(async move {
        first_host
            .invoke_stream(&first_plugin_id, invocation_input("fixture-model"))
            .await
            .unwrap()
    });
    wait_for_active_streams(&host, 1).await;

    let second_host = Arc::clone(&host);
    let second_plugin_id = plugin_id.clone();
    let second = tokio::spawn(async move {
        second_host
            .invoke_stream(&second_plugin_id, invocation_input("fixture-model"))
            .await
            .unwrap()
    });
    sleep(Duration::from_millis(20)).await;

    assert_eq!(host.active_stream_snapshot().await.streams.len(), 1);
    first.await.unwrap();
    second.await.unwrap();
    assert!(host.active_stream_snapshot().await.streams.is_empty());
}

#[tokio::test]
async fn invoke_stream_uses_default_provider_invocation_budget() {
    let package = TempProviderPackage::new();
    package.write_provider_package_with_runtime_timeout("Fixture Provider", 1);
    package.write_slow_invoke_runtime();
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;

    let output = host
        .invoke_stream(&plugin_id, invocation_input("fixture-model"))
        .await
        .expect("provider invocation should not inherit the short runtime command timeout");

    assert_eq!(output.result.final_content.as_deref(), Some("done"));
}

#[tokio::test]
async fn active_invocation_lease_allows_different_provider_pools() {
    let package = TempProviderPackage::new();
    package.write_slow_invoke_runtime();
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    let host = Arc::new(host);

    let first_host = Arc::clone(&host);
    let first_plugin_id = plugin_id.clone();
    let first = tokio::spawn(async move {
        first_host
            .invoke_stream(&first_plugin_id, invocation_input("fixture-model-a"))
            .await
            .unwrap()
    });
    let second_host = Arc::clone(&host);
    let second_plugin_id = plugin_id.clone();
    let second = tokio::spawn(async move {
        second_host
            .invoke_stream(&second_plugin_id, invocation_input("fixture-model-b"))
            .await
            .unwrap()
    });

    wait_for_active_streams(&host, 2).await;
    first.await.unwrap();
    second.await.unwrap();
}

#[tokio::test]
async fn stateful_worker_registry_does_not_serialize_different_plugins() {
    let first_package = TempProviderPackage::new();
    first_package.write_stateful_provider_package(
        "fixture_provider_a",
        "fixture_provider_a",
        "Fixture Provider A",
    );
    first_package.write_slow_worker_runtime("first done");
    let second_package = TempProviderPackage::new();
    second_package.write_stateful_provider_package(
        "fixture_provider_b",
        "fixture_provider_b",
        "Fixture Provider B",
    );
    second_package.write_slow_worker_runtime("second done");
    let mut host = ProviderHost::default();
    let first_plugin_id = host
        .load(first_package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    let second_plugin_id = host
        .load(second_package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    let host = Arc::new(host);

    let first_host = Arc::clone(&host);
    let first = tokio::spawn(async move {
        first_host
            .invoke_stream(&first_plugin_id, invocation_input("fixture-model-a"))
            .await
            .unwrap()
    });
    wait_for_active_streams(&host, 1).await;

    let second_host = Arc::clone(&host);
    let second = tokio::spawn(async move {
        second_host
            .invoke_stream(&second_plugin_id, invocation_input("fixture-model-b"))
            .await
            .unwrap()
    });

    tokio::time::timeout(Duration::from_millis(320), async {
        let first = first.await.unwrap();
        let second = second.await.unwrap();
        assert_eq!(first.result.final_content.as_deref(), Some("first done"));
        assert_eq!(second.result.final_content.as_deref(), Some("second done"));
    })
    .await
    .expect("different stateful provider workers should not be serialized by the registry lock");
}

#[tokio::test]
async fn failed_stateful_worker_is_replaced_on_next_handle_acquisition() {
    let package = TempProviderPackage::new();
    package.write_stateful_provider_package(
        "fixture_provider",
        "fixture_provider",
        "Fixture Provider",
    );
    package.write_lifecycle_worker_runtime();
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;

    host.validate(&plugin_id, json!({ "mode": "normal" }))
        .await
        .unwrap();
    let original = host.provider_worker_snapshot(&plugin_id).unwrap().unwrap();
    assert!(host
        .validate(&plugin_id, json!({ "mode": "crash" }))
        .await
        .is_err());
    shared_workers::wait_reaped(&host, &plugin_id).await;
    let failed_receipt = host
        .provider_worker_cleanup_receipt(&plugin_id)
        .unwrap()
        .unwrap();
    assert_eq!(failed_receipt.generation, original.generation);
    assert_eq!(failed_receipt.prior_pid, original.pid);
    assert!(failed_receipt.exited);

    let output = host
        .validate(&plugin_id, json!({ "mode": "normal" }))
        .await
        .expect("the request after a crash should activate a replacement generation");
    let replacement = host.provider_worker_snapshot(&plugin_id).unwrap().unwrap();
    let retained_receipt = host
        .provider_worker_cleanup_receipt(&plugin_id)
        .unwrap()
        .unwrap();

    assert_eq!(replacement.state, ProviderWorkerLifecycleState::Active);
    assert_eq!(replacement.generation, 2);
    assert_ne!(replacement.pid, original.pid);
    assert_eq!(output.output["pid"], json!(replacement.pid.unwrap()));
    assert_eq!(retained_receipt, failed_receipt);
}

#[tokio::test]
async fn transport_session_control_uses_stateful_stdio_and_returns_safe_receipt() {
    let package = TempProviderPackage::new();
    package.write_stateful_provider_package(
        "fixture_provider",
        "fixture_provider",
        "Fixture Provider",
    );
    package.write_lifecycle_worker_runtime();
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;

    let mut input = invocation_input("fixture-model");
    input
        .set_transport_session_directive(ProviderTransportSessionDirective {
            worker_incarnation: None,
            logical_session_id: "logical-fixture".into(),
            generation: 7,
            task_id: "first".into(),
            state: ProviderLogicalSessionState::Active,
            physical_deadline_unix_ms: 4_102_444_800_000,
        })
        .unwrap();
    host.invoke_stream(&plugin_id, input).await.unwrap();

    let receipt = host
        .transport_session_operation(
            &plugin_id,
            ProviderTransportSessionCommand {
                worker_incarnation: None,
                logical_session_id: "logical-fixture".into(),
                generation: 7,
                action: ProviderTransportSessionAction::Drain,
                deadline_unix_ms: 4_102_444_800_000,
            },
        )
        .unwrap()
        .await
        .unwrap();

    assert_eq!(receipt.generation, 7);
    assert_eq!(receipt.connection_age_ms, 42);
    assert_eq!(receipt.close_acknowledged, Some(true));
    assert!(!serde_json::to_string(&receipt)
        .unwrap()
        .contains("logical-fixture"));
}

#[test]
fn every_stateful_runtime_dispatch_uses_the_supervisor_admission_gate() {
    let source = include_str!("../../provider_host.rs");

    assert_eq!(
        source
            .matches("PluginExecutionMode::StatefulProviderWorker")
            .count(),
        4,
        "unary, streaming selection/dispatch, and transport control are explicit boundaries"
    );
    assert_eq!(source.matches(".call_admitted(").count(), 2);
    assert_eq!(
        source
            .matches("session_workers::acquire_shared_invocation")
            .count(),
        2,
        "multiplex unary and streaming dispatch both acquire supervisor admission"
    );
    let compact_source = source.split_whitespace().collect::<String>();
    assert_eq!(source.matches(".call_streaming_admitted(").count(), 2);
    assert!(compact_source.contains("Box::new((permit,reserved"));
    assert!(compact_source.contains("Box::new((reserved,_lease,active_stream_lease))"));
    assert!(!source.contains("let mut worker = worker.lock().await"));
    assert!(source.contains("call_executable("));
    assert!(source.contains("call_executable_streaming("));
}
