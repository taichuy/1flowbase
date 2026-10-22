use super::*;
use crate::application_public_api::callback_resume::{
    ApplicationPublishedCallbackResumeService, PublishedCallbackResumeSource,
    PublishedCallbackResumeTarget, ResumePublishedCallbackCommand,
};
use crate::application_public_api::compat::openai::history::{
    prove_full_context_input, validate_full_retry_input,
};
use crate::ports::ToolCallbackResultInput;

fn append_notifications(body: &mut Value) {
    body["input"].as_array_mut().unwrap().extend([
        json!({"type":"custom_tool_call_output","call_id":"custom","output":"notify one"}),
        json!({"type":"custom_tool_call_output","call_id":"custom","output":"notify two"}),
    ]);
}

#[tokio::test]
async fn multipart_custom_outputs_preserve_wire_and_one_complete_receipt_per_call() {
    for mode in ["full", "delta", "outputs_only"] {
        let (repository, actor, run) = fixture().await;
        let (callback, mut body) = seed_proven_full_round(&repository, run, true);
        if mode != "full" {
            body["input"] = request()["input"].clone();
        }
        if mode == "delta" {
            body["previous_response_id"] = json!("resp_round");
        }
        append_notifications(&mut body);
        let original = body.clone();
        let (admitted, payload) = correlate_native_responses_callback(&repository, &actor, &body)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(admitted.id, callback.id);
        let results = payload["tool_results"].as_array().unwrap();
        assert_eq!(results.len(), 2, "inbox keeps one receipt per call");
        assert_eq!(
            results[0],
            json!({"tool_call_id":"function","content":"first result"})
        );
        assert_eq!(
            results[1]["content"],
            json!([
                {"type":"input_text","text":"second result"},
                {"type":"input_image","image_url":"data:image/png;base64,AA=="},
                {"type":"input_text","text":"notify one"},
                {"type":"input_text","text":"notify two"}
            ])
        );
        let parts = results[1]["responses_output_items"].as_array().unwrap();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0]["output_index"], 1);
        assert_eq!(parts[2]["item"]["output"], "notify two");
        if mode == "full" {
            let history =
                &callback.request_payload["provider_metadata"]["native_response"]["history"];
            let ids = vec!["function".into(), "custom".into()];
            let proof = prove_full_context_input(&body["input"], history, &ids).unwrap();
            assert_eq!(proof.tool_outputs.len(), 4);
            assert_eq!(proof.ordered, &body["input"].as_array().unwrap()[3..]);
            validate_full_retry_input(&body["input"], history, &ids).unwrap();
        }
        let mut translated =
            crate::application_public_api::compat::openai::translate_response_request(body.clone())
                .unwrap();
        assert_eq!(
            translated
                .request
                .metadata
                .take_provider_transport_payload()
                .unwrap()
                .wire_body(),
            &original
        );
        let receipt = ToolCallbackResultInput::from_payload(results[1].clone()).unwrap();
        let mut reordered = body.clone();
        let items = reordered["input"].as_array_mut().unwrap();
        let len = items.len();
        items.swap(len - 1, len - 2);
        let (_, changed) = correlate_native_responses_callback(&repository, &actor, &reordered)
            .await
            .unwrap()
            .unwrap();
        assert_ne!(
            receipt.result_fingerprint,
            ToolCallbackResultInput::from_payload(changed["tool_results"][1].clone())
                .unwrap()
                .result_fingerprint
        );
        assert_eq!(body, original);
        assert!(repository.callback_resume_attempts().is_empty());
    }
}

#[tokio::test]
async fn multipart_outputs_do_not_relax_function_duplicates_or_round_ownership() {
    let (repository, actor, run) = fixture().await;
    let (_, mut body) = seed_proven_full_round(&repository, run, true);
    append_notifications(&mut body);
    for (kind, id, error) in [
        (
            "function_call_output",
            "custom",
            "native_tool_output_duplicate",
        ),
        (
            "custom_tool_call_output",
            "function",
            "native_tool_output_duplicate",
        ),
        (
            "custom_tool_call_output",
            "foreign",
            "native_tool_output_incomplete_round",
        ),
    ] {
        let mut invalid = body.clone();
        *invalid["input"].as_array_mut().unwrap().last_mut().unwrap() =
            json!({"type":kind,"call_id":id,"output":"extra"});
        assert!(
            correlate_native_responses_callback(&repository, &actor, &invalid)
                .await
                .unwrap_err()
                .to_string()
                .contains(error)
        );
    }
    let mut invalid = body.clone();
    invalid["input"].as_array_mut().unwrap().remove(3);
    assert!(
        correlate_native_responses_callback(&repository, &actor, &invalid)
            .await
            .unwrap_err()
            .to_string()
            .contains("incomplete_round")
    );
    invalid = body;
    invalid["input"][0]["content"] = json!("tampered predecessor");
    assert!(
        correlate_native_responses_callback(&repository, &actor, &invalid)
            .await
            .unwrap_err()
            .to_string()
            .contains("history_mismatch")
    );
}

#[tokio::test]
async fn multipart_callback_replay_consumes_once_and_rejects_changed_notification() {
    let (repository, actor, run) = fixture().await;
    let (callback, mut body) = seed_proven_full_round(&repository, run, true);
    append_notifications(&mut body);
    let (_, payload) = correlate_native_responses_callback(&repository, &actor, &body)
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
        response_payload: payload.clone(),
        response_mode: Some("streaming".into()),
    };
    for _ in 0..2 {
        service
            .resume_callback_for_actor(actor.clone(), command.clone())
            .await
            .unwrap();
    }
    assert_eq!(consumer.calls.lock().unwrap().len(), 1);
    assert_eq!(
        consumer.calls.lock().unwrap()[0]
            .native_transport
            .as_ref()
            .unwrap()
            .wire_body(),
        &body
    );
    assert_eq!(repository.callback_resume_attempts().len(), 1);
    body["input"].as_array_mut().unwrap().last_mut().unwrap()["output"] =
        json!("changed notification");
    let (_, changed) = correlate_native_responses_callback(&repository, &actor, &body)
        .await
        .unwrap()
        .unwrap();
    let mut conflict = command;
    conflict.native_transport = Some(ProviderTransportPayload::openai_responses(body).unwrap());
    conflict.response_payload = changed;
    assert!(service
        .resume_callback_for_actor(actor, conflict)
        .await
        .unwrap_err()
        .to_string()
        .contains("payload_conflict"));
    assert_eq!(consumer.calls.lock().unwrap().len(), 1);
}
