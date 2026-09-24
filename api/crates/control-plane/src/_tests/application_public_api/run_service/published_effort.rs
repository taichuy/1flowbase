use super::*;
use control_plane::application_public_api::{
    compat::openai::translate_response_request,
    model_catalog::{extract_agent_model_catalog_from_start_node, SelectedLlmModelParameterError},
    protocol_translation::TranslationProtocol,
};

fn model(reasoning: serde_json::Value) -> serde_json::Value {
    json!({"id":"public-luna", "capabilities":{"reasoning":true}, "reasoning":reasoning})
}

async fn run(
    repository: &ApplicationPublicApiTestRepository,
    token: &str,
    model_id: &str,
    effort: Option<&str>,
) -> Result<serde_json::Value, NativeRunValidationError> {
    // HTTP and WebSocket Responses both consume this shared mapping entry.
    let mut body = json!({"model":model_id, "input":"hello"});
    if let Some(effort) = effort {
        body["reasoning"] = json!({"effort":effort});
    }
    let request = translate_response_request(body).unwrap().request;
    let result = ApplicationPublishedRunService::new(repository.clone())
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token.to_owned(),
            protocol: TranslationProtocol::OpenAiResponses,
            request,
        })
        .await?;
    Ok(repository
        .get_flow_run(result.application_id, result.id)
        .await
        .unwrap()
        .unwrap()
        .input_payload)
}

#[tokio::test]
async fn increment_published_effort_admission_uses_snapshot_and_preserves_explicit_intent() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Published effort admission");
    let token = issue_key(&harness, application.id).await;
    save_model_catalog(
        &repository,
        &application,
        json!([model(json!({
            "default_effort":"medium", "supported_efforts":["medium","max","ultra","custom-v2"]
        }))]),
    )
    .await;
    let publication = publish_runnable_application(&repository, application.id).await;
    let listed = extract_agent_model_catalog_from_start_node(&publication.document_snapshot);
    assert_eq!(
        listed[0]
            .reasoning
            .as_ref()
            .unwrap()
            .default_effort
            .as_deref(),
        Some("medium")
    );
    // A draft changes neither admission nor the publicly listed active snapshot.
    save_model_catalog(
        &repository,
        &application,
        json!([model(json!({
            "default_effort":"draft-only", "supported_efforts":["draft-only"]
        }))]),
    )
    .await;
    repository.reset_editor_state_read_count();
    for effort in ["max", "ultra", "custom-v2"] {
        let payload = run(&repository, &token, "public-luna", Some(effort))
            .await
            .unwrap();
        assert_eq!(payload["sys"]["requested_model_id"], "public-luna");
        assert_eq!(
            payload["sys"]["model_parameters"]["reasoning"]["effort"],
            effort
        );
    }
    let payload = run(&repository, &token, "public-luna", None).await.unwrap();
    assert_eq!(
        payload["sys"]["model_parameters"]["reasoning"]["effort"],
        "medium"
    );
    assert_eq!(
        run(&repository, &token, "public-luna", Some("draft-only"))
            .await
            .unwrap_err(),
        NativeRunValidationError::UnsupportedModelParameters(
            SelectedLlmModelParameterError::ReasoningEffortUnsupported {
                requested: "draft-only".into()
            }
        )
    );
    assert_eq!(
        run(&repository, &token, "unknown-provider-model", Some("max"))
            .await
            .unwrap_err(),
        NativeRunValidationError::UnknownModel
    );
    let alias_payload = run(
        &repository,
        &token,
        "provider/model:any-public-string",
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        alias_payload["sys"]["requested_model_id"],
        "provider/model:any-public-string"
    );
    assert_eq!(repository.flow_run_count(), 5);
    assert_eq!(repository.editor_state_read_count(), 0);
    let active = repository
        .load_active_application_publication(application.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        extract_agent_model_catalog_from_start_node(&active.document_snapshot),
        listed
    );

    // Publishing the edited configuration activates it without a code change.
    publish_runnable_application(&repository, application.id).await;
    let payload = run(&repository, &token, "public-luna", None).await.unwrap();
    assert_eq!(
        payload["sys"]["model_parameters"]["reasoning"]["effort"],
        "draft-only"
    );
    assert!(matches!(
        run(&repository, &token, "public-luna", Some("max")).await,
        Err(NativeRunValidationError::UnsupportedModelParameters(_))
    ));
}

