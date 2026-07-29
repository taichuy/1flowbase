use super::*;
use control_plane::application_public_api::{
    compat::anthropic::translate_messages_request, protocol_translation::TranslationDecisionKind,
};

#[tokio::test]
async fn start_native_run_creates_published_api_flow_run_from_frozen_publication() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Published Native App");
    let token = issue_key(&harness, application.id).await;
    let publication = publish_runnable_application(&repository, application.id).await;
    let service = ApplicationPublishedRunService::new(repository.clone());

    let result = service
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token,
            request: native_request("streaming", None),
        })
        .await
        .unwrap();
    let flow_run = repository
        .get_flow_run(application.id, result.id)
        .await
        .unwrap()
        .expect("published flow run should be durable");

    assert_eq!(flow_run.run_mode, domain::FlowRunMode::PublishedApiRun);
    assert_eq!(flow_run.created_by, actor_user_id());
    assert_eq!(flow_run.flow_id, publication.flow_id);
    assert_eq!(
        flow_run.compiled_plan_id,
        Some(publication.compiled_plan_id)
    );
    assert_eq!(
        flow_run.flow_schema_version,
        publication.flow_schema_version
    );
    assert_eq!(flow_run.document_hash, publication.document_hash);
    assert_eq!(flow_run.publication_version_id, Some(publication.id));
    assert_eq!(flow_run.target_node_id, None);
    assert_eq!(flow_run.title, "Summarize the incident");
    assert_eq!(flow_run.external_user.as_deref(), Some("customer-1"));
    assert_eq!(
        flow_run.external_conversation_id.as_deref(),
        Some("conversation-1")
    );
    assert_eq!(flow_run.external_trace_id.as_deref(), Some("trace-1"));
    assert_eq!(flow_run.compatibility_mode.as_deref(), Some("native-v1"));
    assert_eq!(
        flow_run.input_payload,
        json!({
            "env": {},
            "node-start": {
                "query": "Summarize the incident",
                "priority": "high",
                "system": [],
                "operation": {"kind": "generate", "profile": "standard"}
            }
        })
    );
    assert_eq!(result.metadata["model"], json!("public-model/pass-through"));
    assert_eq!(repository.published_generate_capability_checks(), 0);
}

#[tokio::test]
async fn trusted_translation_protocol_is_persisted_as_the_canonical_compatibility_mode() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Protocol Persistence App");
    let token = issue_key(&harness, application.id).await;
    publish_runnable_application(&repository, application.id).await;
    let service = ApplicationPublishedRunService::new(repository.clone());

    for (protocol, expected) in [
        (
            control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            "native-v1",
        ),
        (
            control_plane::application_public_api::protocol_translation::TranslationProtocol::AnthropicMessages,
            "anthropic-messages-v1",
        ),
        (
            control_plane::application_public_api::protocol_translation::TranslationProtocol::OpenAiChat,
            "openai-chat-completions-v1",
        ),
        (
            control_plane::application_public_api::protocol_translation::TranslationProtocol::OpenAiResponses,
            "openai-responses-v1",
        ),
    ] {
        let result = service
            .start_native_run(CreateNativeRunCommand {
                protocol,
                bearer_token: token.clone(),
                request: native_request("blocking", None),
            })
            .await
            .expect("trusted protocol should create a durable run");
        let flow_run = repository
            .get_flow_run(application.id, result.id)
            .await
            .unwrap()
            .expect("protocol-tagged run should be durable");

        assert_eq!(flow_run.compatibility_mode.as_deref(), Some(expected));
    }
}

