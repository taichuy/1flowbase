use super::*;

#[tokio::test]
async fn native_execution_compatibility_mode_is_rejected_without_mutating_waiting_callback() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Native Forged Compat App");
    let token = issue_key(&harness, application.id).await;
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: published_mapping(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let service = ApplicationPublishedRunService::new(repository.clone());

    let first = service
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token,
            request: anthropic_request("hi"),
        })
        .await
        .unwrap();
    let callback_task = repository.seed_pending_callback_task(first.id);

    let flow_runs_before_rejection = repository.flow_run_count();
    let rejection = translate_native_run_request(json!({
        "query": "Native caller should not own Anthropic cancellation policy",
        "model": "public-model/pass-through",
        "conversation": {
            "id": "3e7058c2-3120-4222-bb14-c99ec85e1c0f",
            "user": "user_31fb5a_account__session_3e7058c2-3120-4222-bb14-c99ec85e1c0f"
        },
        "response_mode": "blocking",
        "execution": {
            "compatibility_mode": "anthropic-messages-v1"
        }
    }))
    .expect_err("Native execution compatibility_mode must be rejected by the protocol adapter");

    assert_eq!(rejection.code, "compatibility_mode");
    assert!(rejection.report.has_decision(
        "$.execution.compatibility_mode",
        control_plane::application_public_api::protocol_translation::TranslationDecisionKind::Unsupported
    ));
    assert_eq!(repository.flow_run_count(), flow_runs_before_rejection);
    let first_run = repository
        .get_flow_run(application.id, first.id)
        .await
        .unwrap()
        .expect("first run should remain durable");
    let callback_task = repository
        .get_published_callback_task(callback_task.id)
        .await
        .unwrap()
        .expect("callback task should remain durable");
    assert_eq!(first_run.status, domain::FlowRunStatus::WaitingCallback);
    assert_eq!(callback_task.status, domain::CallbackTaskStatus::Pending);
    let first_run_events = repository.run_event_types(first.id);
    assert!(!first_run_events.contains(&"public_run_cancelled".to_string()));
    assert!(!first_run_events.contains(&"public_run_callback_cancelled".to_string()));
}

#[tokio::test]
async fn start_native_run_does_not_read_editor_state_after_publication() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Frozen Native App");
    let token = issue_key(&harness, application.id).await;
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: published_mapping(),
            api_enabled: true,
        })
        .await
        .unwrap();
    repository.reset_editor_state_read_count();
    let service = ApplicationPublishedRunService::new(repository.clone());

    service
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token,
            request: native_request("streaming", None),
        })
        .await
        .unwrap();

    assert_eq!(repository.editor_state_read_count(), 0);
}

#[tokio::test]
async fn start_native_run_returns_application_not_published_for_unpublished_or_disabled_application(
) {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let unpublished = harness.seed_application(actor_user_id(), "Unpublished App");
    let unpublished_token = issue_key(&harness, unpublished.id).await;
    let disabled = harness.seed_application(actor_user_id(), "Disabled App");
    let disabled_token = issue_key(&harness, disabled.id).await;
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: disabled.id,
            mapping: published_mapping(),
            api_enabled: false,
        })
        .await
        .unwrap();
    let service = ApplicationPublishedRunService::new(repository);

    for token in [unpublished_token, disabled_token] {
        let error = service
            .start_native_run(CreateNativeRunCommand {
                bearer_token: token,
                request: native_request("blocking", None),
            })
            .await
            .unwrap_err();

        assert_eq!(error, NativeRunValidationError::ApplicationNotPublished);
    }
}