#[tokio::test]
async fn increment_published_effort_empty_missing_and_invalid_default_fail_closed() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Published effort boundaries");
    let token = issue_key(&harness, application.id).await;
    for (reasoning, explicit, expected) in [
        (
            json!({"supported_efforts":[]}),
            Some("max"),
            Some(NativeRunValidationError::UnsupportedModelParameters(
                SelectedLlmModelParameterError::ReasoningEffortUnsupported {
                    requested: "max".into(),
                },
            )),
        ),
        (json!({"supported_efforts":[]}), None, None),
        (json!({"supported_efforts":["max"]}), None, None),
        (
            json!({"default_effort":"high","supported_efforts":["max"]}),
            None,
            Some(NativeRunValidationError::InvalidPublishedModelConfiguration),
        ),
        (
            json!({"default_effort":"high","supported_efforts":["max"]}),
            Some("max"),
            Some(NativeRunValidationError::InvalidPublishedModelConfiguration),
        ),
        (
            json!({"default_effort":" max ","supported_efforts":[" max "]}),
            None,
            Some(NativeRunValidationError::InvalidPublishedModelConfiguration),
        ),
        (
            json!({"default_effort":"max","supported_efforts":[]}),
            None,
            Some(NativeRunValidationError::InvalidPublishedModelConfiguration),
        ),
    ] {
        save_model_catalog(&repository, &application, json!([model(reasoning)])).await;
        publish_runnable_application(&repository, application.id).await;
        let before = repository.flow_run_count();
        let result = run(&repository, &token, "public-luna", explicit).await;
        if let Some(error) = expected {
            assert_eq!(result.unwrap_err(), error);
            assert_eq!(repository.flow_run_count(), before);
        } else {
            let payload = result.unwrap();
            assert!(
                payload["sys"].get("model_parameters").is_none(),
                "no default means absent effort remains absent"
            );
            let raw = control_plane::ports::ProviderTransportPayload::openai_responses(
                json!({"model":"public-luna", "input":"hello"}),
            )
            .unwrap();
            assert_eq!(
                control_plane::application_public_api::native::NativeExecutionModelParameters::seal_published_reasoning_default(&payload, raw.clone()).unwrap(),
                raw,
            );
        }
    }
    save_model_catalog(&repository, &application, json!(["plain-public-model"])).await;
    publish_runnable_application(&repository, application.id).await;
    assert_eq!(
        run(&repository, &token, "plain-public-model", Some("max"))
            .await
            .unwrap_err(),
        NativeRunValidationError::UnsupportedModelParameters(
            SelectedLlmModelParameterError::ReasoningUnsupported
        )
    );
}

#[tokio::test]
async fn increment_published_effort_does_not_infer_a_missing_public_model() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "No implicit model selection");
    let token = issue_key(&harness, application.id).await;
    save_model_catalog(
        &repository,
        &application,
        json!([model(json!({
            "default_effort":"medium", "supported_efforts":["medium"]
        }))]),
    )
    .await;
    publish_runnable_application(&repository, application.id).await;
    let request = serde_json::from_value(json!({"query":"hello"})).unwrap();
    let result = ApplicationPublishedRunService::new(repository.clone())
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token,
            protocol: TranslationProtocol::Native,
            request,
        })
        .await
        .unwrap();
    let flow_run = repository
        .get_flow_run(application.id, result.id)
        .await
        .unwrap()
        .unwrap();
    assert!(flow_run.input_payload.get("sys").is_none());
}

