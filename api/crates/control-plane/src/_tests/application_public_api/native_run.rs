use control_plane::application_public_api::{
    api_keys::{ApplicationApiKeyService, CreateApplicationApiKeyCommand},
    mapping::{
        ApplicationApiMappingConfig, ApplicationApiMappingInput, ApplicationApiMappingOutput,
    },
    native::{
        translate_native_run_request, ApplicationNativeRunService, CancelNativeRunCommand,
        CreateNativeRunCommand, GetNativeRunCommand, NativeRunRequest, NativeRunStatus,
        NativeRunValidationError,
    },
    protocol_translation::{
        TranslationDecisionKind, TranslationProtocol, TranslationSafeRepresentation,
    },
    publications::{ApplicationPublicationService, PublishApplicationCommand},
    run_service::{ApplicationPublishedRunControlRepository, ApplicationPublishedRunService},
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
                "source": "upload_file_id",
                "value": "file-1",
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
            "trace_id": "trace-native-1"
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
    assert_eq!(accepted.attachments[0].value, "file-1");
    assert_eq!(accepted.conversation["id"], json!("conversation-1"));
    assert_eq!(accepted.response_mode.as_deref(), Some("blocking"));
    assert_eq!(accepted.stream_options["include_usage"], json!(true));
    assert_eq!(accepted.execution["timeout_seconds"], json!(30));
    assert_eq!(accepted.metadata.trace_id(), Some("trace-native-1"));
}

#[test]
fn d2_ac_001_native_adapter_records_supported_and_defaulted_fields_without_request_copy() {
    let sentinel = "D2-NATIVE-SENTINEL-MUST-NOT-REACH-RECEIPT";
    let translated = translate_native_run_request(json!({
        "query": sentinel,
        "inputs": { "priority": "high" }
    }))
    .expect("Native public fields should translate into the canonical request");

    assert_eq!(translated.report.protocol, TranslationProtocol::Native);
    assert!(translated
        .report
        .has_decision("$.query", TranslationDecisionKind::Exact));
    assert!(translated
        .report
        .has_decision("$.inputs", TranslationDecisionKind::Exact));
    assert!(translated
        .report
        .has_decision("$.model", TranslationDecisionKind::Defaulted));
    assert!(!serde_json::to_string(&translated.report)
        .expect("receipt should serialize")
        .contains(sentinel));
}

#[test]
fn d2_ac_001_native_adapter_rejects_unknown_and_legacy_capability_fields_with_receipts() {
    let mut unknown = native_request(json!("any-provider/any-model"));
    unknown["unrecognized_native_option"] = json!(true);
    let unknown_error = translate_native_run_request(unknown)
        .expect_err("unknown Native fields must not reach run creation");
    assert!(unknown_error
        .report
        .has_decision("$.<unknown>[0]", TranslationDecisionKind::Rejected));

    let mut legacy_capability = native_request(json!("any-provider/any-model"));
    legacy_capability["compatibility_mode"] = json!("native-v1");
    let capability_error = translate_native_run_request(legacy_capability)
        .expect_err("legacy contract modes have no Native canonical owner");
    assert!(capability_error
        .report
        .has_decision("$.compatibility_mode", TranslationDecisionKind::Unsupported));
}

#[test]
fn d2_ac_001_native_nested_history_is_validated_before_canonical_mapping() {
    let sentinel = "D2-NATIVE-RAW-HISTORY-MUST-NOT-REACH-CANONICAL";
    let error = translate_native_run_request(json!({
        "query": "hello",
        "history": [{
            "role": "user",
            "content": "prior turn",
            "raw_provider_body": sentinel
        }]
    }))
    .expect_err("untyped Native history fields must not be copied into mapping input");

    let decisions = error
        .report
        .decisions
        .iter()
        .filter(|decision| decision.source_path == "$.history[0].<unknown>[0]")
        .collect::<Vec<_>>();
    assert_eq!(decisions.len(), 1);
    assert_eq!(decisions[0].kind, TranslationDecisionKind::Rejected);
    assert!(
        !serde_json::to_string(&error.report)
            .expect("receipt should serialize")
            .contains(sentinel),
        "receipt must not retain the raw nested sentinel"
    );
}

