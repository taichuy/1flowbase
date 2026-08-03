use super::*;

#[tokio::test]
async fn orchestration_runtime_resolve_llm_instance_keeps_invalid_uuid_as_source_instance_id() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let invoker = RuntimeProviderInvoker {
        repository,
        runtime: test_support::InMemoryProviderRuntime::default(),
        workspace_id: Uuid::nil(),
        provider_secret_master_key: "test-master-key".to_string(),
        live_provider_events: None,
        runtime_event_stream: None,
        flow_run_id: None,
        active_node_id: None,
        active_node_run_id: None,
        api_node_id: Some("local:test".to_string()),
        provider_install_root: Some(std::env::temp_dir()),
        flow_execution_context: None,
        answer_presentation: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };

    let error = invoker
        .resolve_llm_instance(&compiled_llm_runtime("not-a-uuid", "fixture_provider"))
        .await
        .expect_err("invalid provider_instance_id should fail");

    assert_control_plane_error(error, ControlPlaneError::InvalidInput("source_instance_id"));
}

#[tokio::test]
async fn orchestration_runtime_resolve_llm_instance_does_not_fallback_when_selected_instance_is_missing(
) {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (alpha_instance_id, _) = repository.seed_included_provider_instances();
    let invoker = RuntimeProviderInvoker {
        repository,
        runtime: test_support::InMemoryProviderRuntime::default(),
        workspace_id: Uuid::nil(),
        provider_secret_master_key: "test-master-key".to_string(),
        live_provider_events: None,
        runtime_event_stream: None,
        flow_run_id: None,
        active_node_id: None,
        active_node_run_id: None,
        api_node_id: Some("local:test".to_string()),
        provider_install_root: Some(std::env::temp_dir()),
        flow_execution_context: None,
        answer_presentation: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };

    let error = invoker
        .resolve_llm_instance(&compiled_llm_runtime(
            Uuid::now_v7().to_string(),
            "fixture_provider",
        ))
        .await
        .expect_err("missing selected instance should fail");

    assert_control_plane_error(
        error,
        ControlPlaneError::NotFound("model_provider_instance"),
    );
    assert_ne!(alpha_instance_id, Uuid::nil());
}

#[tokio::test]
async fn orchestration_runtime_resolve_llm_instance_does_not_fallback_when_selected_instance_is_not_ready(
) {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (_, backup_instance_id) = repository.seed_included_provider_instances();
    repository.set_instance_status(
        backup_instance_id,
        domain::ModelProviderInstanceStatus::Disabled,
    );
    let invoker = RuntimeProviderInvoker {
        repository,
        runtime: test_support::InMemoryProviderRuntime::default(),
        workspace_id: Uuid::nil(),
        provider_secret_master_key: "test-master-key".to_string(),
        live_provider_events: None,
        runtime_event_stream: None,
        flow_run_id: None,
        active_node_id: None,
        active_node_run_id: None,
        api_node_id: Some("local:test".to_string()),
        provider_install_root: Some(std::env::temp_dir()),
        flow_execution_context: None,
        answer_presentation: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };

    let error = invoker
        .resolve_llm_instance(&compiled_llm_runtime(
            backup_instance_id.to_string(),
            "fixture_provider",
        ))
        .await
        .expect_err("non-ready selected instance should fail");

    assert_control_plane_error(
        error,
        ControlPlaneError::Conflict("provider_instance_not_ready"),
    );
}

#[tokio::test]
async fn orchestration_runtime_resolve_llm_instance_rejects_provider_code_mismatch() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    let invoker = RuntimeProviderInvoker {
        repository,
        runtime: test_support::InMemoryProviderRuntime::default(),
        workspace_id: Uuid::nil(),
        provider_secret_master_key: "test-master-key".to_string(),
        live_provider_events: None,
        runtime_event_stream: None,
        flow_run_id: None,
        active_node_id: None,
        active_node_run_id: None,
        api_node_id: Some("local:test".to_string()),
        provider_install_root: Some(std::env::temp_dir()),
        flow_execution_context: None,
        answer_presentation: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };

    let error = invoker
        .resolve_llm_instance(&compiled_llm_runtime(
            provider_instance_id.to_string(),
            "other_provider",
        ))
        .await
        .expect_err("provider_code mismatch should fail");

    assert_control_plane_error(error, ControlPlaneError::InvalidInput("provider_code"));
}

