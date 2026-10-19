use super::*;

#[tokio::test]
async fn root_1805_model_provider_consumes_lease_free_egress_handoff_through_proxy() {
    let (proxy_url, proxy_request) = spawn_one_request_proxy();
    let proxy_target = "http://model-provider-origin.invalid/generate";
    let package = TempProviderPackage::new();
    write_network_egress_handoff_provider_package(&package, proxy_target);
    let runtime = ApiProviderRuntime::new(Arc::new(
        ApiRuntimeServices::new_without_model_provider_extension_graph_for_tests(),
    ));
    let mut input = ProviderInvocationInput {
        provider_instance_id: "provider-1".to_string(),
        provider_code: "fixture_provider".to_string(),
        protocol: "openai_compatible".to_string(),
        model: "fixture_chat".to_string(),
        provider_config: json!({ "api_key": "provider-secret" }),
        ..ProviderInvocationInput::default()
    };
    input.set_network_egress_context(ProviderNetworkEgressContext {
        mode: ProviderNetworkEgressMode::RequiredHttpProxy,
        http_proxy_url: proxy_url,
        expires_at: "4102444800".to_string(),
        required: true,
    });

    let output = runtime
        .invoke_stream(&fixture_installation(&package), input)
        .await
        .expect("the capability-declaring Provider must receive the host handoff");

    assert_eq!(output.result.final_content.as_deref(), Some("proxied"));
    let request = proxy_request
        .join()
        .expect("proxy fixture thread must finish");
    assert!(
        request.starts_with("GET http://model-provider-origin.invalid/generate HTTP/1.1"),
        "the Provider must send its request through the fake proxy: {request}"
    );
    assert!(!request.contains("provider-secret"));
}

fn root_1805_http_node(target: &str) -> orchestration_runtime::compiled_plan::CompiledNode {
    orchestration_runtime::compiled_plan::CompiledNode {
        node_id: "network-egress-http".to_string(),
        node_type: "http_request".to_string(),
        alias: "Network Egress HTTP".to_string(),
        container_id: None,
        dependency_node_ids: Vec::new(),
        downstream_node_ids: Vec::new(),
        bindings: BTreeMap::new(),
        outputs: Vec::new(),
        config: json!({
            "method": "GET",
            "url": target,
            "body_type": "none",
            "timeout_ms": 5_000,
            "verify_ssl": true,
        }),
        plugin_runtime: None,
        llm_runtime: None,
        code_runtime: None,
    }
}

/// Root #1805 AC-010 / AC-014: this is one continuous chain rather than a handoff unit test. It
/// persists the exact model-instance Route -> Pool -> Provider projection, starts the egress
/// worker, acquires its lease through the real resolver, and then invokes the model worker.
#[tokio::test]
async fn root_1805_model_provider_route_pool_provider_lease_reaches_fake_proxy_without_host_secrets(
) {
    let (proxy_url, proxy_request) = spawn_routed_request_proxy();
    let proxy_target = "http://model-provider-origin.invalid/route-pool-provider";
    let package = TempProviderPackage::new();
    let wire_capture = package.path().join("provider-wire.json");
    write_network_egress_handoff_provider_package_with_wire_capture(
        &package,
        proxy_target,
        Some(&wire_capture),
    );
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let instance_id = Uuid::now_v7();
    let base_runtime = ApiProviderRuntime::new(network_egress_runtime_services());
    let (resolver, _egress_package) =
        crate::_tests::official_extension_catalog_source::seed_network_egress_resolver(
            &state,
            &proxy_url,
            domain::NetworkEgressConsumerSelector::ModelProviderInstance { instance_id },
            base_runtime.clone(),
        )
        .await;
    let runtime = base_runtime.with_network_egress(Arc::new(resolver));
    let output = ProviderRuntimePort::invoke_stream_with_network_egress(
        &runtime,
        &fixture_installation(&package),
        ProviderInvocationInput {
            provider_instance_id: instance_id.to_string(),
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            model: "fixture_chat".to_string(),
            provider_config: json!({"api_key": "model-provider-secret"}),
            ..ProviderInvocationInput::default()
        },
        None,
        state.bootstrap_workspace_id,
        domain::NetworkEgressConsumerSelector::ModelProviderInstance { instance_id },
    )
    .await
    .expect("the persisted model route must reach the fake proxy through the Host resolver");

    assert_eq!(output.result.final_content.as_deref(), Some("proxied"));
    let proxy_request = proxy_request
        .join()
        .expect("fake proxy must receive traffic");
    assert!(
        proxy_request.starts_with(
            "GET http://model-provider-origin.invalid/route-pool-provider HTTP/1.1"
        ),
        "the model worker must reach the non-routable origin through the fake proxy: {proxy_request}"
    );
    let wire = fs::read_to_string(&wire_capture).expect("model worker must record its input wire");
    assert!(wire.contains("network_egress"));
    assert!(!wire.contains("cleanup_token"));
    assert!(!wire.contains("host-private"));
    assert!(!wire.contains("fixture-lease"));
    assert!(!wire.contains("egress-provider-secret"));
    assert!(!wire.contains("fixture_egress"));
}

