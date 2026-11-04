use super::*;
use crate::application_public_api::compat::openai::{
    history::prove_full_context_input, translate_response_request,
};

fn notification() -> Value {
    json!({
        "type":"function_call_output",
        "id":"fco_notification",
        "name":"send_message_to_thread",
        "namespace":"codex_app",
        "output":[{"type":"input_text","text":"Continue with the new context"}],
        "extension":{"source":"another-thread"}
    })
}

#[tokio::test]
async fn standalone_notifications_preserve_native_wire_without_claiming_a_callback() {
    let (repository, actor, run) = fixture().await;
    let callback = seed_round(&repository, run, &request());
    for mut notice in [
        notification(),
        json!({"type":"function_call_output","output":"context"}),
    ] {
        for null_call_id in [false, true] {
            if null_call_id {
                notice["call_id"] = Value::Null;
            }
            let body = json!({"model":"fixture","input":[notice]});
            let envelope = OpenAiResponsesEnvelope::capture(body.clone()).unwrap();
            assert!(
                correlate_semantic_responses_callback(&repository, &actor, &envelope)
                    .await
                    .unwrap()
                    .is_none()
            );
            assert!(
                correlate_native_responses_callback(&repository, &actor, &body)
                    .await
                    .unwrap()
                    .is_none()
            );
            let mut translated = translate_response_request(body.clone()).unwrap();
            assert!(translated.request.query.is_empty());
            assert_eq!(translated.request.history.len(), 1);
            assert_eq!(
                translated.request.history[0]["external_context"]["output"],
                notice["output"]
            );
            assert!(translated.request.history[0].get("tool_call_id").is_none());
            assert_eq!(
                translated
                    .request
                    .metadata
                    .take_provider_transport_payload()
                    .expect("notifications retain preferred native transport")
                    .wire_body(),
                &body
            );
        }
    }
    let unchanged = repository
        .get_published_callback_task(callback.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unchanged.status, CallbackTaskStatus::Pending);
    assert!(unchanged.response_payload.is_none());
    assert!(repository.callback_resume_attempts().is_empty());
}

#[tokio::test]
async fn standalone_notifications_mix_with_native_receipts_in_full_and_delta_requests() {
    let (repository, actor, run) = fixture().await;
    let (callback, mut full) = seed_proven_full_round(&repository, run, true);
    let mut null_notice = notification();
    null_notice["call_id"] = Value::Null;
    let receipts = request()["input"].as_array().unwrap().clone();
    let suffix = vec![
        notification(),
        receipts[0].clone(),
        null_notice,
        receipts[1].clone(),
        notification(),
    ];
    full["input"].as_array_mut().unwrap().truncate(3);
    full["input"].as_array_mut().unwrap().extend(suffix.clone());
    let proof = prove_full_context_input(
        &full["input"],
        &callback.request_payload["provider_metadata"]["native_response"]["history"],
        &["function".into(), "custom".into()],
    )
    .unwrap();
    assert_eq!(proof.ordered, suffix);
    assert_eq!(proof.tool_outputs, receipts.iter().collect::<Vec<_>>());
    assert_eq!(proof.context.len(), 3);
    let mut delta = full.clone();
    delta["previous_response_id"] = json!("resp_round");
    delta["input"] = json!(suffix);
    for body in [full, delta] {
        let envelope = OpenAiResponsesEnvelope::capture(body.clone()).unwrap();
        assert!(
            correlate_semantic_responses_callback(&repository, &actor, &envelope)
                .await
                .unwrap()
                .is_none()
        );
        let (admitted, result) = correlate_native_responses_callback(&repository, &actor, &body)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(admitted.id, callback.id);
        assert_eq!(
            result,
            json!({"tool_results":[
                {"tool_call_id":"function","content":receipts[0]["output"]},
                {"tool_call_id":"custom","content":receipts[1]["output"]}
            ]})
        );
        let mut translated = translate_response_request(body.clone()).unwrap();
        assert_eq!(
            translated
                .request
                .metadata
                .take_provider_transport_payload()
                .unwrap()
                .wire_body(),
            &body,
            "notification fields and order must reach the native provider"
        );
    }
}

#[tokio::test]
async fn standalone_notification_after_an_answer_does_not_reopen_a_completed_receipt() {
    let (repository, actor, run) = fixture().await;
    let (callback, mut body) = seed_proven_full_round(&repository, run, true);
    repository.complete_callback_task_for_test(callback.id);
    body["input"].as_array_mut().unwrap().extend([
        json!({"type":"message","role":"assistant","content":"Finished"}),
        notification(),
    ]);
    assert!(
        correlate_native_responses_callback(&repository, &actor, &body)
            .await
            .unwrap()
            .is_none()
    );
    assert!(correlate_semantic_responses_callback(
        &repository,
        &actor,
        &OpenAiResponsesEnvelope::capture(body).unwrap()
    )
    .await
    .unwrap()
    .is_none());
    assert!(repository.callback_resume_attempts().is_empty());
}