#[tokio::test]
async fn orchestration_runtime_resolve_llm_instance_rejects_instance_not_in_main() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let provider_instance_id = repository.seed_provider_instance(
        "fixture_provider",
        "Not In Main",
        false,
        domain::ModelProviderInstanceStatus::Ready,
        vec!["gpt-5.4-mini"],
    );
    let invoker = RuntimeProviderInvoker {
        repository,
        runtime: test_support::InMemoryProviderRuntime::default(),
        workspace_id: Uuid::nil(),
        provider_secret_master_key: "test-master-key".to_string(),
        live_provider_events: None,
        runtime_event_stream: None,
        flow_run_id: None,
        active_node_id: None,
        active_node_run_id: None,
        api_node_id: Some("local:test".to_string()),
        provider_install_root: Some(std::env::temp_dir()),
        flow_execution_context: None,
        answer_presentation: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };

    let error = invoker
        .resolve_llm_instance(&compiled_llm_runtime(
            provider_instance_id.to_string(),
            "fixture_provider",
        ))
        .await
        .expect_err("instance excluded from main should fail");

    assert_control_plane_error(
        error,
        ControlPlaneError::Conflict("provider_instance_not_in_main"),
    );
}

#[tokio::test]
async fn orchestration_runtime_resolve_llm_instance_rejects_unassigned_installation() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    let installation_id =
        ModelProviderRepository::get_instance(&repository, Uuid::nil(), provider_instance_id)
            .await
            .expect("instance lookup should succeed")
            .expect("instance should exist")
            .installation_id;
    repository.remove_assignment_for_installation(Uuid::nil(), installation_id);
    let invoker = RuntimeProviderInvoker {
        repository,
        runtime: test_support::InMemoryProviderRuntime::default(),
        workspace_id: Uuid::nil(),
        provider_secret_master_key: "test-master-key".to_string(),
        live_provider_events: None,
        runtime_event_stream: None,
        flow_run_id: None,
        active_node_id: None,
        active_node_run_id: None,
        api_node_id: Some("local:test".to_string()),
        provider_install_root: Some(std::env::temp_dir()),
        flow_execution_context: None,
        answer_presentation: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };

    let error = invoker
        .resolve_llm_instance(&compiled_llm_runtime(
            provider_instance_id.to_string(),
            "fixture_provider",
        ))
        .await
        .expect_err("unassigned installation should fail");

    assert_control_plane_error(
        error,
        ControlPlaneError::Conflict("plugin_assignment_required"),
    );
}

#[tokio::test]
async fn orchestration_runtime_resolve_llm_instance_rejects_disabled_installation() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    let installation_id =
        ModelProviderRepository::get_instance(&repository, Uuid::nil(), provider_instance_id)
            .await
            .expect("instance lookup should succeed")
            .expect("instance should exist")
            .installation_id;
    repository.set_installation_state(
        installation_id,
        domain::PluginDesiredState::Disabled,
        domain::PluginAvailabilityStatus::Available,
    );
    let invoker = RuntimeProviderInvoker {
        repository,
        runtime: test_support::InMemoryProviderRuntime::default(),
        workspace_id: Uuid::nil(),
        provider_secret_master_key: "test-master-key".to_string(),
        live_provider_events: None,
        runtime_event_stream: None,
        flow_run_id: None,
        active_node_id: None,
        active_node_run_id: None,
        api_node_id: Some("local:test".to_string()),
        provider_install_root: Some(std::env::temp_dir()),
        flow_execution_context: None,
        answer_presentation: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };

    let error = invoker
        .resolve_llm_instance(&compiled_llm_runtime(
            provider_instance_id.to_string(),
            "fixture_provider",
        ))
        .await
        .expect_err("disabled installation should fail");

    assert_control_plane_error(
        error,
        ControlPlaneError::Conflict("plugin_installation_unavailable"),
    );
}

