use super::*;

#[tokio::test]
async fn identical_processing_native_callback_attaches_without_second_consumer() {
    use crate::application_public_api::callback_resume::{
        ApplicationPublishedCallbackResumeService, PreparedPublishedCallbackResume,
        PublishedCallbackResumeSource, PublishedCallbackResumeTarget,
        ResumePublishedCallbackCommand,
    };
    let (repository, actor, run) =
        fixture_with_input(json!({"sys":{"requested_model_id":"fixture"}})).await;
    let (callback, mut body) = seed_proven_full_round(&repository, run, true);
    let full_body = body.clone();
    body["previous_response_id"] = json!("resp_round");
    body["input"] = request()["input"].clone();
    let (_, results) = correlate_native_responses_callback(&repository, &actor, &body)
        .await
        .unwrap()
        .unwrap();
    let consumer = RecordingNativeCallbackConsumer {
        repository: repository.clone(),
        calls: Default::default(),
    };
    let service =
        ApplicationPublishedCallbackResumeService::new(repository.clone(), consumer.clone());
    let command = ResumePublishedCallbackCommand {
        responses_continuation: None,
        transport_connection_scope: None,
        observation_context: None,
        reserved_attempt_id: None,
        native_transport: Some(ProviderTransportPayload::openai_responses(body.clone()).unwrap()),
        bearer_token: String::new(),
        target: PublishedCallbackResumeTarget::CallbackTask {
            callback_task_id: callback.id,
        },
        source: PublishedCallbackResumeSource::OpenAiResponses,
        response_payload: results,
        response_mode: Some("streaming".into()),
    };
    let attempt_id = service
        .reserve_native_callback_for_actor(actor.clone(), &command)
        .await
        .unwrap();
    repository.complete_callback_task_for_test(callback.id);
    assert!(matches!(
        service
            .prepare_callback_resume_for_actor(actor.clone(), &command)
            .await
            .unwrap(),
        PreparedPublishedCallbackResume::Attach { .. }
    ));
    let mut full_retry = command.clone();
    full_retry.native_transport =
        Some(ProviderTransportPayload::openai_responses(full_body.clone()).unwrap());
    assert!(matches!(
        service
            .prepare_callback_resume_for_actor(actor.clone(), &full_retry)
            .await
            .unwrap(),
        PreparedPublishedCallbackResume::Attach { .. }
    ));
    // The attempt receipt is old, but the execution owner keeps its lease alive.
    repository.expire_callback_resume_attempt_for_test(callback.id);
    repository.set_callback_resume_claim_lease_for_test(
        callback.id,
        OffsetDateTime::now_utc() + time::Duration::minutes(5),
    );
    for retry in [&command, &full_retry] {
        assert!(matches!(
            service
                .prepare_callback_resume_for_actor(actor.clone(), retry)
                .await
                .unwrap(),
            PreparedPublishedCallbackResume::Attach { .. }
        ));
    }
    assert!(service
        .reserve_native_callback_for_actor(actor.clone(), &command)
        .await
        .is_err());
    assert!(consumer.calls.lock().unwrap().is_empty());
    let attempts = repository.callback_resume_attempts();
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].id, attempt_id);
    assert_eq!(
        attempts[0].status,
        domain::FlowRunCallbackResumeAttemptStatus::Processing
    );

    let mut conflicting = command.clone();
    conflicting.response_payload["tool_results"][0]["content"] = json!("changed");
    assert!(matches!(
        service
            .prepare_callback_resume_for_actor(actor.clone(), &conflicting)
            .await
            .unwrap_err()
            .downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::Conflict(
            "callback_resume_payload_conflict"
        ))
    ));
    conflicting = command.clone();
    conflicting.source = PublishedCallbackResumeSource::OpenAiChat;
    assert!(matches!(
        service
            .prepare_callback_resume_for_actor(actor.clone(), &conflicting)
            .await
            .unwrap_err()
            .downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::Conflict(
            "callback_resume_source_conflict"
        ))
    ));
    let mut changed_history = full_body;
    changed_history["input"][0]["content"] = json!("different predecessor");
    let stored = repository
        .get_published_callback_task(callback.id)
        .await
        .unwrap()
        .unwrap();
    let attempts = repository.callback_resume_attempts();
    assert!(
        correlate_native_responses_callback(&repository, &actor, &changed_history)
            .await
            .unwrap()
            .is_none()
    );
    conflicting = command;
    conflicting.native_transport =
        Some(ProviderTransportPayload::openai_responses(changed_history).unwrap());
    assert!(service
        .prepare_callback_resume_for_actor(actor, &conflicting)
        .await
        .is_err());
    assert!(consumer.calls.lock().unwrap().is_empty());
    assert_eq!(
        repository
            .get_published_callback_task(callback.id)
            .await
            .unwrap()
            .unwrap(),
        stored
    );
    assert_eq!(repository.callback_resume_attempts(), attempts);
}