#[tokio::test]
async fn standalone_notifications_do_not_weaken_native_receipt_or_history_validation() {
    let (repository, actor, run) = fixture().await;
    let (callback, mut full) = seed_proven_full_round(&repository, run, true);
    full["input"].as_array_mut().unwrap().push(notification());
    let mut incomplete = full.clone();
    incomplete["input"].as_array_mut().unwrap().remove(3);
    let mut duplicate = full.clone();
    duplicate["input"]
        .as_array_mut()
        .unwrap()
        .push(full["input"][3].clone());
    let mut unknown = full.clone();
    unknown["input"][3]["call_id"] = json!("foreign");
    let mut tampered = full.clone();
    tampered["input"][0]["content"] = json!("changed predecessor");
    for (body, code) in [
        (incomplete, "native_tool_output_incomplete_round"),
        (duplicate, "native_tool_output_duplicate"),
        (unknown, "native_tool_output_incomplete_round"),
    ] {
        assert!(
            correlate_native_responses_callback(&repository, &actor, &body)
                .await
                .unwrap_err()
                .to_string()
                .contains(code),
            "expected {code}"
        );
    }
    assert!(
        correlate_native_responses_callback(&repository, &actor, &tampered)
            .await
            .unwrap()
            .is_none()
    );
    let mut other = actor.clone();
    other.api_key_id = Uuid::now_v7();
    assert!(
        correlate_native_responses_callback(&repository, &other, &full)
            .await
            .expect(
                "closed history under another key starts sampling without claiming the callback"
            )
            .is_none()
    );
    let mut with_parent = full.clone();
    with_parent["previous_response_id"] = json!("resp_round");
    let mut output_only = request();
    output_only["input"]
        .as_array_mut()
        .unwrap()
        .push(notification());
    for body in [with_parent, output_only] {
        assert_eq!(
            correlate_native_responses_callback(&repository, &other, &body)
                .await
                .unwrap_err()
                .downcast_ref::<ControlPlaneError>(),
            Some(&ControlPlaneError::Conflict("native_tool_output_unknown"))
        );
    }
    let unchanged = repository
        .get_published_callback_task(callback.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unchanged.status, CallbackTaskStatus::Pending);
    assert_eq!(unchanged.response_payload, callback.response_payload);
    assert!(repository.callback_resume_attempts().is_empty());
    assert_eq!(repository.flow_run_count(), 1);
}

#[tokio::test]
async fn malformed_receipt_ids_are_not_standalone_notifications() {
    let (repository, actor, _) = fixture().await;
    for kind in ["function_call_output", "custom_tool_call_output"] {
        let ids = if kind == "custom_tool_call_output" {
            vec![Value::Null, json!(""), json!(7), json!(false)]
        } else {
            vec![json!(""), json!(7), json!(false)]
        };
        for id in ids {
            let body =
                json!({"model":"fixture","input":[{"type":kind,"call_id":id,"output":"result"}]});
            assert!(
                correlate_native_responses_callback(&repository, &actor, &body)
                    .await
                    .unwrap_err()
                    .to_string()
                    .contains("native_tool_output_call_id")
            );
        }
    }
    let missing_custom_id =
        json!({"model":"fixture","input":[{"type":"custom_tool_call_output","output":"result"}]});
    assert!(
        correlate_native_responses_callback(&repository, &actor, &missing_custom_id)
            .await
            .unwrap_err()
            .to_string()
            .contains("native_tool_output_call_id")
    );
}

#[test]
fn notifications_keep_entire_ordered_input_and_reject_malformed_ids() {
    let body = json!({"model":"fixture","input":[
        {"role":"user","content":"before"}, notification(),
        {"role":"user","content":"last user"}, notification(),
        {"role":"assistant","content":"trailing assistant"}, notification()
    ]});
    let mut translated = translate_response_request(body.clone()).unwrap();
    assert!(translated.request.query.is_empty());
    assert_eq!(translated.request.history.len(), 6);
    assert_eq!(translated.request.history[2]["content"], "last user");
    assert_eq!(
        translated.request.history[4]["content"],
        "trailing assistant"
    );
    for index in [1, 3, 5] {
        assert_eq!(
            translated.request.history[index]["external_context"]["output"],
            notification()["output"]
        );
    }
    let mapping = crate::application_public_api::mapping::ApplicationApiMappingConfig {
        input: crate::application_public_api::mapping::ApplicationApiMappingInput {
            query_target: "start.query".into(),
            model_target: Some("start.model".into()),
            inputs_target: Some("start".into()),
            history_target: Some("start.history".into()),
            attachments_target: None,
        },
        output: Default::default(),
        extension: None,
    };
    let mapped = crate::application_public_api::native::NativeInputMapper::map(
        &translated.request,
        &mapping,
    )
    .unwrap();
    assert_eq!(
        mapped.node_input_payload["start"]["history"],
        json!(translated.request.history)
    );
    assert_eq!(
        mapped.node_input_payload["__native_model_prompt_context"]["messages"],
        json!(translated.request.history)
    );
    let transport = translated
        .request
        .metadata
        .take_provider_transport_payload()
        .unwrap();
    assert!(transport.is_preferred());
    assert_eq!(transport.wire_body(), &body);
    for id in [json!(""), json!(" "), json!(7), json!(false)] {
        let malformed = json!({"model":"fixture","input":[{"role":"user","content":"work"},{"type":"function_call_output","call_id":id,"output":"notice"}]});
        assert!(translate_response_request(malformed).is_err());
    }
}
