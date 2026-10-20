use super::*;

#[tokio::test]
async fn native_resume_rejects_callback_task_from_another_run() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Resume App");
    let token = issue_key(&harness, application.id, actor_user_id()).await;
    publish_runnable_application(&harness, application.id, actor_user_id()).await;
    let repository = harness.repository();
    let service = ApplicationNativeRunService::new(repository.clone());
    let first = service
        .create_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token.clone(),
            request: serde_json::from_value(json!({ "query": "First" })).unwrap(),
        })
        .await
        .unwrap();
    let second = service
        .create_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token.clone(),
            request: serde_json::from_value(json!({ "query": "Second" })).unwrap(),
        })
        .await
        .unwrap();
    let callback_task = repository.seed_pending_callback_task(second.id);

    let consumer = RecordingCallbackConsumer {
        repository: repository.clone(),
        ..RecordingCallbackConsumer::default()
    };
    let error = ApplicationPublishedCallbackResumeService::new(repository.clone(), consumer)
        .resume_callback(ResumePublishedCallbackCommand {
            responses_continuation: None,
            transport_connection_scope: None,
            observation_context: None,
            reserved_attempt_id: None,
            native_transport: None,
            bearer_token: token,
            target: PublishedCallbackResumeTarget::FlowRun {
                flow_run_id: first.id,
                callback_task_id: callback_task.id,
            },
            source: PublishedCallbackResumeSource::NativeAgent,
            response_payload: json!({ "answer": "approved" }),
            response_mode: Some("blocking".into()),
        })
        .await
        .unwrap_err();

    assert_eq!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(&ControlPlaneError::PermissionDenied(
            "callback_task_flow_run"
        ))
    );
}

#[tokio::test]
async fn native_resume_validates_ownership_before_execution_continuation_boundary() {
    let harness = ApplicationPublicApiTestHarness::new();
    let first_application = harness.seed_application(actor_user_id(), "Owned Resume App");
    let second_application = harness.seed_application(other_user_id(), "Other Resume App");
    let first_token = issue_key(&harness, first_application.id, actor_user_id()).await;
    let second_token = issue_key(&harness, second_application.id, other_user_id()).await;
    publish_runnable_application(&harness, first_application.id, actor_user_id()).await;
    publish_runnable_application(&harness, second_application.id, other_user_id()).await;
    let repository = harness.repository();
    let service = ApplicationNativeRunService::new(repository.clone());
    let run = service
        .create_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: first_token,
            request: serde_json::from_value(json!({ "query": "First" })).unwrap(),
        })
        .await
        .unwrap();
    let callback_task = repository.seed_pending_callback_task(run.id);

    let consumer = RecordingCallbackConsumer {
        repository: repository.clone(),
        ..RecordingCallbackConsumer::default()
    };
    let error = ApplicationPublishedCallbackResumeService::new(repository.clone(), consumer)
        .resume_callback(ResumePublishedCallbackCommand {
            responses_continuation: None,
            transport_connection_scope: None,
            observation_context: None,
            reserved_attempt_id: None,
            native_transport: None,
            bearer_token: second_token,
            target: PublishedCallbackResumeTarget::FlowRun {
                flow_run_id: run.id,
                callback_task_id: callback_task.id,
            },
            source: PublishedCallbackResumeSource::NativeAgent,
            response_payload: json!({ "answer": "approved" }),
            response_mode: Some("streaming".into()),
        })
        .await
        .unwrap_err();

    assert_eq!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(&ControlPlaneError::PermissionDenied(
            "application_public_callback_resume"
        ))
    );
}