#[tokio::test]
async fn full_context_successor_waits_for_processing_predecessor_without_replaying_tools() {
    use crate::application_public_api::callback_resume::{
        ApplicationPublishedCallbackResumeService, PreparedPublishedCallbackResume,
        PublishedCallbackResumeSource, PublishedCallbackResumeTarget,
        ResumePublishedCallbackCommand,
    };
    let (repository, actor, run) =
        fixture_with_input(json!({"sys":{"requested_model_id":"fixture"}})).await;
    let (callback, mut body) = seed_proven_full_round(&repository, run, true);
    let replay_body = body.clone();
    body["input"].as_array_mut().unwrap().push(json!({
        "role":"user", "content":"New mailbox context"
    }));
    let (admitted, results) = correlate_native_responses_callback(&repository, &actor, &body)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(admitted.id, callback.id);
    let consumer = RecordingNativeCallbackConsumer {
        repository: repository.clone(),
        calls: Default::default(),
    };
    let service =
        ApplicationPublishedCallbackResumeService::new(repository.clone(), consumer.clone());
    let mut command = ResumePublishedCallbackCommand {
        responses_continuation: None,
        transport_connection_scope: None,
        observation_context: None,
        reserved_attempt_id: None,
        native_transport: Some(ProviderTransportPayload::openai_responses(body).unwrap()),
        bearer_token: String::new(),
        target: PublishedCallbackResumeTarget::CallbackTask {
            callback_task_id: callback.id,
        },
        source: PublishedCallbackResumeSource::OpenAiResponses,
        response_payload: results,
        response_mode: Some("streaming".into()),
    };
    command.reserved_attempt_id = Some(
        service
            .reserve_native_callback_for_actor(actor.clone(), &command)
            .await
            .unwrap(),
    );

    let mut changed = command.clone();
    changed.response_payload["tool_results"][0]["content"] = json!("tampered");
    let error = service
        .prepare_callback_resume_for_actor(actor.clone(), &changed)
        .await
        .expect_err("a changed tool result must fail before waiting");
    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::Conflict(
            "callback_resume_payload_conflict"
        ))
    ));

    let waiting_service =
        ApplicationPublishedCallbackResumeService::new(repository.clone(), consumer.clone());
    let waiting_command = command.clone();
    let waiting_actor = actor.clone();
    let waiter = tokio::spawn(async move {
        waiting_service
            .prepare_callback_resume_for_actor(waiting_actor, &waiting_command)
            .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    assert!(
        !waiter.is_finished(),
        "the successor must await the durable predecessor"
    );
    assert!(consumer.calls.lock().unwrap().is_empty());

    service
        .resume_callback_for_actor(actor.clone(), command.clone())
        .await
        .unwrap();
    let admission = tokio::time::timeout(std::time::Duration::from_secs(2), waiter)
        .await
        .expect("settled predecessor must release the successor")
        .unwrap()
        .unwrap();
    assert_eq!(
        admission,
        PreparedPublishedCallbackResume::StartNewTurnFromHistory
    );

    let mut replay = command;
    replay.native_transport =
        Some(ProviderTransportPayload::openai_responses(replay_body).unwrap());
    assert!(matches!(
        service
            .prepare_callback_resume_for_actor(actor.clone(), &replay)
            .await
            .unwrap(),
        PreparedPublishedCallbackResume::Resume { .. }
    ));
    service
        .resume_callback_for_actor(actor, replay)
        .await
        .unwrap();
    assert_eq!(consumer.calls.lock().unwrap().len(), 1);
    assert_eq!(repository.callback_resume_attempts().len(), 1);
}

#[tokio::test]
async fn expired_processing_tool_receipt_reclaims_the_same_attempt() {
    use crate::application_public_api::callback_resume::{
        ApplicationPublishedCallbackResumeService, PreparedPublishedCallbackResume,
        PublishedCallbackResumeSource, PublishedCallbackResumeTarget,
        ResumePublishedCallbackCommand,
    };
    let (repository, actor, run) =
        fixture_with_input(json!({"sys":{"requested_model_id":"fixture"}})).await;
    let (callback, body) = seed_proven_full_round(&repository, run, true);
    let (_, results) = correlate_native_responses_callback(&repository, &actor, &body)
        .await
        .unwrap()
        .unwrap();
    let consumer = RecordingNativeCallbackConsumer {
        repository: repository.clone(),
        calls: Default::default(),
    };
    let service =
        ApplicationPublishedCallbackResumeService::new(repository.clone(), consumer.clone());
    let mut command = ResumePublishedCallbackCommand {
        responses_continuation: None,
        transport_connection_scope: None,
        observation_context: None,
        reserved_attempt_id: None,
        native_transport: Some(ProviderTransportPayload::openai_responses(body).unwrap()),
        bearer_token: String::new(),
        target: PublishedCallbackResumeTarget::CallbackTask {
            callback_task_id: callback.id,
        },
        source: PublishedCallbackResumeSource::OpenAiResponses,
        response_payload: results,
        response_mode: Some("streaming".into()),
    };
    let original_attempt = service
        .reserve_native_callback_for_actor(actor.clone(), &command)
        .await
        .unwrap();
    repository.complete_callback_task_for_test(callback.id);
    repository.expire_callback_resume_attempt_for_test(callback.id);
    repository.set_callback_resume_claim_lease_for_test(
        callback.id,
        OffsetDateTime::now_utc() - time::Duration::seconds(1),
    );

    let mut tampered = command.clone();
    tampered.response_payload["tool_results"][0]["content"] = json!("changed");
    assert!(matches!(
        service
            .prepare_callback_resume_for_actor(actor.clone(), &tampered)
            .await
            .unwrap_err()
            .downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::Conflict(
            "callback_resume_payload_conflict"
        ))
    ));
    assert!(matches!(
        service
            .prepare_callback_resume_for_actor(actor.clone(), &command)
            .await
            .unwrap(),
        PreparedPublishedCallbackResume::Resume { .. }
    ));
    command.reserved_attempt_id = Some(
        service
            .reserve_native_callback_for_actor(actor.clone(), &command)
            .await
            .unwrap(),
    );
    assert_eq!(command.reserved_attempt_id, Some(original_attempt));
    service
        .resume_callback_for_actor(actor, command)
        .await
        .unwrap();
    assert_eq!(consumer.calls.lock().unwrap().len(), 1);
    let attempts = repository.callback_resume_attempts();
    assert_eq!(attempts.len(), 1);
    assert_eq!(
        attempts[0].status,
        domain::FlowRunCallbackResumeAttemptStatus::Succeeded
    );
}