#[tokio::test]
async fn d2_f1_anthropic_trace_id_reaches_canonical_metadata_and_durable_flow_run() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Anthropic Trace Id App");
    let token = issue_key(&harness, application.id).await;
    publish_runnable_application(&repository, application.id).await;
    let translated = translate_messages_request(json!({
        "model": "claude-compatible-custom",
        "messages": [{"role": "user", "content": "trace this request"}],
        "metadata": {"trace_id": "anthropic-trace-42"}
    }))
    .expect("Anthropic trace_id should map into the canonical Native request");
    assert_eq!(
        serde_json::to_value(&translated.request).expect("canonical Native request serializes")
            ["metadata"],
        json!({"trace_id": "anthropic-trace-42"})
    );
    assert!(translated
        .report
        .has_decision("$.metadata.trace_id", TranslationDecisionKind::Normalized));

    let result = ApplicationPublishedRunService::new(repository.clone())
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::AnthropicMessages,
            bearer_token: token,
            request: translated.request,
        })
        .await
        .expect("the mapped Native request should create a flow run");
    let flow_run = repository
        .get_flow_run(application.id, result.id)
        .await
        .unwrap()
        .expect("published flow run should be durable");

    assert_eq!(
        flow_run.external_trace_id.as_deref(),
        Some("anthropic-trace-42")
    );
    assert_eq!(
        result.metadata["external_trace_id"],
        json!("anthropic-trace-42")
    );
}

#[tokio::test]
async fn wp_d2a_anthropic_one_m_is_explicit_typed_intent_not_catalog_inference() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Anthropic 1M Context App");
    let token = issue_key(&harness, application.id).await;
    save_start_model_catalog(&repository, &application).await;
    publish_runnable_application(&repository, application.id).await;
    let translated = translate_messages_request(json!({
        "model": "claude-opus-4-8[1M]",
        "max_tokens": 4096,
        "messages": [{"role": "user", "content": "use declared context"}]
    }))
    .expect("Anthropic request should translate");

    let result = ApplicationPublishedRunService::new(repository.clone())
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::AnthropicMessages,
            bearer_token: token.clone(),
            request: translated.request,
        })
        .await
        .expect("declared 1M workflow model should create a run");
    let flow_run = repository
        .get_published_flow_run(result.id)
        .await
        .unwrap()
        .expect("published flow run should be durable");
    assert_eq!(
        flow_run.input_payload["sys"]["model_parameters"]["requested_context_window"],
        json!(1_000_000)
    );
    assert!(flow_run
        .input_payload
        .get("__client_protocol_envelope")
        .is_none());

    let catalog_only = translate_messages_request(json!({
        "model": "claude-opus-4-8",
        "max_tokens": 4096,
        "messages": [{"role": "user", "content": "do not infer context intent"}]
    }))
    .expect("the catalog model without an explicit 1M request should translate");
    let catalog_only_result = ApplicationPublishedRunService::new(repository.clone())
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::AnthropicMessages,
            bearer_token: token,
            request: catalog_only.request,
        })
        .await
        .expect("a catalog model without context intent should create a run");
    let catalog_only_run = repository
        .get_published_flow_run(catalog_only_result.id)
        .await
        .unwrap()
        .expect("catalog-only flow run should be durable");

    assert!(catalog_only_run.input_payload["sys"]["model_parameters"]
        .get("requested_context_window")
        .is_none());
    assert!(catalog_only_run
        .input_payload
        .get("__client_protocol_envelope")
        .is_none());
}

#[tokio::test]
async fn start_native_run_freezes_valid_external_reasoning_parameters_for_runtime() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Published Native Reasoning App");
    let token = issue_key(&harness, application.id).await;
    save_start_model_catalog(&repository, &application).await;
    publish_runnable_application(&repository, application.id).await;
    let service = ApplicationPublishedRunService::new(repository.clone());

    let result = service
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token,
            request: serde_json::from_value(json!({
                "query": "Summarize the incident",
                "model": "gpt-5.4",
                "inputs": {
                    "priority": "high"
                },
                "execution": {
                    "model_parameters": {
                        "reasoning": {
                            "mode": "adaptive",
                            "effort": "high",
                            "budget_tokens": 4096
                        }
                    }
                }
            }))
            .unwrap(),
        })
        .await
        .unwrap();
    let flow_run = repository
        .get_flow_run(application.id, result.id)
        .await
        .unwrap()
        .expect("published flow run should be durable");

    assert_eq!(
        flow_run.input_payload["sys"]["model_parameters"],
        json!({
            "reasoning": {
                "mode": "adaptive",
                "effort": "high",
                "budget_tokens": 4096
            }
        })
    );
    assert_eq!(
        flow_run.input_payload["sys"]["model_parameters"]["reasoning"]["mode"],
        json!("adaptive")
    );
    assert!(flow_run.input_payload["node-start"]
        .get("reasoning_effort")
        .is_none());
}