#[tokio::test]
async fn native_get_run_exposes_pending_callback_required_action() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Required Action App");
    let token = issue_key(&harness, application.id, actor_user_id()).await;
    publish_runnable_application(&harness, application.id, actor_user_id()).await;
    let repository = harness.repository();
    let service = ApplicationNativeRunService::new(repository.clone());
    let run = service
        .create_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token.clone(),
            request: serde_json::from_value(json!({ "query": "First" })).unwrap(),
        })
        .await
        .unwrap();
    let callback_task = repository.seed_pending_callback_task(run.id);

    let result = service
        .get_native_run(
            control_plane::application_public_api::native::GetNativeRunCommand {
                bearer_token: token,
                run_id: run.id,
            },
        )
        .await
        .unwrap();

    assert_eq!(
        result.status,
        control_plane::application_public_api::native::NativeRunStatus::Waiting
    );
    let required_action = result
        .required_action
        .expect("pending callback should be exposed");
    assert_eq!(required_action.action_type, "callback");
    assert_eq!(
        required_action.payload["callback_task_id"],
        json!(callback_task.id)
    );
    assert_eq!(
        required_action.payload["request_payload"],
        callback_task.request_payload
    );
}

#[tokio::test]
async fn public_callback_resume_consumes_pending_callback_in_request() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Unified Resume App");
    let token = issue_key(&harness, application.id, actor_user_id()).await;
    publish_runnable_application(&harness, application.id, actor_user_id()).await;
    let repository = harness.repository();
    let native_service = ApplicationNativeRunService::new(repository.clone());
    let run = native_service
        .create_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token.clone(),
            request: serde_json::from_value(json!({ "query": "First" })).unwrap(),
        })
        .await
        .unwrap();
    let callback_task = repository.seed_pending_callback_task(run.id);
    let consumer = RecordingCallbackConsumer {
        repository: repository.clone(),
        ..RecordingCallbackConsumer::default()
    };

    let observation = control_plane_contracts::ports::WorkflowObservationContext {
        client_request_id: Uuid::now_v7(),
        context_flow_run_id: Some(run.id),
        context_response_id: Some("resp-prior-round".into()),
        is_resume: true,
    };
    let result =
        ApplicationPublishedCallbackResumeService::new(repository.clone(), consumer.clone())
            .resume_callback(ResumePublishedCallbackCommand {
                responses_continuation: None,
                transport_connection_scope: Some("host-generated-connection".into()),
                observation_context: Some(observation.clone()),
                reserved_attempt_id: None,
                native_transport: None,
                bearer_token: token,
                target: PublishedCallbackResumeTarget::FlowRun {
                    flow_run_id: run.id,
                    callback_task_id: callback_task.id,
                },
                source: PublishedCallbackResumeSource::NativeAgent,
                response_payload: json!({ "answer": "approved" }),
                response_mode: Some("blocking".into()),
            })
            .await
            .unwrap();

    let calls = consumer.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].transport_connection_scope.as_deref(),
        Some("host-generated-connection")
    );
    assert_eq!(calls[0].observation_context.as_ref(), Some(&observation));
    assert_eq!(calls[0].application_id, application.id);
    assert_eq!(calls[0].callback_task_id, callback_task.id);
    assert_eq!(calls[0].response_payload, json!({ "answer": "approved" }));
    assert_eq!(
        result.attempt.status,
        domain::FlowRunCallbackResumeAttemptStatus::Succeeded
    );

    let attempts = repository.callback_resume_attempts();
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].flow_run_id, run.id);
    assert_eq!(attempts[0].callback_task_id, callback_task.id);
    assert_eq!(attempts[0].source, "native_agent");
    assert_eq!(
        attempts[0].status,
        domain::FlowRunCallbackResumeAttemptStatus::Succeeded
    );

    let event_types = repository.run_event_types(run.id);
    assert!(event_types.contains(&"public_run_resume_requested".to_string()));
    assert!(event_types.contains(&"public_run_resume_succeeded".to_string()));
    assert!(!event_types.contains(&"public_run_resume_claimed".to_string()));
}

