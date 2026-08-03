use super::*;

#[derive(Clone)]
struct CapturingCountTokensRuntime {
    captured: Arc<Mutex<Vec<ProviderCountTokensInput>>>,
}

#[derive(Clone)]
struct CapturingCompactRuntime {
    captured: Arc<Mutex<Vec<ProviderInvocationInput>>>,
}

#[async_trait]
impl ProviderRuntimePort for CapturingCompactRuntime {
    async fn ensure_loaded(
        &self,
        _installation: &domain::LocalPluginInstallationRecord,
    ) -> Result<()> {
        Ok(())
    }

    async fn validate_provider(
        &self,
        _installation: &domain::LocalPluginInstallationRecord,
        _provider_config: Value,
    ) -> Result<Value> {
        Ok(json!({ "ok": true }))
    }

    async fn list_models(
        &self,
        _installation: &domain::LocalPluginInstallationRecord,
        _provider_config: Value,
    ) -> Result<Vec<ProviderModelDescriptor>> {
        Ok(Vec::new())
    }

    async fn compact(
        &self,
        _installation: &domain::LocalPluginInstallationRecord,
        input: ProviderInvocationInput,
    ) -> Result<ProviderCompactResult> {
        self.captured
            .lock()
            .expect("capture mutex poisoned")
            .push(input);
        Ok(ProviderCompactResult::ResponseItems {
            operation: ProviderWireOperation::Compact,
            profile: ProviderCompactProfile::ResponsesCompact,
            response_items: vec![json!({ "type": "message" })],
        })
    }

    async fn invoke_stream(
        &self,
        _installation: &domain::LocalPluginInstallationRecord,
        _input: ProviderInvocationInput,
    ) -> Result<crate::ports::ProviderRuntimeInvocationOutput> {
        panic!("Compact adapter must not invoke Generate")
    }
}

#[tokio::test]
async fn orchestration_runtime_compact_resolves_selected_runtime_and_provider_config() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    let captured = Arc::new(Mutex::new(Vec::new()));
    let invoker = RuntimeProviderInvoker {
        repository,
        runtime: CapturingCompactRuntime {
            captured: captured.clone(),
        },
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
        provider_transport_payload: Some(
            crate::ports::ProviderTransportPayload::openai_responses(json!({
                "model": "gpt-5.4-mini",
                "input": "compact transport"
            }))
            .unwrap()
            .with_affinity(crate::ports::ProviderTransportAffinity::new(
                provider_instance_id.to_string(),
                "fixture_provider",
                "openai_compatible",
                "gpt-5.4-mini",
            )),
        ),
        provider_transport_store: None,
        provider_continuation: None,
    };
    let runtime = compiled_llm_runtime(provider_instance_id.to_string(), "fixture_provider");
    let result = orchestration_runtime::execution_engine::ProviderInvoker::compact(
        &invoker,
        &runtime,
        ProviderInvocationInput {
            operation: ProviderWireOperation::Compact,
            profile: Some(ProviderCompactProfile::ResponsesCompact),
            provider_instance_id: provider_instance_id.to_string(),
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            model: "gpt-5.4-mini".to_string(),
            messages: vec![ProviderMessage {
                role: ProviderMessageRole::User,
                content: "canonical compact prompt".to_string(),
                name: None,
                tool_call_id: None,
                is_error: None,
                tool_calls: None,
                content_blocks: None,
            }],
            ..ProviderInvocationInput::default()
        },
    )
    .await
    .expect("selected provider Compact capability should run");
    assert!(result.satisfies_profile(ProviderCompactProfile::ResponsesCompact));
    let captured = captured.lock().expect("capture mutex poisoned");
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].operation, ProviderWireOperation::Compact);
    assert_eq!(
        captured[0].profile,
        Some(ProviderCompactProfile::ResponsesCompact)
    );
    assert_eq!(
        captured[0].provider_instance_id,
        provider_instance_id.to_string()
    );
    assert_eq!(captured[0].model, "gpt-5.4-mini");
    assert_eq!(captured[0].messages[0].content, "canonical compact prompt");
    assert!(captured[0].native_transport.is_some());
    assert!(!captured[0].provider_config.is_null());
}

#[async_trait]
impl ProviderRuntimePort for CapturingCountTokensRuntime {
    async fn ensure_loaded(
        &self,
        _installation: &domain::LocalPluginInstallationRecord,
    ) -> Result<()> {
        Ok(())
    }

    async fn validate_provider(
        &self,
        _installation: &domain::LocalPluginInstallationRecord,
        _provider_config: Value,
    ) -> Result<Value> {
        Ok(json!({ "ok": true }))
    }

    async fn list_models(
        &self,
        _installation: &domain::LocalPluginInstallationRecord,
        _provider_config: Value,
    ) -> Result<Vec<ProviderModelDescriptor>> {
        Ok(Vec::new())
    }

    async fn count_tokens(
        &self,
        _installation: &domain::LocalPluginInstallationRecord,
        input: ProviderCountTokensInput,
    ) -> Result<ProviderCountTokensResult> {
        self.captured
            .lock()
            .expect("capture mutex poisoned")
            .push(input);
        Ok(ProviderCountTokensResult {
            operation: ProviderWireOperation::CountTokens,
            input_tokens: 23,
            ..ProviderCountTokensResult::default()
        })
    }

    async fn invoke_stream(
        &self,
        _installation: &domain::LocalPluginInstallationRecord,
        _input: ProviderInvocationInput,
    ) -> Result<crate::ports::ProviderRuntimeInvocationOutput> {
        panic!("CountTokens adapter must not invoke Generate")
    }
}

#[tokio::test]
async fn orchestration_runtime_count_tokens_resolves_selected_runtime_and_provider_config() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    let captured = Arc::new(Mutex::new(Vec::new()));
    let invoker = RuntimeProviderInvoker {
        repository,
        runtime: CapturingCountTokensRuntime {
            captured: captured.clone(),
        },
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
    let runtime = compiled_llm_runtime(provider_instance_id.to_string(), "fixture_provider");
    let result = orchestration_runtime::execution_engine::ProviderInvoker::count_tokens(
        &invoker,
        &runtime,
        ProviderCountTokensInput::from_invocation(ProviderInvocationInput {
            provider_instance_id: provider_instance_id.to_string(),
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            model: "gpt-5.4-mini".to_string(),
            messages: vec![ProviderMessage {
                role: ProviderMessageRole::User,
                content: "canonical prompt".to_string(),
                name: None,
                tool_call_id: None,
                is_error: None,
                tool_calls: None,
                content_blocks: None,
            }],
            ..ProviderInvocationInput::default()
        }),
    )
    .await
    .expect("selected provider CountTokens capability should run");
    assert_eq!(result.input_tokens, 23);
    let captured = captured.lock().expect("capture mutex poisoned");
    assert_eq!(captured.len(), 1);
    assert_eq!(
        captured[0].provider_instance_id,
        provider_instance_id.to_string()
    );
    assert_eq!(captured[0].model, "gpt-5.4-mini");
    assert_eq!(captured[0].messages[0].content, "canonical prompt");
    assert!(!captured[0].provider_config.is_null());
}