#[tokio::test]
async fn completed_tool_receipt_after_incomplete_inference_starts_new_full_context_turn() {
    use crate::application_public_api::callback_resume::{
        ApplicationPublishedCallbackResumeService, PreparedPublishedCallbackResume,
        PublishedCallbackResumeSource, PublishedCallbackResumeTarget,
        ResumePublishedCallbackCommand,
    };
    let (repository, actor, run) =
        fixture_with_input(json!({"sys":{"requested_model_id":"fixture"}})).await;
    let (callback, mut body) = seed_proven_full_round(&repository, run, true);
    body["input"].as_array_mut().unwrap().push(json!({
        "role":"user", "content":"Continue after output limit"
    }));
    let (admitted, results) = correlate_native_responses_callback(&repository, &actor, &body)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(admitted.id, callback.id);
    let service = ApplicationPublishedCallbackResumeService::new(
        repository.clone(),
        RecordingNativeCallbackConsumer {
            repository: repository.clone(),
            calls: Default::default(),
        },
    );
    let mut command = ResumePublishedCallbackCommand {
        responses_continuation: None,
        transport_connection_scope: None,
        observation_context: None,
        reserved_attempt_id: None,
        native_transport: Some(ProviderTransportPayload::openai_responses(body).unwrap()),
        bearer_token: String::new(),
        target: PublishedCallbackResumeTarget::CallbackTask {
            callback_task_id: callback.id,
        },
        source: PublishedCallbackResumeSource::OpenAiResponses,
        response_payload: results,
        response_mode: Some("streaming".into()),
    };
    command.reserved_attempt_id = Some(
        service
            .reserve_native_callback_for_actor(actor.clone(), &command)
            .await
            .unwrap(),
    );
    service
        .resume_callback_for_actor(actor.clone(), command.clone())
        .await
        .unwrap();
    repository.set_flow_run_status_for_test(run, FlowRunStatus::Incomplete);
    assert_eq!(
        service
            .prepare_callback_resume_for_actor(actor, &command)
            .await
            .unwrap(),
        PreparedPublishedCallbackResume::StartNewTurnFromHistory
    );
}

