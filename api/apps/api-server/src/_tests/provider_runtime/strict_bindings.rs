use super::*;

#[tokio::test]
async fn strict_runtime_uses_one_boot_graph_for_model_provider_and_empty_input_pipeline() {
    let graph = model_provider_extension_graph();
    let services = ApiRuntimeServices::new(Arc::clone(&graph)).unwrap();
    let snapshot = crate::extension_bus::ExtensionBootSnapshot::new(Arc::clone(&graph));
    let resolver_graph = services.model_provider_extension_graph().unwrap();

    assert!(Arc::ptr_eq(resolver_graph, snapshot.graph_arc()));
    assert_eq!(
        resolver_graph.fingerprint().as_str(),
        snapshot.fingerprint()
    );
    let pipeline_output = ProviderRuntimePort::pipeline_provider_input(
        &ApiProviderRuntime::new(Arc::new(services.clone())),
        ProviderInvocationInput::default(),
    )
    .await
    .unwrap();
    assert_eq!(pipeline_output.input, ProviderInvocationInput::default());
    assert_eq!(
        pipeline_output.receipt.unwrap().graph_fingerprint,
        snapshot.fingerprint()
    );

    let legacy = ApiRuntimeServices::new_without_model_provider_extension_graph_for_tests();
    assert!(legacy.model_provider_extension_graph().is_none());
}

#[tokio::test]
async fn strict_model_provider_binding_loads_the_stateful_lifecycle_host() {
    let package = TempProviderPackage::new();
    let spawn_marker = package.path().join("spawned");
    write_stateful_slot_provider_package(&package, &spawn_marker);
    let mut installation = strict_fixture_installation(&package);
    installation.artifact.availability_status = PluginAvailabilityStatus::InstallIncomplete;
    let graph = model_provider_extension_graph();
    let services = Arc::new(ApiRuntimeServices::new(Arc::clone(&graph)).unwrap());
    let runtime = ApiProviderRuntime::new(services);

    let output = ProviderRuntimePort::validate_provider(&runtime, &installation, json!({}))
        .await
        .expect("strict binding should dispatch through the stateful provider host");
    assert_eq!(output["provider"], "fixture");
    assert!(spawn_marker.exists());
    let binding = crate::provider_runtime::ModelProviderSlotResolver::new(graph)
        .resolve(&installation)
        .unwrap();
    assert_eq!(binding.installation_id, installation.id);
    assert_eq!(binding.plugin_id, installation.plugin_id);
    assert_eq!(binding.provider_code, installation.provider_code);
    assert!(matches!(
        binding.provenance,
        crate::provider_runtime::ModelProviderBindingProvenance::BootGraph(_)
    ));
}

