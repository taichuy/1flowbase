use super::*;

async fn parent_round(
    semantic: bool,
    custom: bool,
) -> (
    ApplicationPublicApiTestRepository,
    ApplicationApiKeyActor,
    CallbackTaskRecord,
    Value,
) {
    let first = json!({"model":"fixture","tools":[],"input":[
        {"type":"additional_tools","id":"at_parent","role":"developer","tools":[]},
        {"type":"message","id":"msg_parent","role":"developer","content":"Shared instructions"},
        {"role":"user","content":"Read files"}
    ]});
    let digest = ProviderTransportPayload::openai_responses(first.clone())
        .unwrap()
        .configuration_digest()
        .unwrap();
    let (repository, actor, run) = fixture_with_input(json!({"sys":{
        "provider_session_identity":"parent-session",
        "responses_configuration_digest":digest,
        "requested_model_id":"fixture"
    }}))
    .await;
    let call_id = if custom { "custom" } else { "function" };
    let calls = json!([{"id":call_id,"name":"read","arguments":{}}]);
    let output = if custom {
        json!([{"type":"custom_tool_call","call_id":call_id,"name":"read","input":"read"}])
    } else {
        json!([{"type":"function_call","call_id":call_id,"name":"read","arguments":"{}"}])
    };
    let history = crate::application_public_api::compat::openai::history::completed_history(
        &first,
        None,
        output.as_array().unwrap(),
    )
    .unwrap()
    .unwrap();
    let evidence = if semantic {
        json!({"tool_calls":calls,"responses_round":{
            "response_id":"resp_parent","history":history,"output":output
        }})
    } else {
        json!({"tool_calls":calls,"provider_metadata":{"native_response":{
            "response_id":"resp_parent","configuration_digest":digest,"history":history
        }}})
    };
    let callback = repository.seed_pending_llm_tool_callback_task(run, evidence);
    let mut full = first;
    full["input"]
        .as_array_mut()
        .unwrap()
        .extend(output.as_array().unwrap().iter().cloned());
    full["input"].as_array_mut().unwrap().push(json!({
        "type":if custom { "custom_tool_call_output" } else { "function_call_output" },
        "call_id":call_id,"output":"File contents"
    }));
    (repository, actor, callback, full)
}

fn fork_request(mut full: Value) -> OpenAiResponsesEnvelope {
    // Codex core/client.rs builds these UUIDv5 IDs in a thread-specific namespace.
    // Only their IDs change; no assistant response has followed the tool result yet.
    full["input"][0]["id"] = json!("at_fork");
    full["input"][1]["id"] = json!("msg_fork");
    full["input"].as_array_mut().unwrap().push(json!({
        "role":"user","content":"What command was executed?"
    }));
    OpenAiResponsesEnvelope::capture(full)
        .unwrap()
        .with_session_identity("fork-session".into())
}

#[tokio::test]
async fn native_new_session_history_does_not_resume_parent_before_assistant_response() {
    for (completed, custom) in [(false, false), (true, false), (false, true), (true, true)] {
        let (repository, actor, callback, mut full) = parent_round(false, custom).await;
        if custom {
            full["input"].as_array_mut().unwrap().push(json!({
                "type":"custom_tool_call_output","call_id":"custom","output":"Additional notification"
            }));
        }
        if completed {
            repository.complete_callback_task_for_test(callback.id);
        }
        let envelope = fork_request(full);
        assert!(
            correlate_native_responses_callback_from_envelope(&repository, &actor, &envelope)
                .await
                .expect("an independent full history must not prove the parent's thread-local IDs")
                .is_none()
        );
        let mut translated =
            crate::application_public_api::compat::openai::translate_response_request(
                envelope.raw_body().clone(),
            )
            .unwrap();
        assert_eq!(
            translated
                .request
                .metadata
                .take_provider_transport_payload()
                .unwrap()
                .wire_body(),
            envelope.raw_body(),
            "new sampling must retain the exact wire history"
        );
        assert!(repository.callback_resume_attempts().is_empty());
        assert_eq!(
            repository
                .get_published_callback_task(callback.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            if completed {
                CallbackTaskStatus::Completed
            } else {
                CallbackTaskStatus::Pending
            }
        );
    }
}

#[tokio::test]
async fn semantic_new_session_history_does_not_mint_a_parent_continuation_grant() {
    for completed in [false, true] {
        let (repository, actor, callback, full) = parent_round(true, false).await;
        if completed {
            repository.complete_callback_task_for_test(callback.id);
        }
        assert!(
            correlate_semantic_responses_callback(&repository, &actor, &fork_request(full))
                .await
                .expect("an independent history cannot claim a semantic continuation")
                .is_none()
        );
        assert!(repository.callback_resume_attempts().is_empty());
    }
}

#[tokio::test]
async fn native_new_session_history_keeps_existing_callback_proof_boundaries() {
    let (repository, actor, callback, full) = parent_round(false, false).await;
    repository.complete_callback_task_for_test(callback.id);
    let fork = fork_request(full.clone());
    for identity in [None, Some("parent-session")] {
        let mut body = fork.raw_body().clone();
        body["client_metadata"] = json!({"provider_session_identity":"fork-session"});
        let mut envelope = OpenAiResponsesEnvelope::capture(body).unwrap();
        if let Some(identity) = identity {
            envelope = envelope.with_session_identity(identity.into());
        }
        let error =
            correlate_native_responses_callback_from_envelope(&repository, &actor, &envelope)
                .await
                .unwrap_err();
        assert_eq!(
            error.downcast_ref::<ControlPlaneError>(),
            Some(&ControlPlaneError::Conflict(
                "native_tool_output_history_mismatch"
            ))
        );
    }
    let mut delta = full.clone();
    delta["input"] = json!([full["input"].as_array().unwrap().last().unwrap()]);
    let mut cursor = fork.raw_body().clone();
    cursor["previous_response_id"] = json!("resp_parent");
    for body in [delta, cursor] {
        let envelope = OpenAiResponsesEnvelope::capture(body)
            .unwrap()
            .with_session_identity("fork-session".into());
        let correlated =
            correlate_native_responses_callback_from_envelope(&repository, &actor, &envelope)
                .await
                .unwrap()
                .unwrap();
        assert_eq!(
            correlated.0.id, callback.id,
            "output-only and explicit-cursor requests retain their existing callback owner"
        );
    }
    let mut unpaired = fork.raw_body().clone();
    unpaired["input"].as_array_mut().unwrap().remove(3);
    let error = correlate_native_responses_callback_from_envelope(
        &repository,
        &actor,
        &OpenAiResponsesEnvelope::capture(unpaired)
            .unwrap()
            .with_session_identity("fork-session".into()),
    )
    .await
    .unwrap_err();
    assert_eq!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(&ControlPlaneError::Conflict(
            "native_tool_output_history_mismatch"
        ))
    );
    assert!(repository.callback_resume_attempts().is_empty());
}