#[tokio::test]
async fn ac_004_start_native_run_freezes_external_max_output_tokens_for_runtime() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Published Native Token App");
    let token = issue_key(&harness, application.id).await;
    save_start_model_catalog(&repository, &application).await;
    publish_runnable_application(&repository, application.id).await;
    let service = ApplicationPublishedRunService::new(repository.clone());

    let result = service
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token,
            request: native_request_with_model_parameters(
                "gpt-5.4",
                json!({ "max_output_tokens": 32000 }),
            ),
        })
        .await
        .unwrap();
    let flow_run = repository
        .get_flow_run(application.id, result.id)
        .await
        .unwrap()
        .expect("published flow run should be durable");

    assert_eq!(
        flow_run.input_payload["sys"]["model_parameters"]["max_output_tokens"],
        json!(32000)
    );
    assert!(flow_run.input_payload["node-start"]
        .get("max_output_tokens")
        .is_none());
}

#[test]
fn native_adapter_rejects_context_window_as_runtime_model_parameter() {
    let error = translate_native_run_request(json!({
        "query": "Summarize the incident",
        "model": "gpt-5.4",
        "execution": {
            "model_parameters": {
                "context_window": 128000
            }
        }
    }))
    .expect_err("unknown model parameters must fail at the Native adapter");

    assert_eq!(error.code, "invalid_model_parameters");
    assert!(error
        .report
        .decisions
        .iter()
        .any(|decision| { decision.source_path == "$.execution.model_parameters.<unknown>[0]" }));
}

#[tokio::test]
async fn start_native_run_defers_requested_model_validation_to_the_selected_llm_node() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Published Native Unknown App");
    let token = issue_key(&harness, application.id).await;
    save_start_model_catalog(&repository, &application).await;
    publish_runnable_application(&repository, application.id).await;
    let service = ApplicationPublishedRunService::new(repository.clone());

    let result = service
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token,
            request: native_request_with_model_parameters(
                "missing-model",
                json!({
                    "reasoning": {
                        "mode": "enabled",
                        "effort": "high"
                    }
                }),
            ),
        })
        .await
        .expect("the workflow must select its LLM node before model capability validation");
    let flow_run = repository
        .get_published_flow_run(result.id)
        .await
        .unwrap()
        .expect("published flow run should be durable");

    assert_eq!(result.metadata["model"], json!("missing-model"));
    assert_eq!(
        flow_run.input_payload["sys"]["model_parameters"]["reasoning"]["effort"],
        json!("high")
    );
}

#[tokio::test]
async fn ac_004_start_native_run_accepts_request_cap_over_catalog_default_output() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Published Native Output App");
    let token = issue_key(&harness, application.id).await;
    save_start_model_catalog(&repository, &application).await;
    publish_runnable_application(&repository, application.id).await;
    let service = ApplicationPublishedRunService::new(repository.clone());

    let request =
        native_request_with_model_parameters("gpt-5.4", json!({ "max_output_tokens": 64000 }));

    let result = service
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token,
            request,
        })
        .await
        .expect("request cap above the catalog default output should pass through");

    let flow_run = repository
        .get_published_flow_run(result.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        flow_run.input_payload["sys"]["model_parameters"]["max_output_tokens"],
        json!(64000)
    );
}

