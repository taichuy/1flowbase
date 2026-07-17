use control_plane::application_public_api::{
    api_keys::{ApplicationApiKeyService, CreateApplicationApiKeyCommand},
    mapping::{
        ApplicationApiMappingConfig, ApplicationApiMappingInput, ApplicationApiMappingOutput,
    },
    native::{
        ApplicationNativeRunService, CancelNativeRunCommand, CreateNativeRunCommand,
        GetNativeRunCommand, NativeRunRequest, NativeRunStatus, NativeRunValidationError,
    },
    publications::{ApplicationPublicationService, PublishApplicationCommand},
    ApplicationPublicApiTestHarness,
};
use serde_json::{json, Value};
use uuid::Uuid;

fn actor_user_id() -> Uuid {
    Uuid::from_u128(0x11111111111111111111111111111111)
}

fn other_user_id() -> Uuid {
    Uuid::from_u128(0x22222222222222222222222222222222)
}

fn native_request(model: Value) -> Value {
    json!({
        "query": "Summarize the incident",
        "model": model,
        "inputs": {
            "priority": "high",
            "ticket_id": "T-100"
        },
        "history": [
            {
                "role": "user",
                "content": "The customer cannot log in."
            }
        ],
        "attachments": [
            {
                "type": "file",
                "id": "file-1",
                "name": "screenshot.png"
            }
        ],
        "conversation": {
            "id": "conversation-1",
            "user": "customer-1"
        },
        "response_mode": "blocking",
        "stream_options": {
            "include_usage": true
        },
        "execution": {
            "timeout_seconds": 30
        },
        "metadata": {
            "request_id": "req-1"
        }
    })
}

fn mapping_without_model_target() -> ApplicationApiMappingConfig {
    ApplicationApiMappingConfig {
        input: ApplicationApiMappingInput {
            query_target: "node-start.query".into(),
            model_target: None,
            inputs_target: Some("node-start".into()),
            history_target: Some("node-start.history".into()),
            attachments_target: Some("node-start.files".into()),
        },
        output: ApplicationApiMappingOutput::default(),
        extension: None,
    }
}

async fn issue_application_key(
    harness: &ApplicationPublicApiTestHarness,
    application_id: Uuid,
    owner_user_id: Uuid,
) -> String {
    ApplicationApiKeyService::new(harness.repository())
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: owner_user_id,
            application_id,
            name: "Native runner".into(),
            expires_at: None,
        })
        .await
        .unwrap()
        .token
}

async fn publish_application(
    harness: &ApplicationPublicApiTestHarness,
    application_id: Uuid,
    mapping: ApplicationApiMappingConfig,
    owner_user_id: Uuid,
) {
    ApplicationPublicationService::new(harness.repository())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: owner_user_id,
            application_id,
            mapping,
            api_enabled: true,
        })
        .await
        .unwrap();
}

#[test]
fn native_run_request_model_accepts_any_string() {
    for model in [
        "gpt-5.4-mini",
        "provider/model:2026-05-10",
        "tenant-local_model.anything",
    ] {
        let request: NativeRunRequest =
            serde_json::from_value(native_request(json!(model))).unwrap();

        assert_eq!(request.model.as_deref(), Some(model));
    }
}

#[test]
fn native_run_request_model_rejects_non_string_json_values() {
    for invalid_model in [
        json!(null),
        json!(42),
        json!(true),
        json!({ "name": "gpt" }),
        json!(["gpt"]),
    ] {
        assert!(serde_json::from_value::<NativeRunRequest>(native_request(invalid_model)).is_err());
    }
}

#[test]
fn native_run_request_validates_public_native_fields() {
    let accepted: NativeRunRequest =
        serde_json::from_value(native_request(json!("any-provider/any-model"))).unwrap();

    assert_eq!(accepted.query, "Summarize the incident");
    assert_eq!(accepted.inputs["priority"], json!("high"));
    assert_eq!(accepted.history[0]["role"], json!("user"));
    assert_eq!(accepted.attachments[0]["id"], json!("file-1"));
    assert_eq!(accepted.conversation["id"], json!("conversation-1"));
    assert_eq!(accepted.response_mode.as_deref(), Some("blocking"));
    assert_eq!(accepted.stream_options["include_usage"], json!(true));
    assert_eq!(accepted.execution["timeout_seconds"], json!(30));
    assert_eq!(accepted.metadata["request_id"], json!("req-1"));
}