#[tokio::test]
async fn increment_published_default_reaches_native_wire_and_callback_configuration_digest() {
    use control_plane::application_public_api::{
        native::NativeExecutionModelParameters,
        native_tool_resume::correlate_native_responses_callback,
    };
    use control_plane::ports::{ProviderTransportAffinity, ProviderTransportPayload};

    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Frozen native default wire");
    let token = issue_key(&harness, application.id).await;
    save_model_catalog(
        &repository,
        &application,
        json!([model(json!({
            "default_effort":"medium", "supported_efforts":["medium","max","custom-v2"]
        }))]),
    )
    .await;
    publish_runnable_application(&repository, application.id).await;
    let actor = ApplicationApiKeyService::new(repository.clone())
        .authenticate_bearer_token(&token)
        .await
        .unwrap();
    let service = ApplicationPublishedRunService::new(repository.clone());
    let affinity = ProviderTransportAffinity::new(
        "actual-provider",
        "openai",
        "openai_responses",
        "provider-model",
    );

    for explicit in [None, Some("max"), Some("custom-v2")] {
        let mut body = json!({"model":"public-luna", "input":[{"role":"user","content":"hello"}],
            "reasoning":{"summary":"auto"}, "generate":false, "tools":[{"type":"function","name":"read","parameters":{"type":"object"}}]});
        if let Some(effort) = explicit {
            body["reasoning"]["effort"] = json!(effort);
        }
        let translated = translate_response_request(body.clone()).unwrap();
        let result = service
            .start_native_run(CreateNativeRunCommand {
                bearer_token: token.clone(),
                protocol: TranslationProtocol::OpenAiResponses,
                request: translated.request,
            })
            .await
            .unwrap();
        let flow = repository
            .get_flow_run(application.id, result.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            flow.input_payload["sys"]["published_reasoning_default_effort"].as_str(),
            explicit.is_none().then_some("medium")
        );
        let raw = ProviderTransportPayload::openai_responses(body.clone())
            .unwrap()
            .with_affinity(affinity.clone());
        let sealed = NativeExecutionModelParameters::seal_published_reasoning_default(
            &flow.input_payload,
            raw.clone(),
        )
        .unwrap();
        assert_eq!(
            sealed.wire_body()["reasoning"]["effort"],
            explicit.unwrap_or("medium")
        );
        assert_eq!(sealed.wire_body()["reasoning"]["summary"], "auto");
        assert_eq!(sealed.wire_body()["generate"], false);
        assert_eq!(sealed.wire_body()["input"], body["input"]);
        assert_eq!(sealed.affinity(), Some(&affinity));
        assert_eq!(
            NativeExecutionModelParameters::configuration_digest_with_published_reasoning_default(
                &flow.input_payload,
                &body,
            )
            .unwrap(),
            sealed.configuration_digest().unwrap()
        );
        assert_eq!(
            sealed.size_bytes(),
            serde_json::to_vec(sealed.wire_body()).unwrap().len()
        );
        assert_eq!(
            NativeExecutionModelParameters::seal_published_reasoning_default(
                &flow.input_payload,
                sealed.clone()
            )
            .unwrap(),
            sealed
        );
        if explicit.is_none() {
            assert_ne!(sealed.digest(), raw.digest());
        } else {
            assert_eq!(sealed, raw);
        }

        let response_id = format!("resp-{}", result.id);
        let callback = repository.seed_pending_llm_tool_callback_task(
            result.id,
            json!({
                "tool_calls":[{"id":"call-read","name":"read"}],
                "provider_metadata":{"native_response":{
                    "response_id":response_id,
                    "configuration_digest":sealed.configuration_digest().unwrap()
                }}
            }),
        );
        // A newly published default must not rewrite this original callback's frozen intent.
        save_model_catalog(
            &repository,
            &application,
            json!([model(json!({
                "default_effort":"custom-v2", "supported_efforts":["medium","max","custom-v2"]
            }))]),
        )
        .await;
        publish_runnable_application(&repository, application.id).await;
        let mut resumed = body.clone();
        resumed["input"] =
            json!([{"type":"function_call_output","call_id":"call-read","output":"done"}]);
        resumed["previous_response_id"] = json!(response_id);
        let (matched, _) = correlate_native_responses_callback(&repository, &actor, &resumed)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(matched.id, callback.id);
        let resumed_payload = NativeExecutionModelParameters::seal_published_reasoning_default(
            &flow.input_payload,
            ProviderTransportPayload::openai_responses(resumed.clone())
                .unwrap()
                .with_affinity(affinity.clone()),
        )
        .unwrap();
        assert_eq!(
            resumed_payload.configuration_digest().unwrap(),
            sealed.configuration_digest().unwrap()
        );
        assert_eq!(
            NativeExecutionModelParameters::configuration_digest_with_published_reasoning_default(
                &flow.input_payload,
                &resumed,
            )
            .unwrap(),
            resumed_payload.configuration_digest().unwrap()
        );
        assert_eq!(
            resumed_payload.wire_body()["previous_response_id"],
            response_id
        );
        assert_eq!(resumed_payload.affinity(), Some(&affinity));
        resumed["reasoning"]["effort"] = json!(if explicit == Some("max") {
            "medium"
        } else {
            "max"
        });
        let error = correlate_native_responses_callback(&repository, &actor, &resumed)
            .await
            .unwrap_err();
        assert_eq!(
            error.downcast_ref::<control_plane::errors::ControlPlaneError>(),
            Some(&control_plane::errors::ControlPlaneError::Conflict(
                "native_tool_output_configuration_mismatch"
            ))
        );
        assert!(repository.callback_resume_attempts().is_empty());
        repository.complete_callback_task_for_test(callback.id);
    }
}