#[tokio::test]
async fn orchestration_runtime_resolve_llm_instance_rejects_unavailable_installation() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    let installation_id =
        ModelProviderRepository::get_instance(&repository, Uuid::nil(), provider_instance_id)
            .await
            .expect("instance lookup should succeed")
            .expect("instance should exist")
            .installation_id;
    repository.set_installation_state(
        installation_id,
        domain::PluginDesiredState::ActiveRequested,
        domain::PluginAvailabilityStatus::ArtifactMissing,
    );
    let invoker = RuntimeProviderInvoker {
        repository,
        runtime: test_support::InMemoryProviderRuntime::default(),
        workspace_id: Uuid::nil(),
        provider_secret_master_key: "test-master-key".to_string(),
        live_provider_events: None,
        runtime_event_stream: None,
        flow_run_id: None,
        active_node_id: None,
        active_node_run_id: None,
        api_node_id: Some("local:test".to_string()),
        provider_install_root: Some(std::env::temp_dir()),
        flow_execution_context: None,
        answer_presentation: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };

    let error = invoker
        .resolve_llm_instance(&compiled_llm_runtime(
            provider_instance_id.to_string(),
            "fixture_provider",
        ))
        .await
        .expect_err("unavailable installation should fail");

    assert_control_plane_error(
        error,
        ControlPlaneError::Conflict("plugin_installation_unavailable"),
    );
}

#[tokio::test]
async fn orchestration_runtime_resolve_llm_instance_uses_selected_child_instance_without_provider_fallback(
) {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (_, backup_instance_id) = repository.seed_included_provider_instances();
    repository.set_instance_enabled_models(backup_instance_id, vec!["gpt-5.4-mini"]);
    let invoker = RuntimeProviderInvoker {
        repository: repository.clone(),
        runtime: test_support::InMemoryProviderRuntime::default(),
        workspace_id: Uuid::nil(),
        provider_secret_master_key: "test-master-key".to_string(),
        live_provider_events: None,
        runtime_event_stream: None,
        flow_run_id: None,
        active_node_id: None,
        active_node_run_id: None,
        api_node_id: Some("local:test".to_string()),
        provider_install_root: Some(std::env::temp_dir()),
        flow_execution_context: None,
        answer_presentation: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };

    let resolved = invoker
        .resolve_llm_instance(&orchestration_runtime::compiled_plan::CompiledLlmRuntime {
            provider_instance_id: backup_instance_id.to_string(),
            provider_instance_display_name: String::new(),
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            model: "gpt-5.4-mini".to_string(),
            routing: None,
        })
        .await
        .expect("selected child instance should resolve");

    let repository_instance =
        ModelProviderRepository::get_instance(&repository, Uuid::nil(), backup_instance_id)
            .await
            .expect("instance lookup should succeed")
            .expect("instance should exist");
    assert_eq!(resolved.id, repository_instance.id);
    assert_eq!(resolved.display_name, repository_instance.display_name);
}

#[tokio::test]
async fn orchestration_runtime_resolve_llm_instance_rejects_model_only_present_in_catalog_cache() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let selected_instance_id = repository.seed_provider_instance(
        "fixture_provider",
        "Cache Wider Than Enabled",
        true,
        domain::ModelProviderInstanceStatus::Ready,
        vec!["other-model"],
    );
    repository
        .set_instance_catalog_models(selected_instance_id, vec!["other-model", "gpt-5.4-mini"]);
    let invoker = RuntimeProviderInvoker {
        repository,
        runtime: test_support::InMemoryProviderRuntime::default(),
        workspace_id: Uuid::nil(),
        provider_secret_master_key: "test-master-key".to_string(),
        live_provider_events: None,
        runtime_event_stream: None,
        flow_run_id: None,
        active_node_id: None,
        active_node_run_id: None,
        api_node_id: Some("local:test".to_string()),
        provider_install_root: Some(std::env::temp_dir()),
        flow_execution_context: None,
        answer_presentation: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };

    let error = invoker
        .resolve_llm_instance(&orchestration_runtime::compiled_plan::CompiledLlmRuntime {
            provider_instance_id: selected_instance_id.to_string(),
            provider_instance_display_name: String::new(),
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            model: "gpt-5.4-mini".to_string(),
            routing: None,
        })
        .await
        .expect_err("model outside enabled_model_ids should fail");

    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::InvalidInput("model"))
    ));
}