#[test]
fn d1_ac_009_native_run_request_rejects_unknown_fields_before_execution() {
    let mut payload = native_request(json!("any-provider/any-model"));
    payload["unrecognized_native_option"] = json!(true);

    assert!(
        serde_json::from_value::<NativeRunRequest>(payload).is_err(),
        "D1-AC-009: Native ingress must reject an unknown field instead of silently dropping it"
    );
}

#[test]
fn native_run_request_accepts_expand_id_and_title() {
    let mut payload = native_request(json!("any-provider/any-model"));
    payload["expand_id"] = json!("external-user-123");
    payload["title"] = json!("Quarterly support escalation");

    let accepted: NativeRunRequest = serde_json::from_value(payload).unwrap();

    assert_eq!(accepted.expand_id.as_deref(), Some("external-user-123"));
    assert_eq!(
        accepted.title.as_deref(),
        Some("Quarterly support escalation")
    );
}

#[test]
fn native_run_request_rejects_invalid_public_native_fields() {
    for (field, invalid_value) in [
        ("query", json!(false)),
        ("inputs", json!("not-object")),
        ("history", json!({ "role": "user" })),
        ("attachments", json!({ "id": "file-1" })),
        ("conversation", json!("not-object")),
        ("expand_id", json!({ "id": "external-user-123" })),
        ("response_mode", json!(["blocking"])),
        ("stream_options", json!("not-object")),
        ("execution", json!("not-object")),
        ("metadata", json!("not-object")),
        ("title", json!(["Quarterly support escalation"])),
    ] {
        let mut payload = native_request(json!("any-model"));
        payload[field] = invalid_value;

        assert!(
            serde_json::from_value::<NativeRunRequest>(payload).is_err(),
            "{field} should reject invalid JSON shape"
        );
    }
}

#[test]
fn native_run_request_rejects_legacy_user_id_field() {
    let mut payload = native_request(json!("any-provider/any-model"));
    payload["user_id"] = json!("external-user-123");

    assert!(serde_json::from_value::<NativeRunRequest>(payload).is_err());
}

#[tokio::test]
async fn native_run_with_null_model_target_keeps_model_metadata_out_of_node_input_payload() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Native Null Model Target");
    let token = issue_application_key(&harness, application.id, actor_user_id()).await;
    publish_application(
        &harness,
        application.id,
        mapping_without_model_target(),
        actor_user_id(),
    )
    .await;
    let service = ApplicationNativeRunService::new(harness.repository());

    let run = service
        .create_native_run(CreateNativeRunCommand {
            bearer_token: token,
            request: serde_json::from_value(native_request(json!("pass-through-model"))).unwrap(),
        })
        .await
        .unwrap();

    assert_eq!(run.metadata["model"], json!("pass-through-model"));
    assert_eq!(
        run.node_input_payload["node-start"]["query"],
        json!("Summarize the incident")
    );
    assert_eq!(
        run.node_input_payload["node-start"]["priority"],
        json!("high")
    );
    assert_eq!(
        run.node_input_payload["node-start"]["history"][0]["role"],
        json!("user")
    );
    assert_eq!(
        run.node_input_payload["node-start"]["files"][0]["id"],
        json!("file-1")
    );
    assert!(run.node_input_payload["node-start"].get("model").is_none());
}

#[tokio::test]
async fn native_run_returns_application_not_published_when_key_application_has_no_active_publication(
) {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Unpublished Native App");
    let token = issue_application_key(&harness, application.id, actor_user_id()).await;
    let service = ApplicationNativeRunService::new(harness.repository());

    let error = service
        .create_native_run(CreateNativeRunCommand {
            bearer_token: token,
            request: serde_json::from_value(native_request(json!("any-model"))).unwrap(),
        })
        .await
        .unwrap_err();

    assert_eq!(error, NativeRunValidationError::ApplicationNotPublished);
}