#[test]
fn d2_ac_001_native_system_block_unknown_fields_have_a_safe_nested_receipt() {
    let sentinel = "D2-NATIVE-RAW-SYSTEM-BLOCK-MUST-NOT-REACH-CANONICAL";
    let error = translate_native_run_request(json!({
        "query": "hello",
        "system": [{
            "type": "text",
            "text": "Follow the runbook.",
            "raw_provider_body": sentinel
        }]
    }))
    .expect_err("unknown Native system-block fields must be rejected before canonical prompts");

    assert!(error.report.has_decision(
        "$.system[0].<unknown>[0]",
        TranslationDecisionKind::Rejected
    ));
    assert!(
        !serde_json::to_string(&error.report)
            .expect("receipt should serialize")
            .contains(sentinel),
        "receipt must not retain raw system-block input"
    );
}

#[test]
fn d2_ac_001_native_valid_system_prompt_block_records_each_nested_leaf_once() {
    let sentinel = "D2-NATIVE-SYSTEM-PROMPT-MUST-NOT-REACH-RECEIPT";
    let translated = translate_native_run_request(json!({
        "query": "hello",
        "system": [{
            "type": "text",
            "text": sentinel,
            "cache_control": { "type": "ephemeral", "ttl": "1h" }
        }]
    }))
    .expect("a valid typed Native system prompt block should translate");

    for source_path in [
        "$.system",
        "$.system[0].type",
        "$.system[0].text",
        "$.system[0].cache_control",
        "$.system[0].cache_control.type",
        "$.system[0].cache_control.ttl",
    ] {
        assert_eq!(
            translated
                .report
                .decisions
                .iter()
                .filter(|decision| decision.source_path == source_path)
                .count(),
            1,
            "{source_path} must have exactly one TranslationDecision"
        );
    }
    assert!(
        !serde_json::to_string(&translated.report)
            .expect("receipt should serialize")
            .contains(sentinel),
        "the receipt must not retain the valid prompt text"
    );
}

#[test]
fn d2_ac_001_native_system_cache_ttl_default_has_its_own_receipt() {
    let translated = translate_native_run_request(json!({
        "query": "hello",
        "system": [{
            "type": "text",
            "text": "Follow the runbook.",
            "cache_control": { "type": "ephemeral" }
        }]
    }))
    .expect("a cache-control TTL may use the Native default");

    let decisions = translated
        .report
        .decisions
        .iter()
        .filter(|decision| decision.source_path == "$.system[0].cache_control.ttl")
        .collect::<Vec<_>>();
    assert_eq!(decisions.len(), 1, "default TTL needs one receipt");
    assert_eq!(decisions[0].kind, TranslationDecisionKind::Defaulted);
    assert_eq!(
        decisions[0].effective_value,
        TranslationSafeRepresentation::Defaulted
    );
}

#[test]
fn d2_f1_native_system_cache_control_default_has_its_own_receipt() {
    let translated = translate_native_run_request(json!({
        "query": "hello",
        "system": [{
            "type": "text",
            "text": "Follow the runbook."
        }]
    }))
    .expect("a system text block may omit cache_control");

    let decisions = translated
        .report
        .decisions
        .iter()
        .filter(|decision| decision.source_path == "$.system[0].cache_control")
        .collect::<Vec<_>>();
    assert_eq!(
        decisions.len(),
        1,
        "missing cache_control needs one receipt"
    );
    assert_eq!(decisions[0].kind, TranslationDecisionKind::Defaulted);
    assert_eq!(
        decisions[0].effective_value,
        TranslationSafeRepresentation::Defaulted
    );
}