#[tokio::test]
async fn ac_004_start_native_run_defers_output_limit_validation_to_the_selected_llm_node() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Published Native Output App");
    let token = issue_key(&harness, application.id).await;
    save_start_model_catalog(&repository, &application).await;
    publish_runnable_application(&repository, application.id).await;
    let service = ApplicationPublishedRunService::new(repository.clone());

    let mut request =
        native_request_with_model_parameters("gpt-5.4", json!({ "max_output_tokens": 128001 }));
    request.conversation = serde_json::from_value(json!({
        "id": "catalog-over-limit-conversation",
        "user": "catalog-over-limit-user"
    }))
    .expect("conversation fixture must be valid Native data");
    let result = service
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token,
            request,
        })
        .await
        .expect("the selected LLM/provider owns its output limit");
    let flow_run = repository
        .get_published_flow_run(result.id)
        .await
        .unwrap()
        .expect("published flow run should be durable");

    assert_eq!(
        flow_run.input_payload["sys"]["model_parameters"]["max_output_tokens"],
        json!(128001)
    );
}

#[tokio::test]
async fn start_native_run_freezes_application_environment_variables() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Published Native Env App");
    let token = issue_key(&harness, application.id).await;
    publish_runnable_application(&repository, application.id).await;
    ApplicationRepository::replace_application_environment_variables(
        &repository,
        &ReplaceApplicationEnvironmentVariablesInput {
            actor_user_id: actor_user_id(),
            workspace_id: application.workspace_id,
            application_id: application.id,
            variables: vec![ApplicationEnvironmentVariableInput {
                name: "ApiBaseUrl".into(),
                value_type: "string".into(),
                value: json!("https://api.at-start.example.com"),
                description: "Native API base URL".into(),
            }],
        },
    )
    .await
    .unwrap();
    let service = ApplicationPublishedRunService::new(repository.clone());

    let result = service
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token,
            request: native_request("streaming", None),
        })
        .await
        .unwrap();
    ApplicationRepository::replace_application_environment_variables(
        &repository,
        &ReplaceApplicationEnvironmentVariablesInput {
            actor_user_id: actor_user_id(),
            workspace_id: application.workspace_id,
            application_id: application.id,
            variables: vec![ApplicationEnvironmentVariableInput {
                name: "ApiBaseUrl".into(),
                value_type: "string".into(),
                value: json!("https://api.changed.example.com"),
                description: "Changed Native API base URL".into(),
            }],
        },
    )
    .await
    .unwrap();
    let flow_run = repository
        .get_flow_run(application.id, result.id)
        .await
        .unwrap()
        .expect("published flow run should be durable");

    assert_eq!(
        flow_run.input_payload["env"]["ApiBaseUrl"],
        json!("https://api.at-start.example.com")
    );
}

#[tokio::test]
async fn start_native_run_uses_expand_id_and_truncates_title() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Expanded Native User App");
    let token = issue_key(&harness, application.id).await;
    publish_runnable_application(&repository, application.id).await;
    let service = ApplicationPublishedRunService::new(repository.clone());
    let long_query = "Q".repeat(300);
    let expected_title = "Q".repeat(255);

    let result = service
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token,
            request: serde_json::from_value(json!({
                "query": long_query,
                "model": "public-model/pass-through",
                "inputs": {
                    "priority": "high"
                },
                "expand_id": "customer-alias-1",
                "response_mode": "blocking",
                "execution": {},
                "metadata": {
                    "trace_id": "trace-1"
                }
            }))
            .unwrap(),
        })
        .await
        .unwrap();
    let flow_run = repository
        .get_flow_run(application.id, result.id)
        .await
        .unwrap()
        .expect("published flow run should be durable");

    assert_eq!(flow_run.external_user.as_deref(), Some("customer-alias-1"));
    assert!(flow_run
        .external_conversation_id
        .as_deref()
        .is_some_and(|value| value.starts_with("conv_")));
    assert_eq!(flow_run.title, expected_title);
    assert_eq!(result.metadata["expand_id"], json!("customer-alias-1"));
    assert!(result.metadata.get("user_id").is_none());
}

