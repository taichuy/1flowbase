use super::*;

#[tokio::test]
async fn native_run_with_null_model_target_keeps_model_metadata_out_of_node_input_payload() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Native Null Model Target");
    let token = issue_application_key(&harness, application.id, actor_user_id()).await;
    publish_runnable_application(
        &harness,
        &application,
        mapping_without_model_target(),
        actor_user_id(),
    )
    .await;
    let service = ApplicationNativeRunService::new(harness.repository());

    let run = service
        .create_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
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
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token,
            request: serde_json::from_value(native_request(json!("any-model"))).unwrap(),
        })
        .await.map_err(crate::_tests::application_public_api::expect_validation_error)
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
    publish_runnable_application(
        &harness,
        &first_application,
        mapping_without_model_target(),
        actor_user_id(),
    )
    .await;
    publish_runnable_application(
        &harness,
        &second_application,
        mapping_without_model_target(),
        other_user_id(),
    )
    .await;
    let service = ApplicationNativeRunService::new(harness.repository());
    let run = service
        .create_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
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
        .map_err(crate::_tests::application_public_api::expect_validation_error)
        .unwrap_err();

    assert_eq!(error, NativeRunValidationError::Forbidden);
}

#[tokio::test]
async fn native_run_read_loads_durable_published_flow_run_without_test_only_result_storage() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Durable Read Native App");
    let token = issue_application_key(&harness, application.id, actor_user_id()).await;
    publish_runnable_application(
        &harness,
        &application,
        mapping_without_model_target(),
        actor_user_id(),
    )
    .await;
    let repository = harness.repository();
    let service = ApplicationNativeRunService::new(repository.clone());
    let created = service
        .create_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
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
async fn native_run_read_resolves_provider_response_id_within_api_key_scope() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Provider Continuation App");
    let other_application = harness.seed_application(other_user_id(), "Other Continuation App");
    let token = issue_application_key(&harness, application.id, actor_user_id()).await;
    let other_token = issue_application_key(&harness, other_application.id, other_user_id()).await;
    publish_runnable_application(
        &harness,
        &application,
        mapping_without_model_target(),
        actor_user_id(),
    )
    .await;
    let repository = harness.repository();
    let service = ApplicationNativeRunService::new(repository.clone());
    let created = service
        .create_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token.clone(),
            request: serde_json::from_value(native_request(json!("any-model"))).unwrap(),
        })
        .await
        .unwrap();
    repository.seed_provider_response_id(created.id, "resp_provider_owned");

    let loaded = service
        .get_native_run_by_provider_response_id(GetNativeRunByProviderResponseIdCommand {
            bearer_token: token,
            provider_response_id: "resp_provider_owned".to_string(),
        })
        .await
        .unwrap();
    assert_eq!(loaded.id, created.id);

    let error = service
        .get_native_run_by_provider_response_id(GetNativeRunByProviderResponseIdCommand {
            bearer_token: other_token,
            provider_response_id: "resp_provider_owned".to_string(),
        })
        .await
        .map_err(crate::_tests::application_public_api::expect_validation_error)
        .unwrap_err();
    assert_eq!(error, NativeRunValidationError::NotFound);
}

#[tokio::test]
async fn native_run_cancel_verifies_ownership_and_marks_published_run_cancelled() {
    let harness = ApplicationPublicApiTestHarness::new();
    let first_application = harness.seed_application(actor_user_id(), "Cancelable Native App");
    let second_application = harness.seed_application(other_user_id(), "Other Native App");
    let first_token = issue_application_key(&harness, first_application.id, actor_user_id()).await;
    let second_token =
        issue_application_key(&harness, second_application.id, other_user_id()).await;
    publish_runnable_application(
        &harness,
        &first_application,
        mapping_without_model_target(),
        actor_user_id(),
    )
    .await;
    publish_runnable_application(
        &harness,
        &second_application,
        mapping_without_model_target(),
        other_user_id(),
    )
    .await;
    let repository = harness.repository();
    let service = ApplicationNativeRunService::new(repository.clone());
    let run = service
        .create_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: first_token.clone(),
            request: serde_json::from_value(native_request(json!("any-model"))).unwrap(),
        })
        .await
        .unwrap();
    repository.seed_flow_run_output_payload(
        run.id,
        json!({
            "answer": "canonical partial before cancellation",
            "__canonical_answer_presentation": true
        }),
    );

    let forbidden = service
        .cancel_native_run(CancelNativeRunCommand {
            bearer_token: second_token,
            run_id: run.id,
        })
        .await
        .map_err(crate::_tests::application_public_api::expect_validation_error)
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
    assert_eq!(
        cancelled.answer.as_deref(),
        Some("canonical partial before cancellation")
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
    publish_runnable_application(
        &harness,
        &application,
        mapping_without_model_target(),
        actor_user_id(),
    )
    .await;
    let created = ApplicationNativeRunService::new(repository.clone())
        .create_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
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
    let terminal_types = repository.run_event_types(created.id);
    assert_eq!(
        terminal_types
            .iter()
            .filter(|event_type| event_type.as_str() == "flow_run_cancelled")
            .count(),
        1,
        "typed cancel receipt must commit one canonical flow terminal"
    );
    assert!(
        terminal_types
            .iter()
            .all(|event_type| event_type.as_str() != "flow_cancelled"),
        "Native result projection must not create a second durable terminal owner"
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

#[test]
fn increment_effort_native_preserves_extensions_and_rejects_malformed_values() {
    for effort in [json!("max"), json!("ultra"), json!("custom-v2")] {
        let translated = translate_native_run_request(json!({"query":"hello", "execution":{"model_parameters":{"reasoning":{"effort":effort}}}})).unwrap();
        let value = serde_json::to_value(translated.request.execution).unwrap();
        assert_eq!(value["model_parameters"]["reasoning"]["effort"], effort);
    }
    for effort in [
        json!(""),
        json!(" max"),
        json!("max "),
        json!(7),
        json!(null),
        json!({}),
        json!("x".repeat(129)),
        json!("a\nb"),
    ] {
        assert!(translate_native_run_request(json!({"query":"hello", "execution":{"model_parameters":{"reasoning":{"effort":effort}}}})).is_err());
    }
}