#[test]
fn d2_ac_001_native_invalid_system_prompt_leaves_preserve_wire_presence() {
    let cases = [
        (
            json!({
                "query": "hello",
                "system": [{"text": "Follow the runbook."}]
            }),
            "$.system[0].type",
            TranslationSafeRepresentation::Absent,
        ),
        (
            json!({
                "query": "hello",
                "system": [{"type": false, "text": "Follow the runbook."}]
            }),
            "$.system[0].type",
            TranslationSafeRepresentation::Present,
        ),
        (
            json!({
                "query": "hello",
                "system": [{"type": "text"}]
            }),
            "$.system[0].text",
            TranslationSafeRepresentation::Absent,
        ),
        (
            json!({
                "query": "hello",
                "system": [{"type": "text", "text": false}]
            }),
            "$.system[0].text",
            TranslationSafeRepresentation::Present,
        ),
        (
            json!({
                "query": "hello",
                "system": [{
                    "type": "text",
                    "text": "Follow the runbook.",
                    "cache_control": false
                }]
            }),
            "$.system[0].cache_control",
            TranslationSafeRepresentation::Present,
        ),
        (
            json!({
                "query": "hello",
                "system": [{
                    "type": "text",
                    "text": "Follow the runbook.",
                    "cache_control": {}
                }]
            }),
            "$.system[0].cache_control.type",
            TranslationSafeRepresentation::Absent,
        ),
        (
            json!({
                "query": "hello",
                "system": [{
                    "type": "text",
                    "text": "Follow the runbook.",
                    "cache_control": {"type": false}
                }]
            }),
            "$.system[0].cache_control.type",
            TranslationSafeRepresentation::Present,
        ),
        (
            json!({
                "query": "hello",
                "system": [{
                    "type": "text",
                    "text": "Follow the runbook.",
                    "cache_control": {"type": "ephemeral", "ttl": false}
                }]
            }),
            "$.system[0].cache_control.ttl",
            TranslationSafeRepresentation::Present,
        ),
    ];

    for (request, source_path, effective_value) in cases {
        let error = translate_native_run_request(request)
            .expect_err("invalid Native prompt leaves must be rejected by the adapter");
        let decisions = error
            .report
            .decisions
            .iter()
            .filter(|decision| decision.source_path == source_path)
            .collect::<Vec<_>>();
        assert_eq!(decisions.len(), 1, "{source_path} needs one final receipt");
        assert_eq!(decisions[0].kind, TranslationDecisionKind::Rejected);
        assert_eq!(decisions[0].effective_value, effective_value);
    }
}

#[test]
fn d2_ac_001_native_model_parameter_leaves_have_one_safe_canonical_receipt() {
    let translated = translate_native_run_request(json!({
        "query": "hello",
        "execution": {
            "model_parameters": {
                "max_output_tokens": 4096,
                "reasoning": {
                    "enabled": true,
                    "effort": " high ",
                    "budget_tokens": 2048
                }
            }
        }
    }))
    .expect("a complete typed model-parameter request should translate");

    for (source_path, target_path, kind) in [
        (
            "$.execution.model_parameters",
            "$.execution.model_parameters",
            TranslationDecisionKind::Exact,
        ),
        (
            "$.execution.model_parameters.max_output_tokens",
            "$.execution.model_parameters.max_output_tokens",
            TranslationDecisionKind::Exact,
        ),
        (
            "$.execution.model_parameters.reasoning",
            "$.execution.model_parameters.reasoning",
            TranslationDecisionKind::Exact,
        ),
        (
            "$.execution.model_parameters.reasoning.enabled",
            "$.execution.model_parameters.reasoning.enabled",
            TranslationDecisionKind::Exact,
        ),
        (
            "$.execution.model_parameters.reasoning.effort",
            "$.execution.model_parameters.reasoning.effort",
            TranslationDecisionKind::Normalized,
        ),
        (
            "$.execution.model_parameters.reasoning.budget_tokens",
            "$.execution.model_parameters.reasoning.budget_tokens",
            TranslationDecisionKind::Exact,
        ),
    ] {
        let decisions = translated
            .report
            .decisions
            .iter()
            .filter(|decision| decision.source_path == source_path)
            .collect::<Vec<_>>();
        assert_eq!(decisions.len(), 1, "{source_path} needs one final receipt");
        assert_eq!(decisions[0].target_path.as_deref(), Some(target_path));
        assert_eq!(decisions[0].kind, kind);
        assert!(
            !decisions[0]
                .target_path
                .as_deref()
                .is_some_and(|target| target.contains(".sys") || target.contains("node-start")),
            "adapter receipts must point at the canonical Native execution path"
        );
    }
    assert_eq!(
        serde_json::to_value(&translated.request).expect("typed request should serialize")
            ["execution"]["model_parameters"],
        json!({
            "max_output_tokens": 4096,
            "reasoning": {
                "enabled": true,
                "effort": "high",
                "budget_tokens": 2048
            }
        })
    );
}

