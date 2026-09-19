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
    assert_eq!(repository.flow_run_count(), 4);
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