#[tokio::test]
async fn start_native_run_replays_existing_run_for_same_idempotency_key() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Idempotent Native App");
    let token = issue_key(&harness, application.id).await;
    publish_runnable_application(&repository, application.id).await;
    let service = ApplicationPublishedRunService::new(repository.clone());

    let first = service
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token.clone(),
            request: native_request("blocking", Some("idem-1")),
        })
        .await
        .unwrap();
    let second = service
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token,
            request: native_request("blocking", Some("idem-1")),
        })
        .await
        .unwrap();

    assert_eq!(first.id, second.id);
    assert_eq!(repository.flow_run_count(), 1);
}

#[tokio::test]
async fn same_idempotency_key_cannot_replay_a_run_created_through_another_protocol() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Cross Protocol Idempotency App");
    let token = issue_key(&harness, application.id).await;
    publish_runnable_application(&repository, application.id).await;
    let service = ApplicationPublishedRunService::new(repository.clone());
    let request = native_request("blocking", Some("cross-protocol-idem"));

    service
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::OpenAiChat,
            bearer_token: token.clone(),
            request: request.clone(),
        })
        .await
        .expect("first protocol should create the run");
    let error = service
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::OpenAiResponses,
            bearer_token: token,
            request,
        })
        .await
        .expect_err("a different trusted protocol must change the fingerprint");

    assert_eq!(error, NativeRunValidationError::IdempotencyConflict);
    assert_eq!(repository.flow_run_count(), 1);
}

#[tokio::test]
async fn typed_reasoning_effort_preserves_idempotency_spelling_but_freezes_normalized_runtime() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Typed Reasoning Effort App");
    let token = issue_key(&harness, application.id).await;
    save_start_model_catalog(&repository, &application).await;
    publish_runnable_application(&repository, application.id).await;
    let service = ApplicationPublishedRunService::new(repository.clone());
    let request_with_effort = |idempotency_key: &str, effort: &str| {
        serde_json::from_value(json!({
            "query": "Summarize the incident",
            "model": "gpt-5.4",
            "execution": {
                "idempotency_key": idempotency_key,
                "model_parameters": {"reasoning": {"effort": effort}}
            }
        }))
        .expect("valid reasoning effort fixture")
    };

    let normal = service
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token.clone(),
            request: request_with_effort("normal-effort", "high"),
        })
        .await
        .unwrap();
    let spaced = service
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token.clone(),
            request: request_with_effort("spaced-effort", " high "),
        })
        .await
        .unwrap();
    let conflict = service
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token,
            request: request_with_effort("normal-effort", " high "),
        })
        .await
        .unwrap_err();

    assert_eq!(conflict, NativeRunValidationError::IdempotencyConflict);
    for run_id in [normal.id, spaced.id] {
        let run = repository
            .get_flow_run(application.id, run_id)
            .await
            .unwrap()
            .expect("typed reasoning run should be durable");
        assert_eq!(
            run.input_payload["sys"]["model_parameters"]["reasoning"]["effort"],
            json!("high")
        );
        assert!(run.input_payload["node-start"]
            .get("reasoning_effort")
            .is_none());
    }
    assert_eq!(repository.flow_run_count(), 2);
}

#[tokio::test]
async fn start_native_run_rejects_same_idempotency_key_with_different_request() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Idempotent Native App");
    let token = issue_key(&harness, application.id).await;
    publish_runnable_application(&repository, application.id).await;
    let service = ApplicationPublishedRunService::new(repository.clone());
    service
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token.clone(),
            request: native_request("blocking", Some("idem-conflict")),
        })
        .await
        .unwrap();
    let mut changed_request = native_request("blocking", Some("idem-conflict"));
    changed_request.query = "Summarize a different incident".to_string();

    let error = service
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token,
            request: changed_request,
        })
        .await
        .unwrap_err();

    assert_eq!(error, NativeRunValidationError::IdempotencyConflict);
    assert_eq!(repository.flow_run_count(), 1);
}