#[test]
fn d2_ac_001_native_model_parameter_shape_errors_are_safe_and_specific() {
    let sentinel = "D2-NATIVE-MODEL-PARAMETER-SENTINEL-MUST-NOT-REACH-RECEIPT";
    let cases = [
        (
            json!({"max_output_tokens": 0}),
            "$.execution.model_parameters.max_output_tokens",
            TranslationSafeRepresentation::Present,
        ),
        (
            json!({"reasoning": {"budget_tokens": 0}}),
            "$.execution.model_parameters.reasoning.budget_tokens",
            TranslationSafeRepresentation::Present,
        ),
        (
            json!({"reasoning": {"effort": "turbo"}}),
            "$.execution.model_parameters.reasoning.effort",
            TranslationSafeRepresentation::Present,
        ),
        (
            json!({"reasoning": {"raw_provider_body": sentinel}}),
            "$.execution.model_parameters.reasoning.<unknown>[0]",
            TranslationSafeRepresentation::Present,
        ),
        (
            json!({"raw_provider_body": sentinel}),
            "$.execution.model_parameters.<unknown>[0]",
            TranslationSafeRepresentation::Present,
        ),
    ];

    for (model_parameters, source_path, effective_value) in cases {
        let error = translate_native_run_request(json!({
            "query": "hello",
            "execution": {"model_parameters": model_parameters}
        }))
        .expect_err("invalid Native model parameters must fail in the adapter");
        assert_eq!(error.code, "invalid_model_parameters");
        let decisions = error
            .report
            .decisions
            .iter()
            .filter(|decision| decision.source_path == source_path)
            .collect::<Vec<_>>();
        assert_eq!(decisions.len(), 1, "{source_path} needs one final receipt");
        assert_eq!(decisions[0].kind, TranslationDecisionKind::Rejected);
        assert_eq!(decisions[0].effective_value, effective_value);
        assert!(
            !serde_json::to_string(&error.report)
                .expect("receipt should serialize")
                .contains(sentinel),
            "a rejected model-parameter value must not become receipt content"
        );
    }

    let translated = translate_native_run_request(json!({
        "query": "hello",
        "execution": {"model_parameters": {"reasoning": {}}}
    }))
    .expect("a missing reasoning.enabled value uses the documented default");
    let decisions = translated
        .report
        .decisions
        .iter()
        .filter(|decision| decision.source_path == "$.execution.model_parameters.reasoning.enabled")
        .collect::<Vec<_>>();
    assert_eq!(decisions.len(), 1, "missing enabled needs one receipt");
    assert_eq!(decisions[0].kind, TranslationDecisionKind::Defaulted);
    assert_eq!(
        decisions[0].effective_value,
        TranslationSafeRepresentation::Defaulted
    );
}