#[tokio::test]
async fn strict_model_provider_binding_rejects_invalid_dynamic_facts_before_spawn() {
    let package = TempProviderPackage::new();
    let spawn_marker = package.path().join("spawned");
    write_stateful_slot_provider_package(&package, &spawn_marker);
    let graph = model_provider_extension_graph();
    let runtime = ApiProviderRuntime::new(Arc::new(ApiRuntimeServices::new(graph).unwrap()));
    let valid = strict_fixture_installation(&package);
    let mut cases = Vec::new();

    let mut disabled = valid.clone();
    disabled.installation.desired_state = PluginDesiredState::Disabled;
    cases.push(disabled.clone());
    let mut pending = valid.clone();
    pending.installation.desired_state = PluginDesiredState::PendingRestart;
    cases.push(pending);
    let mut artifact_missing = valid.clone();
    artifact_missing.artifact.artifact_status = domain::PluginArtifactInstanceStatus::Missing;
    cases.push(artifact_missing);
    let mut artifact_mismatched = valid.clone();
    artifact_mismatched.artifact.artifact_status = domain::PluginArtifactInstanceStatus::Mismatched;
    cases.push(artifact_mismatched);
    let mut load_failed = valid.clone();
    load_failed.artifact.runtime_status = PluginRuntimeStatus::LoadFailed;
    cases.push(load_failed);
    let mut verification_pending = valid.clone();
    verification_pending.installation.verification_status = PluginVerificationStatus::Pending;
    cases.push(verification_pending);
    let mut wrong_contract = valid.clone();
    wrong_contract.installation.contract_version = "1flowbase.data_source/v1".to_string();
    cases.push(wrong_contract);
    let mut wrong_category = valid.clone();
    wrong_category.installation.category = domain::ExtensionCategory::CapabilityPlugins;
    cases.push(wrong_category);

    for installation in cases {
        assert!(
            ProviderRuntimePort::validate_provider(&runtime, &installation, json!({}))
                .await
                .is_err()
        );
    }
    let canonical_input = ProviderInvocationInput {
        provider_code: disabled.provider_code.clone(),
        ..ProviderInvocationInput::default()
    };
    assert!(ProviderRuntimePort::ensure_loaded(&runtime, &disabled)
        .await
        .is_err());
    assert!(
        ProviderRuntimePort::list_models(&runtime, &disabled, json!({}))
            .await
            .is_err()
    );
    assert!(
        ProviderRuntimePort::get_balance(&runtime, &disabled, json!({}))
            .await
            .is_err()
    );
    assert!(ProviderRuntimePort::count_tokens(
        &runtime,
        &disabled,
        ProviderCountTokensInput::from_invocation(canonical_input.clone())
    )
    .await
    .is_err());
    assert!(
        ProviderRuntimePort::compact(&runtime, &disabled, canonical_input.clone())
            .await
            .is_err()
    );
    assert!(
        ProviderRuntimePort::invoke_stream(&runtime, &disabled, canonical_input.clone())
            .await
            .is_err()
    );
    assert!(ProviderRuntimePort::invoke_stream_with_live_events(
        &runtime,
        &disabled,
        canonical_input,
        None
    )
    .await
    .is_err());
    assert!(!spawn_marker.exists());
}

#[tokio::test]
async fn strict_model_provider_binding_rejects_wrong_provider_code_and_missing_point_before_spawn()
{
    let package = TempProviderPackage::new();
    let spawn_marker = package.path().join("spawned");
    write_stateful_slot_provider_package(&package, &spawn_marker);
    let installation = strict_fixture_installation(&package);
    let runtime = ApiProviderRuntime::new(Arc::new(
        ApiRuntimeServices::new(model_provider_extension_graph()).unwrap(),
    ));
    let wrong_provider = ProviderInvocationInput {
        provider_code: "another_provider".to_string(),
        ..ProviderInvocationInput::default()
    };
    assert!(runtime
        .invoke_stream(&installation, wrong_provider)
        .await
        .is_err());

    let missing_point_error = crate::provider_runtime::ModelProviderSlotResolver::new(
        graph_without_model_provider_point(),
    )
    .resolve(&installation)
    .unwrap_err();
    assert!(missing_point_error
        .to_string()
        .contains("model_provider_extension_slot_unavailable"));
    assert!(!spawn_marker.exists());
}