#[tokio::test]
async fn semantic_owned_round_verifies_full_delta_and_rejects_crossed_or_modified_inputs() {
    let first = json!({"model":"fixture", "tools":[], "input":[{"role":"user","content":"work"}]});
    let digest = ProviderTransportPayload::openai_responses(first.clone())
        .unwrap()
        .configuration_digest()
        .unwrap();
    let (repository, actor, run) =
        fixture_with_input(json!({"sys":{"responses_configuration_digest":digest}})).await;
    let calls = vec![json!({"id":"call_semantic","name":"read","arguments":{}})];
    let output =
        crate::application_public_api::compat::openai::projection::function_call_items(&calls);
    let history = crate::application_public_api::compat::openai::history::completed_history(
        &first, None, &output,
    )
    .unwrap()
    .unwrap();
    let callback = repository.seed_pending_llm_tool_callback_task(run, json!({"tool_calls":calls,"responses_round":{"response_id":format!("resp_{run}"),"history":history,"output":output}}));
    let suffix = json!([{"role":"user","content":"before"},{"type":"function_call_output","call_id":"call_semantic","output":"ok"},{"role":"user","content":"after"}]);
    let mut delta = first.clone();
    delta["input"] = suffix.clone();
    delta["previous_response_id"] = json!(format!("resp_{run}"));
    let grant = correlate_semantic_responses_callback(
        &repository,
        &actor,
        &OpenAiResponsesEnvelope::capture(delta.clone()).unwrap(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(grant.callback_task_id(), callback.id);
    assert_eq!(
        grant.payload()["responses_continuation"]["ordered_input"],
        suffix
    );
    assert_eq!(
        grant.payload()["responses_continuation"]["ordered_messages"][2]["content"],
        "after"
    );
    let mut full = first.clone();
    full["input"] = json!(first["input"]
        .as_array()
        .unwrap()
        .iter()
        .cloned()
        .chain(output)
        .chain(suffix.as_array().unwrap().iter().cloned())
        .collect::<Vec<_>>());
    let full_grant = correlate_semantic_responses_callback(
        &repository,
        &actor,
        &OpenAiResponsesEnvelope::capture(full.clone()).unwrap(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(grant.payload(), full_grant.payload());
    let mut changed = full.clone();
    changed["input"][0]["content"] = json!("edited history");
    let stored = repository
        .get_published_callback_task(callback.id)
        .await
        .unwrap()
        .unwrap();
    assert!(correlate_semantic_responses_callback(
        &repository,
        &actor,
        &OpenAiResponsesEnvelope::capture(changed).unwrap()
    )
    .await
    .unwrap()
    .is_none());
    assert_eq!(
        repository
            .get_published_callback_task(callback.id)
            .await
            .unwrap()
            .unwrap(),
        stored
    );
    assert!(repository.callback_resume_attempts().is_empty());
    let mut changed = delta.clone();
    changed["instructions"] = json!("new instructions");
    assert!(correlate_semantic_responses_callback(
        &repository,
        &actor,
        &OpenAiResponsesEnvelope::capture(changed).unwrap()
    )
    .await
    .is_err());
    let mut changed = delta.clone();
    changed["previous_response_id"] = json!("resp_wrong");
    assert!(correlate_semantic_responses_callback(
        &repository,
        &actor,
        &OpenAiResponsesEnvelope::capture(changed).unwrap()
    )
    .await
    .is_err());
    let mut changed = delta.clone();
    changed["input"]
        .as_array_mut()
        .unwrap()
        .push(suffix[1].clone());
    assert!(correlate_semantic_responses_callback(
        &repository,
        &actor,
        &OpenAiResponsesEnvelope::capture(changed).unwrap()
    )
    .await
    .is_err());
    for unsupported in [
        json!({"role":"user","content":"context","phase":"analysis"}),
        json!({"role":"assistant","content":"context","id":"msg_unowned"}),
        json!({"role":7,"content":"context"}),
        json!({"role":"user","content":[{"type":"input_text","text":"context","unknown_extension":true}]}),
        json!({"role":"user","content":[{"type":"input_image","image_url":{"url":"https://example.test/image.png","unknown_extension":true}}]}),
        json!({"role":"system","content":"new policy"}),
        json!({"role":"developer","content":"new policy"}),
        json!({"type":"reasoning","summary":[]}),
    ] {
        let mut changed = delta.clone();
        changed["input"]
            .as_array_mut()
            .unwrap()
            .insert(0, unsupported);
        let error = correlate_semantic_responses_callback(
            &repository,
            &actor,
            &OpenAiResponsesEnvelope::capture(changed).unwrap(),
        )
        .await
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("semantic_responses_continuation_item_unsupported"));
        let unchanged = repository
            .get_published_callback_task(callback.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(unchanged.status, CallbackTaskStatus::Pending);
        assert!(unchanged.response_payload.is_none());
    }
    for notice in [
        json!({"type":"function_call_output","name":"notification","namespace":"codex_app","output":[{"type":"input_text","text":"context"}],"extension":{"role":"system"}}),
        json!({"type":"function_call_output","call_id":null,"output":"context"}),
    ] {
        let mut mixed_delta = delta.clone();
        mixed_delta["input"]
            .as_array_mut()
            .unwrap()
            .insert(0, notice.clone());
        mixed_delta["input"]
            .as_array_mut()
            .unwrap()
            .push(notice.clone());
        let mut mixed_without_cursor = mixed_delta.clone();
        mixed_without_cursor
            .as_object_mut()
            .unwrap()
            .remove("previous_response_id");
        let mut mixed_full = full.clone();
        let prefix = mixed_full["input"].as_array().unwrap()[..2].to_vec();
        mixed_full["input"] = json!(prefix
            .into_iter()
            .chain(mixed_delta["input"].as_array().unwrap().iter().cloned())
            .collect::<Vec<_>>());
        for mixed in [mixed_delta, mixed_without_cursor, mixed_full] {
            let mixed_grant = correlate_semantic_responses_callback(
                &repository,
                &actor,
                &OpenAiResponsesEnvelope::capture(mixed).unwrap(),
            )
            .await
            .unwrap()
            .unwrap();
            let continuation = &mixed_grant.payload()["responses_continuation"];
            assert_eq!(continuation["ordered_input"][0], notice);
            assert_eq!(
                continuation["ordered_messages"][0]["external_context"]["output"],
                notice["output"]
            );
            assert!(continuation["ordered_messages"][0]
                .get("tool_call_id")
                .is_none());
            assert_eq!(
                continuation["ordered_messages"][2]["tool_call_id"],
                "call_semantic"
            );
            assert_eq!(
                continuation["ordered_messages"].as_array().unwrap().len(),
                5
            );
        }
    }
    let mut media = delta.clone();
    media["input"][0] = json!({"role":"user","content":[{"type":"input_text","text":"inspect"},{"type":"input_image","image_url":"https://example.test/image.png"}]});
    let media_grant = correlate_semantic_responses_callback(
        &repository,
        &actor,
        &OpenAiResponsesEnvelope::capture(media.clone()).unwrap(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        media_grant.payload()["responses_continuation"]["ordered_input"],
        media["input"]
    );
    assert_eq!(
        media_grant.payload()["responses_continuation"]["ordered_messages"][0]["content_blocks"][1]
            ["image_url"]["url"],
        "https://example.test/image.png"
    );
    let mut fork = full;
    fork["instructions"] = json!("New fork instructions");
    fork["input"][0]["content"] = json!("Reconstructed fork prefix");
    fork["input"].as_array_mut().unwrap().extend([
        json!({"type":"message","role":"assistant","phase":"final_answer","content":"Previous answer"}),
        json!({"role":"user","content":"Continue in a new fork"}),
    ]);
    assert!(
        correlate_semantic_responses_callback(
            &repository,
            &actor,
            &OpenAiResponsesEnvelope::capture(fork.clone()).unwrap(),
        )
        .await
        .unwrap()
        .is_none(),
        "rebuilt closed history starts new sampling without a pending receipt grant"
    );
    assert_eq!(
        repository
            .get_published_callback_task(callback.id)
            .await
            .unwrap()
            .unwrap(),
        stored
    );
    assert!(repository.callback_resume_attempts().is_empty());
    repository.complete_callback_task_for_test(callback.id);
    let mut cursor_retry = fork.clone();
    cursor_retry["previous_response_id"] = json!(format!("resp_{run}"));
    assert!(
        correlate_semantic_responses_callback(
            &repository,
            &actor,
            &OpenAiResponsesEnvelope::capture(cursor_retry).unwrap(),
        )
        .await
        .is_err(),
        "an assistant message must not bypass an explicit cursor"
    );
    assert!(correlate_semantic_responses_callback(
        &repository,
        &actor,
        &OpenAiResponsesEnvelope::capture(fork).unwrap(),
    )
    .await
    .unwrap()
    .is_none());
    assert!(repository.callback_resume_attempts().is_empty());
    let mut other = actor.clone();
    other.api_key_id = Uuid::now_v7();
    assert!(correlate_semantic_responses_callback(
        &repository,
        &other,
        &OpenAiResponsesEnvelope::capture(delta).unwrap()
    )
    .await
    .unwrap()
    .is_none());
}