#[test]
fn d2_ac_003_native_unknown_model_parameter_keys_are_redacted_from_receipts() {
    let sentinels = [
        "D2-NATIVE-UNKNOWN-KEY-ALPHA-MUST-NOT-REACH-RECEIPT",
        "D2-NATIVE-UNKNOWN-KEY-BETA-MUST-NOT-REACH-RECEIPT",
    ];
    let mut unknown_model_parameter = serde_json::Map::new();
    for sentinel in sentinels.iter().rev() {
        unknown_model_parameter.insert((*sentinel).to_string(), json!("ignored"));
    }
    let model_error = translate_native_run_request(json!({
        "query": "hello",
        "execution": {"model_parameters": unknown_model_parameter}
    }))
    .expect_err("an unknown model-parameter key must fail at the Native adapter");

    let mut unknown_reasoning_parameter = serde_json::Map::new();
    for sentinel in sentinels.iter().rev() {
        unknown_reasoning_parameter.insert((*sentinel).to_string(), json!("ignored"));
    }
    let reasoning_error = translate_native_run_request(json!({
        "query": "hello",
        "execution": {
            "model_parameters": {"reasoning": unknown_reasoning_parameter}
        }
    }))
    .expect_err("an unknown reasoning key must fail at the Native adapter");

    for (error, source_path_prefix, source_paths) in [
        (
            model_error,
            "$.execution.model_parameters.<unknown>",
            [
                "$.execution.model_parameters.<unknown>[0]",
                "$.execution.model_parameters.<unknown>[1]",
            ],
        ),
        (
            reasoning_error,
            "$.execution.model_parameters.reasoning.<unknown>",
            [
                "$.execution.model_parameters.reasoning.<unknown>[0]",
                "$.execution.model_parameters.reasoning.<unknown>[1]",
            ],
        ),
    ] {
        assert_eq!(error.code, "invalid_model_parameters");
        let unknown_decisions = error
            .report
            .decisions
            .iter()
            .filter(|decision| decision.source_path.starts_with(source_path_prefix))
            .collect::<Vec<_>>();
        assert_eq!(unknown_decisions.len(), 2);
        assert_eq!(
            unknown_decisions
                .iter()
                .map(|decision| decision.source_path.as_str())
                .collect::<Vec<_>>(),
            source_paths
        );
        assert!(unknown_decisions
            .iter()
            .all(|decision| decision.kind == TranslationDecisionKind::Rejected));
        let serialized = serde_json::to_string(&error.report).expect("receipt should serialize");
        assert!(
            sentinels
                .iter()
                .all(|sentinel| !serialized.contains(sentinel)),
            "an unknown request key must not become receipt content"
        );
    }
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
        run.node_input_payload["node-start"]["files"][0]["value"],
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
    assert!(
        cancelled.answer.is_none(),
        "cancelled runs never expose an Answer"
    );
    let error = cancelled
        .error
        .expect("cancelled runs expose a canonical safe cancellation error");
    assert_eq!(error.code, "cancelled");
    assert_eq!(error.message, "published run cancelled");
}

#[tokio::test]
async fn native_run_cancel_cas_miss_reloads_durable_winner_without_second_public_cancellation_event(
) {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Cancel CAS Winner Native App");
    let token = issue_application_key(&harness, application.id, actor_user_id()).await;
    publish_application(
        &harness,
        application.id,
        mapping_without_model_target(),
        actor_user_id(),
    )
    .await;
    let created = ApplicationNativeRunService::new(repository.clone())
        .create_native_run(CreateNativeRunCommand {
            bearer_token: token.clone(),
            request: serde_json::from_value(native_request(json!("any-model"))).unwrap(),
        })
        .await
        .unwrap();
    let actor = ApplicationApiKeyService::new(repository.clone())
        .authenticate_bearer_token(&token)
        .await
        .unwrap();
    let stale = repository
        .get_published_flow_run(created.id)
        .await
        .unwrap()
        .expect("created Native run must be durable");
    let service = ApplicationPublishedRunService::new(repository.clone());

    let winner = service.cancel_published_run(&actor, &stale).await.unwrap();
    let loser = service.cancel_published_run(&actor, &stale).await.unwrap();

    assert_eq!(winner.status, domain::FlowRunStatus::Cancelled);
    assert_eq!(
        loser.status,
        domain::FlowRunStatus::Cancelled,
        "CAS miss must reload the durable terminal winner instead of returning the stale snapshot"
    );
    assert_eq!(
        repository
            .run_event_types(created.id)
            .into_iter()
            .filter(|event_type| event_type == "public_run_cancelled")
            .count(),
        1,
        "CAS miss must not append a second public cancellation event"
    );
}