#[tokio::test]
async fn increment_published_default_respects_explicit_disabled_reasoning_through_native_wire() {
    use control_plane::application_public_api::native::NativeExecutionModelParameters;
    use control_plane::ports::ProviderTransportPayload;

    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Explicit reasoning disabled");
    let token = issue_key(&harness, application.id).await;
    save_model_catalog(
        &repository,
        &application,
        json!([model(json!({
            "default_effort":"medium", "supported_efforts":["medium","max"]
        }))]),
    )
    .await;
    publish_runnable_application(&repository, application.id).await;
    let service = ApplicationPublishedRunService::new(repository.clone());

    for (mode, default) in [("disabled", None), ("enabled", Some("medium"))] {
        let request = native_request_with_model_parameters(
            "public-luna",
            json!({
                "reasoning":{"mode":mode}
            }),
        );
        let result = service
            .start_native_run(CreateNativeRunCommand {
                bearer_token: token.clone(),
                protocol: TranslationProtocol::Native,
                request,
            })
            .await
            .unwrap();
        let flow = repository
            .get_flow_run(application.id, result.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            flow.input_payload["sys"]["model_parameters"]["reasoning"]["mode"],
            mode
        );
        assert_eq!(
            flow.input_payload["sys"]["model_parameters"]["reasoning"]["effort"].as_str(),
            default
        );
        assert_eq!(
            flow.input_payload["sys"]["published_reasoning_default_effort"].as_str(),
            default
        );
        let original = ProviderTransportPayload::openai_responses(json!({
            "model":"public-luna", "input":"hello", "reasoning":{"summary":"auto"}
        }))
        .unwrap();
        let sealed = NativeExecutionModelParameters::seal_published_reasoning_default(
            &flow.input_payload,
            original.clone(),
        )
        .unwrap();
        assert_eq!(sealed.wire_body()["reasoning"]["effort"].as_str(), default);
        if mode == "disabled" {
            assert_eq!(sealed, original);
        }
    }
}