#[tokio::test]
async fn callback_resume_preserves_original_compatibility_mode() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Canonical Callback Resume App");
    let token = issue_key(&harness, application.id, actor_user_id()).await;
    publish_runnable_application(&harness, application.id, actor_user_id()).await;
    let repository = harness.repository();
    let run_service = ApplicationPublishedRunService::new(repository.clone());
    let agent_prompt = "Search the 1flowbase frontend for the navigation code.";

    let absent_parent = run_service
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token.clone(),
            request: anthropic_request("Find the navigation code"),
        })
        .await
        .unwrap();
    let absent_parent_callback = repository.seed_pending_llm_tool_callback_task(
        absent_parent.id,
        json!({
            "tool_calls": [{
                "id": "call_agent_absent",
                "name": "Agent",
                "arguments": {"prompt": agent_prompt}
            }]
        }),
    );
    let absent_subagent = run_service
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token.clone(),
            request: anthropic_builtin_agent_request(agent_prompt),
        })
        .await
        .unwrap();
    let absent_subagent_callback = repository.seed_pending_callback_task(absent_subagent.id);

    let legacy_parent = run_service
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token.clone(),
            request: anthropic_request("Find the navigation code"),
        })
        .await
        .unwrap();
    let legacy_parent_callback = repository.seed_pending_llm_tool_callback_task(
        legacy_parent.id,
        json!({
            "tool_calls": [{
                "id": "call_agent_legacy",
                "name": "Agent",
                "arguments": {"prompt": agent_prompt}
            }]
        }),
    );
    let legacy_subagent = run_service
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token.clone(),
            request: anthropic_builtin_agent_request(agent_prompt),
        })
        .await
        .unwrap();
    let legacy_subagent_callback = repository.seed_pending_callback_task(legacy_subagent.id);
    repository
        .set_flow_run_compatibility_mode_for_test(legacy_parent.id, Some("anthropic-messages-v1"));
    repository.set_flow_run_compatibility_mode_for_test(
        legacy_subagent.id,
        Some("anthropic-messages-v1"),
    );

    let consumer = RecordingCallbackConsumer {
        repository: repository.clone(),
        ..RecordingCallbackConsumer::default()
    };
    for (flow_run_id, callback_task_id, tool_call_id) in [
        (
            absent_parent.id,
            absent_parent_callback.id,
            "call_agent_absent",
        ),
        (
            legacy_parent.id,
            legacy_parent_callback.id,
            "call_agent_legacy",
        ),
    ] {
        ApplicationPublishedCallbackResumeService::new(repository.clone(), consumer.clone())
            .resume_callback(ResumePublishedCallbackCommand {
                responses_continuation: None,
                transport_connection_scope: None,
                observation_context: None,
                reserved_attempt_id: None,
                native_transport: None,
                bearer_token: token.clone(),
                target: PublishedCallbackResumeTarget::FlowRun {
                    flow_run_id,
                    callback_task_id,
                },
                source: PublishedCallbackResumeSource::AnthropicMessages,
                response_payload: json!({
                    "tool_results": [{
                        "tool_call_id": tool_call_id,
                        "content": "Navigation lives in web/app/src/app-shell/Navigation.tsx."
                    }]
                }),
                response_mode: Some("streaming".into()),
            })
            .await
            .unwrap();
    }

    for (subagent_id, callback_task_id) in [
        (absent_subagent.id, absent_subagent_callback.id),
        (legacy_subagent.id, legacy_subagent_callback.id),
    ] {
        let subagent = repository
            .get_flow_run(application.id, subagent_id)
            .await
            .unwrap()
            .expect("subagent run should remain durable");
        let callback_task = repository
            .get_published_callback_task(callback_task_id)
            .await
            .unwrap()
            .expect("subagent callback task should remain durable");
        assert_eq!(subagent.status, domain::FlowRunStatus::WaitingCallback);
        assert_eq!(callback_task.status, domain::CallbackTaskStatus::Pending);
        let events = repository.run_event_types(subagent_id);
        assert!(!events.contains(&"public_run_internal_agent_result_projected".to_string()));
        assert!(!events.contains(&"public_run_callback_cancelled".to_string()));
    }

    for (flow_run_id, expected) in [
        (absent_parent.id, "native-v1"),
        (legacy_parent.id, "anthropic-messages-v1"),
    ] {
        let resumed = repository
            .get_flow_run(application.id, flow_run_id)
            .await
            .unwrap()
            .expect("resumed parent run should remain durable");
        assert_eq!(resumed.compatibility_mode.as_deref(), Some(expected));
    }
}