#[test]
fn d2_f1_native_execution_and_system_rejections_decide_the_typed_container_once() {
    let execution_error = translate_native_run_request(json!({
        "query": "hello",
        "execution": {"model_parameters": {"max_output_tokens": 0}}
    }))
    .expect_err("invalid model parameters must reject the execution container");
    for source_path in [
        "$.execution",
        "$.execution.model_parameters.max_output_tokens",
    ] {
        let decisions = execution_error
            .report
            .decisions
            .iter()
            .filter(|decision| decision.source_path == source_path)
            .collect::<Vec<_>>();
        assert_eq!(decisions.len(), 1, "{source_path} needs one receipt");
        assert_eq!(decisions[0].kind, TranslationDecisionKind::Rejected);
    }

    let system_error = translate_native_run_request(json!({
        "query": "hello",
        "system": [{"type": "text", "text": false}]
    }))
    .expect_err("invalid prompt blocks must reject the system container");
    for source_path in ["$.system", "$.system[0].text"] {
        let decisions = system_error
            .report
            .decisions
            .iter()
            .filter(|decision| decision.source_path == source_path)
            .collect::<Vec<_>>();
        assert_eq!(decisions.len(), 1, "{source_path} needs one receipt");
        assert_eq!(decisions[0].kind, TranslationDecisionKind::Rejected);
    }
}

