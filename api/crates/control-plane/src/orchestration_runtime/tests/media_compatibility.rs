use super::*;

#[tokio::test]
async fn orchestration_runtime_textualizes_user_media_when_selected_model_is_not_multimodal() {
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
    let runtime = orchestration_runtime::compiled_plan::CompiledLlmRuntime {
        provider_instance_id: provider_instance_id.to_string(),
        provider_instance_display_name: String::new(),
        provider_code: "fixture_provider".to_string(),
        protocol: "openai_compatible".to_string(),
        model: "gpt-5.4-mini".to_string(),
        routing: None,
    };
    let input = ProviderInvocationInput {
        provider_instance_id: provider_instance_id.to_string(),
        provider_code: "fixture_provider".to_string(),
        protocol: "openai_compatible".to_string(),
        model: "gpt-5.4-mini".to_string(),
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::User,
            content: "Describe image".to_string(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: Some(json!([
                {"type": "text", "text": "Describe image"},
                {
                    "type": "image_url",
                    "image_url": {"url": "https://example.com/cat.png"}
                }
            ])),
        }],
        ..ProviderInvocationInput::default()
    };

    let output = orchestration_runtime::execution_engine::ProviderInvoker::invoke_llm(
        &invoker, &runtime, input,
    )
    .await
    .expect("non-multimodal model should receive textualized media context");

    let content = output.result.final_content.unwrap_or_default();
    assert!(content.contains("\"error_code\":\"message_media_unsupported\""));
    assert!(content.contains("\"url\":\"https://example.com/cat.png\""));
    assert!(!content.contains("content_blocks"));
}

#[tokio::test]
async fn orchestration_runtime_keeps_user_media_when_configured_model_supports_multimodal() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    repository.set_configured_model_supports_multimodal(provider_instance_id, "gpt-5.4-mini", true);
    let (runtime_port, captured_inputs) =
        test_support::InMemoryProviderRuntime::with_invocation_capture();
    let invoker = RuntimeProviderInvoker {
        repository,
        runtime: runtime_port,
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
    let runtime = orchestration_runtime::compiled_plan::CompiledLlmRuntime {
        provider_instance_id: provider_instance_id.to_string(),
        provider_instance_display_name: String::new(),
        provider_code: "fixture_provider".to_string(),
        protocol: "openai_compatible".to_string(),
        model: "gpt-5.4-mini".to_string(),
        routing: None,
    };
    let input = ProviderInvocationInput {
        provider_instance_id: provider_instance_id.to_string(),
        provider_code: "fixture_provider".to_string(),
        protocol: "openai_compatible".to_string(),
        model: "gpt-5.4-mini".to_string(),
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::User,
            content: "Describe image".to_string(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: Some(json!([
                {"type": "text", "text": "Describe image"},
                {
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": "image/png",
                        "data": "aW1hZ2U="
                    }
                }
            ])),
        }],
        ..ProviderInvocationInput::default()
    };

    orchestration_runtime::execution_engine::ProviderInvoker::invoke_llm(&invoker, &runtime, input)
        .await
        .expect("configured multimodal model should receive media content blocks");

    let captured = captured_inputs
        .lock()
        .expect("captured provider inputs should be readable");
    let content_blocks = captured[0].messages[0]
        .content_blocks
        .as_ref()
        .expect("media content blocks should be preserved for multimodal configured models");
    assert_eq!(content_blocks[1]["type"], json!("image"));
    assert_eq!(
        content_blocks[1]["source"]["media_type"],
        json!("image/png")
    );
    assert!(!captured[0].messages[0]
        .content
        .contains("message_media_unsupported"));
}

#[test]
fn orchestration_runtime_textualizes_tool_result_media_for_text_models() {
    let mut input = ProviderInvocationInput {
        messages: vec![
            ProviderMessage {
                role: ProviderMessageRole::User,
                content: "Describe image".to_string(),
                name: None,
                tool_call_id: None,
                is_error: None,
                tool_calls: None,
                content_blocks: None,
            },
            ProviderMessage {
                role: ProviderMessageRole::Tool,
                content: String::new(),
                name: Some("Read".to_string()),
                tool_call_id: Some("call_read".to_string()),
                is_error: None,
                tool_calls: None,
                content_blocks: Some(json!([
                    {
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": "image/png",
                            "data": "aW1hZ2U="
                        }
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": "data:image/png;base64,SHOULD_NOT_BE_VISIBLE"
                        }
                    }
                ])),
            },
        ],
        ..ProviderInvocationInput::default()
    };

    provider_invoker::textualize_media_content_blocks_for_text_model(&mut input);

    let tool_message = &input.messages[1];
    assert!(tool_message.content_blocks.is_none());
    assert!(tool_message
        .content
        .contains("\"error_code\":\"tool_result_media_unsupported\""));
    assert!(tool_message
        .content
        .contains("\"media_type\":\"image/png\""));
    assert!(!tool_message.content.contains("aW1hZ2U="));
    assert!(tool_message
        .content
        .contains("\"url\":\"data:image/png;base64,[redacted]\""));
    assert!(!tool_message.content.contains("SHOULD_NOT_BE_VISIBLE"));
}

#[test]
fn orchestration_runtime_textualizes_routed_media_as_retry_guidance_for_text_models() {
    let mut input = ProviderInvocationInput {
        messages: vec![
            ProviderMessage {
                role: ProviderMessageRole::User,
                content: "Describe image".to_string(),
                name: None,
                tool_call_id: None,
                is_error: None,
                tool_calls: None,
                content_blocks: None,
            },
            ProviderMessage {
                role: ProviderMessageRole::Tool,
                content: String::new(),
                name: Some("Read".to_string()),
                tool_call_id: Some("call_read".to_string()),
                is_error: None,
                tool_calls: None,
                content_blocks: Some(json!([
                    {
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": "image/png",
                            "data": "aW1hZ2U="
                        }
                    }
                ])),
            },
        ],
        run_context: std::collections::BTreeMap::from([(
            "visible_internal_llm_media_tools".to_string(),
            json!([
                {
                    "name": "image_llm",
                    "media_kind": "image"
                }
            ]),
        )]),
        ..ProviderInvocationInput::default()
    };

    provider_invoker::textualize_media_content_blocks_for_text_model(&mut input);

    let tool_message = &input.messages[1];
    assert!(tool_message.content_blocks.is_none());
    assert!(tool_message
        .content
        .contains("\"event\":\"routed_media_content_available\""));
    assert!(tool_message.content.contains("\"name\":\"image_llm\""));
    assert!(tool_message
        .content
        .contains("Call the routed media tool again"));
    assert!(!tool_message
        .content
        .contains("tool_result_media_unsupported"));
    assert!(!tool_message.content.contains("message_media_unsupported"));
    assert!(!tool_message.content.contains("aW1hZ2U="));
}