#[tokio::test]
async fn native_run_read_rejects_run_created_by_different_application_api_key() {
    let harness = ApplicationPublicApiTestHarness::new();
    let first_application = harness.seed_application(actor_user_id(), "First Native App");
    let second_application = harness.seed_application(other_user_id(), "Second Native App");
    let first_token = issue_application_key(&harness, first_application.id, actor_user_id()).await;
    let second_token =
        issue_application_key(&harness, second_application.id, other_user_id()).await;
    publish_application(
        &harness,
        first_application.id,
        mapping_without_model_target(),
        actor_user_id(),
    )
    .await;
    publish_application(
        &harness,
        second_application.id,
        mapping_without_model_target(),
        other_user_id(),
    )
    .await;
    let service = ApplicationNativeRunService::new(harness.repository());
    let run = service
        .create_native_run(CreateNativeRunCommand {
            bearer_token: first_token,
            request: serde_json::from_value(native_request(json!("any-model"))).unwrap(),
        })
        .await
        .unwrap();

    let error = service
        .get_native_run(GetNativeRunCommand {
            bearer_token: second_token,
            run_id: run.id,
        })
        .await
        .unwrap_err();

    assert_eq!(error, NativeRunValidationError::Forbidden);
}

#[tokio::test]
async fn native_run_read_loads_durable_published_flow_run_without_test_only_result_storage() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Durable Read Native App");
    let token = issue_application_key(&harness, application.id, actor_user_id()).await;
    publish_application(
        &harness,
        application.id,
        mapping_without_model_target(),
        actor_user_id(),
    )
    .await;
    let repository = harness.repository();
    let service = ApplicationNativeRunService::new(repository.clone());
    let created = service
        .create_native_run(CreateNativeRunCommand {
            bearer_token: token.clone(),
            request: serde_json::from_value(native_request(json!("any-model"))).unwrap(),
        })
        .await
        .unwrap();
    repository.clear_native_run_results();

    let loaded = service
        .get_native_run(GetNativeRunCommand {
            bearer_token: token,
            run_id: created.id,
        })
        .await
        .unwrap();

    assert_eq!(loaded.id, created.id);
    assert_eq!(loaded.application_id, application.id);
    assert_eq!(loaded.api_key_id, created.api_key_id);
    assert_eq!(loaded.status, NativeRunStatus::Queued);
    assert_eq!(
        loaded.node_input_payload["node-start"]["query"],
        json!("Summarize the incident")
    );
}

#[tokio::test]
async fn native_run_cancel_verifies_ownership_and_marks_published_run_cancelled() {
    let harness = ApplicationPublicApiTestHarness::new();
    let first_application = harness.seed_application(actor_user_id(), "Cancelable Native App");
    let second_application = harness.seed_application(other_user_id(), "Other Native App");
    let first_token = issue_application_key(&harness, first_application.id, actor_user_id()).await;
    let second_token =
        issue_application_key(&harness, second_application.id, other_user_id()).await;
    publish_application(
        &harness,
        first_application.id,
        mapping_without_model_target(),
        actor_user_id(),
    )
    .await;
    publish_application(
        &harness,
        second_application.id,
        mapping_without_model_target(),
        other_user_id(),
    )
    .await;
    let service = ApplicationNativeRunService::new(harness.repository());
    let run = service
        .create_native_run(CreateNativeRunCommand {
            bearer_token: first_token.clone(),
            request: serde_json::from_value(native_request(json!("any-model"))).unwrap(),
        })
        .await
        .unwrap();

    let forbidden = service
        .cancel_native_run(CancelNativeRunCommand {
            bearer_token: second_token,
            run_id: run.id,
        })
        .await
        .unwrap_err();
    assert_eq!(forbidden, NativeRunValidationError::Forbidden);

    let cancelled = service
        .cancel_native_run(CancelNativeRunCommand {
            bearer_token: first_token,
            run_id: run.id,
        })
        .await
        .unwrap();

    assert_eq!(cancelled.status, NativeRunStatus::Cancelled);
}