#[test]
fn provider_runtime_consumer_has_no_provider_specific_branch_and_production_wires_strict_graph() {
    let consumer = concat!(
        include_str!("../../provider_runtime/mod.rs"),
        "\n",
        include_str!("../../provider_runtime/provider_port.rs"),
        "\n",
        include_str!("../../provider_runtime/capability_port.rs"),
    );
    let transport_lifecycle = concat!(
        include_str!("../../provider_runtime/transport_session_lifecycle.rs"),
        "\n",
        include_str!("../../provider_runtime/transport_session_lifecycle/admission.rs"),
    );
    let boot = include_str!("../../lib.rs");

    assert!(!consumer.to_ascii_lowercase().contains("openai"));
    assert!(!consumer.to_ascii_lowercase().contains("anthropic"));
    assert_eq!(
        consumer
            .matches("self.resolve_model_provider_binding(installation)?")
            .count(),
        13,
        "every ProviderRuntimePort operation must resolve the typed slot binding"
    );
    assert!(consumer.contains(".orchestration_backend"));
    assert!(consumer.contains(".provider_count_tokens("));
    assert!(consumer.contains(".data_source_execute_sql("));
    assert!(consumer.contains(".capability_execute("));
    assert!(consumer.contains(".network_egress_sync("));
    assert!(!consumer.contains(".provider_registry()"));
    assert!(!consumer.contains(".data_source_registry()"));
    assert!(!consumer.contains(".capability_registry()"));
    assert!(!consumer.contains(".network_egress_registry()"));
    assert_eq!(
        consumer.matches("binding.require_provider_code(").count(),
        5,
        "every canonical provider input must fail closed on a crossed provider code"
    );
    assert!(boot.contains(
        "RuntimeExtensionHost::new_with_artifact_resolver_plugin_data_and_profile_source("
    ));
    assert!(boot.contains("ApiRuntimeArtifactResolver::new("));
    assert!(boot.contains("RuntimeBackendSlot::default()"));
    assert!(boot.contains("runtime_backend_slot.bind(runtime_extension_host.clone())"));
    assert!(boot.contains("ApiRuntimeServices::new_with_runtime_backend_and_transport_config("));
    assert!(boot.contains("provider_runtime.start_transport_session_scheduler()"));
    assert!(boot.contains("shutdown_transport_sessions("));
    assert!(transport_lifecycle.contains("maintain_and_dispatch"));
    assert!(transport_lifecycle.contains("ProviderTransportSessionAction::Drain"));
    assert!(transport_lifecycle.contains("ProviderTransportSessionAction::Close"));
    assert!(transport_lifecycle.contains("generation: fence.generation.get()"));
    assert!(transport_lifecycle.contains("InvocationRequest"));
    assert!(transport_lifecycle.contains(".rotate_generation_to(&fence, target.clone())"));
    assert!(transport_lifecycle.contains("drain_disposition(&snapshot, &task.fence)"));
    assert!(!transport_lifecycle.contains("task_deadline"));
    assert!(boot.contains("Arc::clone(&extension_graph)"));
    assert!(!boot.contains("new_without_model_provider_extension_graph_for_tests"));
}

#[tokio::test]
async fn ac_003_api_runtime_dispatches_network_egress_lifecycle_without_model_provider_v2_wire() {
    let package = TempProviderPackage::new();
    write_network_egress_package(&package);
    let installation = network_egress_fixture_installation(&package);
    let services =
        Arc::new(ApiRuntimeServices::new_without_model_provider_extension_graph_for_tests());
    let runtime = ApiProviderRuntime::new(services);

    ProviderRuntimePort::activate_plugin(&runtime, &installation)
        .await
        .expect("network egress activation must dispatch its stateful worker");
    ProviderRuntimePort::deactivate_plugin(&runtime, &installation.installation)
        .await
        .expect("network egress deactivation must revoke its active lease");
}

#[tokio::test]
async fn data_source_runtime_intakes_and_dispatches_checked_external_template() {
    let services =
        Arc::new(ApiRuntimeServices::new_without_model_provider_extension_graph_for_tests());
    let runtime = ApiProviderRuntime::new(services.clone());
    let installation = checked_data_source_fixture_installation();

    DataSourceRuntimePort::ensure_loaded(&runtime, &installation)
        .await
        .expect("checked data source fixture should load");
    let identity = DataModelTemplateIdentity {
        provider: "data_source_http_fixture".to_string(),
        code: "contact_archive".to_string(),
        version: "v1".to_string(),
    };
    let template = services
        .data_model_template_catalog()
        .resolve(&identity)
        .expect("checked external template should enter the shared catalog");
    assert_eq!(template.descriptor().operations[0].code, "archive_contact");

    let output = DataSourceRuntimePort::execute_model_operation(
        &runtime,
        &installation,
        checked_data_source_model_operation_input(json!({ "reason": "duplicate" })),
    )
    .await
    .expect("checked external operation should dispatch through the host");
    assert_eq!(output["archived"], true);
    assert_eq!(output["contact_id"], "contact-1");

    let malformed_error = DataSourceRuntimePort::execute_model_operation(
        &runtime,
        &installation,
        checked_data_source_model_operation_input(json!({ "malformed_response": true })),
    )
    .await
    .expect_err("malformed external operation output must fail closed");
    assert!(matches!(
        malformed_error.downcast_ref::<control_plane::errors::ControlPlaneError>(),
        Some(control_plane::errors::ControlPlaneError::InvalidInput(
            "data_source_runtime"
        ))
    ));
}