/// Root #1805 AC-013: a real resolver with no matching model route retains direct behavior.
#[tokio::test]
async fn root_1805_model_provider_no_route_keeps_direct_behavior_without_handoff() {
    let (origin, origin_request) = spawn_one_request_proxy();
    let package = TempProviderPackage::new();
    let wire_capture = package.path().join("provider-wire.json");
    write_network_egress_handoff_provider_package_with_wire_capture(
        &package,
        &format!("{origin}/model-direct"),
        Some(&wire_capture),
    );
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let base_runtime = ApiProviderRuntime::new(network_egress_runtime_services());
    let resolver = crate::network_egress_client::NetworkEgressHttpClientResolver::new(
        state.store.clone(),
        base_runtime.clone(),
        state.provider_secret_master_key.clone(),
        state.api_node_id.clone(),
    );
    let runtime = base_runtime.with_network_egress(Arc::new(resolver));
    let instance_id = Uuid::now_v7();
    let output = ProviderRuntimePort::invoke_stream_with_network_egress(
        &runtime,
        &fixture_installation(&package),
        ProviderInvocationInput {
            provider_instance_id: instance_id.to_string(),
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            model: "fixture_chat".to_string(),
            provider_config: json!({"api_key": "model-provider-secret"}),
            ..ProviderInvocationInput::default()
        },
        None,
        state.bootstrap_workspace_id,
        domain::NetworkEgressConsumerSelector::ModelProviderInstance { instance_id },
    )
    .await
    .expect("no model route must retain ordinary provider invocation behavior");

    assert_eq!(output.result.final_content.as_deref(), Some("proxied"));
    assert!(origin_request
        .join()
        .expect("direct origin must receive the request")
        .starts_with("GET /model-direct HTTP/1.1"));
    let wire = fs::read_to_string(&wire_capture).expect("model worker must record its input wire");
    assert!(!wire.contains("network_egress"));
    assert!(!wire.contains("http_proxy_url"));
    assert!(!wire.contains("cleanup_token"));
    assert!(!wire.contains("egress-provider-secret"));
}

/// Root #1805 AC-011 / AC-014: the node asks its invoker for a lease at execution time. The
/// invoker calls `ApiProviderRuntime::acquire_http_node_client`, reaching the persisted Route ->
/// Pool -> Provider -> Host lease path before HTTP execution.
#[tokio::test]
async fn root_1805_http_node_route_pool_provider_lease_reaches_fake_proxy_without_host_secrets() {
    let (proxy_url, proxy_request) = spawn_routed_request_proxy();
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let base_runtime = ApiProviderRuntime::new(network_egress_runtime_services());
    let (resolver, _egress_package) =
        crate::_tests::official_extension_catalog_source::seed_network_egress_resolver(
            &state,
            &proxy_url,
            domain::NetworkEgressConsumerSelector::HttpNodeDefault,
            base_runtime.clone(),
        )
        .await;
    let invoker = ResolverBackedHttpNodeInvoker {
        runtime: base_runtime.with_network_egress(Arc::new(resolver)),
        workspace_id: state.bootstrap_workspace_id,
    };
    let execution =
        orchestration_runtime::execution_engine::execute_http_request_node_with_provider_invoker(
            &root_1805_http_node("http://http-node-origin.invalid/route-pool-provider"),
            &Map::new(),
            &Map::new(),
            None,
            Some(&invoker),
        )
        .await
        .expect("HTTP execution must preserve its normal output envelope");

    assert_eq!(execution.error_payload, None);
    assert_eq!(execution.output_payload["status_code"], json!(200));
    let observable = serde_json::to_string(&json!({
        "output_payload": execution.output_payload,
        "error_payload": execution.error_payload,
        "metrics_payload": execution.metrics_payload,
        "debug_payload": execution.debug_payload,
    }))
    .expect("HTTP output envelope must serialize");
    assert!(!observable.contains("cleanup_token"));
    assert!(!observable.contains("host-private"));
    assert!(!observable.contains("fixture-lease"));
    assert!(!observable.contains("egress-provider-secret"));
    let proxy_request = proxy_request
        .join()
        .expect("fake proxy must receive traffic");
    assert!(
        proxy_request.starts_with(
            "GET http://http-node-origin.invalid/route-pool-provider HTTP/1.1"
        ),
        "the HTTP node must reach the non-routable origin through the Host-created proxy client: {proxy_request}"
    );
}

/// Root #1805 AC-013: no persisted HTTP-node route retains direct reqwest behavior. No client or
/// lease is preconstructed by this fixture; the real resolver returns `None` to the node.
#[tokio::test]
async fn root_1805_http_node_no_route_keeps_direct_behavior_without_host_lease() {
    let (origin, origin_request) = spawn_one_request_proxy();
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let base_runtime = ApiProviderRuntime::new(network_egress_runtime_services());
    let resolver = crate::network_egress_client::NetworkEgressHttpClientResolver::new(
        state.store.clone(),
        base_runtime.clone(),
        state.provider_secret_master_key.clone(),
        state.api_node_id.clone(),
    );
    let invoker = ResolverBackedHttpNodeInvoker {
        runtime: base_runtime.with_network_egress(Arc::new(resolver)),
        workspace_id: state.bootstrap_workspace_id,
    };
    let execution =
        orchestration_runtime::execution_engine::execute_http_request_node_with_provider_invoker(
            &root_1805_http_node(&format!("{origin}/http-node-direct")),
            &Map::new(),
            &Map::new(),
            None,
            Some(&invoker),
        )
        .await
        .expect("HTTP execution must retain its normal result envelope");

    assert_eq!(execution.error_payload, None);
    assert_eq!(execution.output_payload["status_code"], json!(200));
    assert!(origin_request
        .join()
        .expect("direct origin must receive the request")
        .starts_with("GET /http-node-direct HTTP/1.1"));
    let observable = serde_json::to_string(&json!({
        "output_payload": execution.output_payload,
        "error_payload": execution.error_payload,
        "metrics_payload": execution.metrics_payload,
        "debug_payload": execution.debug_payload,
    }))
    .expect("HTTP output envelope must serialize");
    assert!(!observable.contains("cleanup_token"));
    assert!(!observable.contains("host-private"));
    assert!(!observable.contains("egress-provider-secret"));
}