#[test]
fn d2_f1_native_idempotency_key_is_typed_and_unknown_defined_keys_are_anonymous() {
    let translated = translate_native_run_request(json!({
        "query": "hello",
        "execution": {"idempotency_key": "request-42"}
    }))
    .expect("a textual idempotency key must remain available to the run service");
    assert_eq!(
        serde_json::to_value(&translated.request).expect("request serializes")["execution"]
            ["idempotency_key"],
        json!("request-42")
    );
    let key_decisions = translated
        .report
        .decisions
        .iter()
        .filter(|decision| decision.source_path == "$.execution.idempotency_key")
        .collect::<Vec<_>>();
    assert_eq!(key_decisions.len(), 1);
    assert_eq!(key_decisions[0].kind, TranslationDecisionKind::Exact);

    let malformed = translate_native_run_request(json!({
        "query": "hello",
        "execution": {"idempotency_key": 42}
    }))
    .expect_err("a non-text idempotency key must not disable idempotency silently");
    let malformed_decisions = malformed
        .report
        .decisions
        .iter()
        .filter(|decision| decision.source_path == "$.execution.idempotency_key")
        .collect::<Vec<_>>();
    assert_eq!(malformed_decisions.len(), 1);
    assert_eq!(
        malformed_decisions[0].kind,
        TranslationDecisionKind::Rejected
    );
    assert_eq!(
        malformed_decisions[0].effective_value,
        TranslationSafeRepresentation::Present
    );

    let alpha = "D2-F1-NATIVE-UNKNOWN-KEY-ALPHA";
    let beta = "D2-F1-NATIVE-UNKNOWN-KEY-BETA";
    let unknown = translate_native_run_request(json!({
        "query": "hello",
        alpha: true,
        beta: false
    }))
    .expect_err("all unknown Native root keys must be rejected without being disclosed");
    let unknown_paths = unknown
        .report
        .decisions
        .iter()
        .filter(|decision| decision.source_path.starts_with("$.<unknown>"))
        .map(|decision| decision.source_path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(unknown_paths, ["$.<unknown>[0]", "$.<unknown>[1]"]);
    let serialized = serde_json::to_string(&unknown.report).expect("receipt serializes");
    assert!(!serialized.contains(alpha));
    assert!(!serialized.contains(beta));
}

#[test]
fn d2_f1_native_execution_compatibility_mode_is_unsupported_before_fingerprint_or_metadata() {
    let sentinel = "D2-F1-EXECUTION-COMPATIBILITY-MODE-MUST-NOT-ESCAPE";
    let error = translate_native_run_request(json!({
        "query": "hello",
        "execution": {"compatibility_mode": sentinel}
    }))
    .expect_err("execution compatibility mode has no canonical Native owner");
    let decisions = error
        .report
        .decisions
        .iter()
        .filter(|decision| decision.source_path == "$.execution.compatibility_mode")
        .collect::<Vec<_>>();
    assert_eq!(decisions.len(), 1);
    assert_eq!(decisions[0].kind, TranslationDecisionKind::Unsupported);
    assert!(
        !serde_json::to_string(&error.report)
            .expect("receipt serializes")
            .contains(sentinel),
        "the rejected mode cannot reach response metadata or an idempotency fingerprint"
    );
    assert!(
        serde_json::from_value::<NativeRunRequest>(json!({
            "query": "hello",
            "execution": {"compatibility_mode": sentinel}
        }))
        .is_err(),
        "direct Native request deserialization must not construct a request that can be fingerprinted"
    );
}

#[test]
fn d2_f1_native_metadata_has_one_typed_owner_before_fingerprint_or_response_echo() {
    let trace_id = "trace-native-typed-1";
    let translated = translate_native_run_request(json!({
        "query": "hello",
        "metadata": {"trace_id": trace_id}
    }))
    .expect("a typed Native trace_id should translate");

    assert_eq!(
        translated.request.metadata.as_value(),
        json!({"trace_id": trace_id})
    );
    let serialized_request =
        serde_json::to_value(&translated.request).expect("canonical request serializes");
    assert_eq!(
        serialized_request["metadata"],
        json!({"trace_id": trace_id})
    );
    let decisions = translated
        .report
        .decisions
        .iter()
        .filter(|decision| decision.source_path == "$.metadata.trace_id")
        .collect::<Vec<_>>();
    assert_eq!(decisions.len(), 1, "trace_id needs one receipt");
    assert_eq!(decisions[0].kind, TranslationDecisionKind::Exact);

    let missing = translate_native_run_request(json!({"query": "hello"}))
        .expect("missing metadata should use the Native default");
    let missing_metadata = missing
        .report
        .decisions
        .iter()
        .filter(|decision| decision.source_path == "$.metadata")
        .collect::<Vec<_>>();
    assert_eq!(
        missing_metadata.len(),
        1,
        "missing metadata needs one receipt"
    );
    assert_eq!(missing_metadata[0].kind, TranslationDecisionKind::Defaulted);

    let non_string = translate_native_run_request(json!({
        "query": "hello",
        "metadata": {"trace_id": false}
    }))
    .expect_err("a non-string trace_id must not become canonical metadata");
    let rejected_trace = non_string
        .report
        .decisions
        .iter()
        .filter(|decision| decision.source_path == "$.metadata.trace_id")
        .collect::<Vec<_>>();
    assert_eq!(
        rejected_trace.len(),
        1,
        "rejected trace_id needs one receipt"
    );
    assert_eq!(rejected_trace[0].kind, TranslationDecisionKind::Rejected);
    assert_eq!(
        rejected_trace[0].effective_value,
        TranslationSafeRepresentation::Present
    );

    let non_object = translate_native_run_request(json!({
        "query": "hello",
        "metadata": false
    }))
    .expect_err("non-object metadata must not become canonical metadata");
    assert!(non_object
        .report
        .has_decision("$.metadata", TranslationDecisionKind::Rejected));
}

#[test]
fn d2_f1_native_unknown_metadata_cannot_form_canonical_or_fingerprint_input() {
    let alpha = "D2-F1-NATIVE-METADATA-SECRET-ALPHA";
    let beta = "D2-F1-NATIVE-METADATA-SECRET-BETA";
    let request = json!({
        "query": "hello",
        "execution": {"idempotency_key": "same-request"},
        "metadata": {
            "trace_id": "trace-native-typed-1",
            alpha: "secret-a",
            beta: "secret-b"
        }
    });
    let error = translate_native_run_request(request.clone())
        .expect_err("unknown metadata must not reach canonical request, fingerprint, or response");

    let unknown_paths = error
        .report
        .decisions
        .iter()
        .filter(|decision| decision.source_path.starts_with("$.metadata.<unknown>"))
        .map(|decision| decision.source_path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        unknown_paths,
        ["$.metadata.<unknown>[0]", "$.metadata.<unknown>[1]"]
    );
    assert!(error
        .report
        .has_decision("$.metadata", TranslationDecisionKind::Rejected));
    let serialized_report = serde_json::to_string(&error.report).expect("receipt serializes");
    assert!(!serialized_report.contains(alpha));
    assert!(!serialized_report.contains(beta));
    assert!(
        serde_json::from_value::<NativeRunRequest>(request).is_err(),
        "direct Native deserialization must not construct a request that can feed a fingerprint"
    );
}

#[test]
fn d2_f1_native_defined_container_receipts_remain_unique_on_nested_rejection() {
    let history_error = translate_native_run_request(json!({
        "query": "hello",
        "history": [{"role": false, "content": "earlier"}]
    }))
    .expect_err("a malformed history field rejects its defined containers");
    for (source_path, kind) in [
        ("$.history", TranslationDecisionKind::Normalized),
        ("$.history[0]", TranslationDecisionKind::Normalized),
        ("$.history[0].role", TranslationDecisionKind::Rejected),
    ] {
        let decisions = history_error
            .report
            .decisions
            .iter()
            .filter(|decision| decision.source_path == source_path)
            .collect::<Vec<_>>();
        assert_eq!(decisions.len(), 1, "{source_path} needs one final receipt");
        assert_eq!(decisions[0].kind, kind);
    }

    let attachment_error = translate_native_run_request(json!({
        "query": "hello",
        "attachments": [{"source": false, "value": "file-1"}]
    }))
    .expect_err("a malformed attachment field rejects its defined containers");
    for (source_path, kind) in [
        ("$.attachments", TranslationDecisionKind::Normalized),
        ("$.attachments[0]", TranslationDecisionKind::Normalized),
        ("$.attachments[0].source", TranslationDecisionKind::Rejected),
    ] {
        let decisions = attachment_error
            .report
            .decisions
            .iter()
            .filter(|decision| decision.source_path == source_path)
            .collect::<Vec<_>>();
        assert_eq!(decisions.len(), 1, "{source_path} needs one final receipt");
        assert_eq!(decisions[0].kind, kind);
    }

    let system_error = translate_native_run_request(json!({
        "query": "hello",
        "system": [{
            "type": "text",
            "text": "Use the runbook.",
            "cache_control": {"type": false}
        }]
    }))
    .expect_err("a malformed system cache rejects the system and its typed containers");
    for source_path in [
        "$.system",
        "$.system[0]",
        "$.system[0].cache_control",
        "$.system[0].cache_control.type",
    ] {
        let decisions = system_error
            .report
            .decisions
            .iter()
            .filter(|decision| decision.source_path == source_path)
            .collect::<Vec<_>>();
        assert_eq!(decisions.len(), 1, "{source_path} needs one final receipt");
        assert_eq!(decisions[0].kind, TranslationDecisionKind::Rejected);
    }

    let execution_error = translate_native_run_request(json!({
        "query": "hello",
        "execution": {"model_parameters": {"reasoning": {"budget_tokens": 0}}}
    }))
    .expect_err("a malformed reasoning leaf rejects every typed execution container");
    for source_path in [
        "$.execution",
        "$.execution.model_parameters",
        "$.execution.model_parameters.reasoning",
        "$.execution.model_parameters.reasoning.budget_tokens",
    ] {
        let decisions = execution_error
            .report
            .decisions
            .iter()
            .filter(|decision| decision.source_path == source_path)
            .collect::<Vec<_>>();
        assert_eq!(decisions.len(), 1, "{source_path} needs one final receipt");
        assert_eq!(decisions[0].kind, TranslationDecisionKind::Rejected);
    }
}
