use control_plane::application_public_api::{
    api_keys::{ApplicationApiKeyService, CreateApplicationApiKeyCommand},
    mapping::{
        ApplicationApiMappingConfig, ApplicationApiMappingInput, ApplicationApiMappingOutput,
    },
    native::{
        translate_native_run_request, ApplicationNativeRunService, CancelNativeRunCommand,
        CreateNativeRunCommand, GetNativeRunByProviderResponseIdCommand, GetNativeRunCommand,
        NativeRunRequest, NativeRunStatus, NativeRunValidationError,
    },
    protocol_translation::{
        TranslationDecisionKind, TranslationProtocol, TranslationSafeRepresentation,
    },
    publications::{ApplicationPublicationService, PublishApplicationCommand},
    run_service::{ApplicationPublishedRunControlRepository, ApplicationPublishedRunService},
    ApplicationPublicApiTestHarness,
};
use control_plane::ports::FlowRepository;
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
            "include_workflow_events": "public"
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

async fn publish_runnable_application(
    harness: &ApplicationPublicApiTestHarness,
    application: &domain::ApplicationRecord,
    mapping: ApplicationApiMappingConfig,
    owner_user_id: Uuid,
) {
    let application_id = application.id;
    let repository = harness.repository();
    let editor_state = repository
        .get_or_create_editor_state(application.workspace_id, application_id, owner_user_id)
        .await
        .unwrap();
    let mut document = editor_state.draft.document;
    let start = document["graph"]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["type"] == "start")
        .unwrap();
    // These public aliases are independent of the compiled provider route.
    // Declare them before publication so read/cancel tests reach their target behavior.
    start["config"]["model_list"] = json!([
        {"id": "any-model"},
        {"id": "pass-through-model"}
    ]);
    FlowRepository::save_draft(
        &repository,
        application.workspace_id,
        application_id,
        owner_user_id,
        document,
        domain::FlowChangeKind::Logical,
        "Declare native run fixture models",
    )
    .await
    .unwrap();
    ApplicationPublicationService::new(harness.repository())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: owner_user_id,
            application_id,
            mapping,
            api_enabled: true,
        })
        .await
        .unwrap();
    harness
        .repository()
        .configure_runnable_published_generate_route(application_id);
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
    assert_eq!(
        accepted.stream_options.include_workflow_events,
        control_plane::application_public_api::native::NativeWorkflowEventVisibility::Public
    );
    assert_eq!(accepted.execution["timeout_seconds"], json!(30));
    assert_eq!(accepted.metadata.trace_id(), Some("trace-native-1"));
}

#[test]
fn issue_1601_native_stream_options_reject_unknown_and_accept_typed_visibility() {
    let mut unknown = native_request(json!("provider/model"));
    unknown["stream_options"] = json!({"include_usage": true});
    assert!(translate_native_run_request(unknown).is_err());

    let mut debug = native_request(json!("provider/model"));
    debug["stream_options"] = json!({"include_workflow_events": "debug"});
    let translated = translate_native_run_request(debug).expect("debug is a typed Native value");
    assert_eq!(
        translated.request.stream_options.include_workflow_events,
        control_plane::application_public_api::native::NativeWorkflowEventVisibility::Debug
    );
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
                "requested_context_window": 1000000,
                "reasoning": {
                    "mode": "adaptive",
                    "effort": "high",
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
            "$.execution.model_parameters.requested_context_window",
            "$.execution.model_parameters.requested_context_window",
            TranslationDecisionKind::Exact,
        ),
        (
            "$.execution.model_parameters.reasoning",
            "$.execution.model_parameters.reasoning",
            TranslationDecisionKind::Exact,
        ),
        (
            "$.execution.model_parameters.reasoning.mode",
            "$.execution.model_parameters.reasoning.mode",
            TranslationDecisionKind::Exact,
        ),
        (
            "$.execution.model_parameters.reasoning.effort",
            "$.execution.model_parameters.reasoning.effort",
            TranslationDecisionKind::Exact,
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
            "requested_context_window": 1000000,
            "reasoning": {
                "mode": "adaptive",
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
            json!({"requested_context_window": 0}),
            "$.execution.model_parameters.requested_context_window",
            TranslationSafeRepresentation::Present,
        ),
        (
            json!({"reasoning": {"mode": "automatic"}}),
            "$.execution.model_parameters.reasoning.mode",
            TranslationSafeRepresentation::Present,
        ),
        (
            json!({"reasoning": {"budget_tokens": 0}}),
            "$.execution.model_parameters.reasoning.budget_tokens",
            TranslationSafeRepresentation::Present,
        ),
        (
            json!({"reasoning": {"effort": " high "}}),
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
    .expect("a missing reasoning.mode value uses the documented default");
    let decisions = translated
        .report
        .decisions
        .iter()
        .filter(|decision| decision.source_path == "$.execution.model_parameters.reasoning.mode")
        .collect::<Vec<_>>();
    assert_eq!(decisions.len(), 1, "missing mode needs one receipt");
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
mod execution_and_receipts;
