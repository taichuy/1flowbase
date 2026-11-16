use super::*;
use crate::application_public_api::{
    anthropic_continuation::verify_anthropic_continuation,
    client_protocol_envelope::{capture_source_protocol_request_body, ClientProtocolIngressPolicy},
};

async fn verified_command(
    repository: &ApplicationPublicApiTestRepository,
    actor: &crate::application_public_api::api_keys::ApplicationApiKeyActor,
    token: &str,
    run: &NativeRunResult,
    callback_id: Uuid,
    source: Value,
    result: &str,
) -> ResumePublishedCallbackCommand {
    let envelope = capture_source_protocol_request_body(
        ClientProtocolIngressPolicy::AnthropicMessages,
        &source,
    )
    .unwrap();
    let grant = verify_anthropic_continuation(
        repository,
        actor,
        callback_id,
        json!([{"tool_call_id":"a","content":result}]),
        source,
        envelope,
        None,
    )
    .await
    .unwrap();
    let mut command = resume_command(token, run.id, callback_id, grant.payload().clone());
    command.source = PublishedCallbackResumeSource::AnthropicMessages;
    command.anthropic_continuation = Some(grant);
    command
}

fn source_request() -> Value {
    json!({"model":"claude-fixture","max_tokens":100,"system":"system",
    "tools":[{"name":"Read","input_schema":{"type":"object"}}],
    "messages":[
        {"role":"user","content":"read source"},
        {"role":"assistant","content":[{"type":"tool_use","id":"a","name":"Read","input":{}}]},
        {"role":"user","content":[{"type":"tool_result","tool_use_id":"a","content":"file"}]}
    ]})
}

#[tokio::test]
async fn anthropic_verified_callback_retry_pre_token_429_starts_new_turn_without_reconsuming_receipt(
) {
    let (repository, mut consumer, token, run) = callback_fixture().await;
    consumer.terminal_error = Some(json!({
        "error_code":"provider_upstream_error", "status_code":429,
        "failed_after_first_token":false
    }));
    let callback = repository.seed_pending_llm_tool_callback_task(
        run.id,
        json!({"tool_calls":[{"id":"a","name":"Read","arguments":{}}]}),
    );
    let actor = ApplicationApiKeyService::new(repository.clone())
        .authenticate_bearer_token(&token)
        .await
        .unwrap();
    let service =
        ApplicationPublishedCallbackResumeService::new(repository.clone(), consumer.clone());
    let original = verified_command(
        &repository,
        &actor,
        &token,
        &run,
        callback.id,
        source_request(),
        "file",
    )
    .await;
    let first = service.resume_callback(original).await.unwrap();
    assert_eq!(
        first.run.status,
        crate::application_public_api::native::NativeRunStatus::Failed
    );
    assert_eq!(
        first.attempt.status,
        domain::FlowRunCallbackResumeAttemptStatus::Succeeded
    );
    let attempts = repository.callback_resume_attempts();
    let stored = repository
        .get_published_callback_task(callback.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.status, domain::CallbackTaskStatus::Completed);

    // Real ingress verifies each transport request, including an identical retry.
    let retry = verified_command(
        &repository,
        &actor,
        &token,
        &run,
        callback.id,
        source_request(),
        "file",
    )
    .await;
    assert!(retry.anthropic_continuation.is_some());
    assert_eq!(
        service.prepare_callback_resume(&retry).await.unwrap(),
        PreparedPublishedCallbackResume::StartNewTurnFromHistory
    );
    assert_eq!(consumer.call_count(), 1);
    assert_eq!(attempts.len(), 1);
    assert_eq!(repository.callback_resume_attempts(), attempts);
    assert_eq!(
        repository
            .get_published_callback_task(callback.id)
            .await
            .unwrap()
            .unwrap(),
        stored
    );
}

#[tokio::test]
async fn anthropic_verified_callback_retry_keeps_unsafe_failures_and_changed_context_fenced() {
    for variant in [
        "partial_output",
        "unknown_token_boundary",
        "auth_failed",
        "model_not_found",
        "provider_affinity_mismatch",
        "provider_transport_unavailable",
        "tool_output",
        "prior_context",
        "options",
        "source",
    ] {
        let (repository, mut consumer, token, run) = callback_fixture().await;
        let mut error = json!({
            "error_code":"provider_upstream_error", "status_code":429,
            "failed_after_first_token":false
        });
        match variant {
            "partial_output" => error["failed_after_first_token"] = json!(true),
            "unknown_token_boundary" => {
                error
                    .as_object_mut()
                    .unwrap()
                    .remove("failed_after_first_token");
            }
            "auth_failed"
            | "model_not_found"
            | "provider_affinity_mismatch"
            | "provider_transport_unavailable" => error["error_code"] = json!(variant),
            _ => {}
        }
        consumer.terminal_error = Some(error);
        let callback = repository.seed_pending_llm_tool_callback_task(
            run.id,
            json!({"tool_calls":[{"id":"a","name":"Read","arguments":{}}]}),
        );
        let actor = ApplicationApiKeyService::new(repository.clone())
            .authenticate_bearer_token(&token)
            .await
            .unwrap();
        let service =
            ApplicationPublishedCallbackResumeService::new(repository.clone(), consumer.clone());
        let original = verified_command(
            &repository,
            &actor,
            &token,
            &run,
            callback.id,
            source_request(),
            "file",
        )
        .await;
        service.resume_callback(original).await.unwrap();
        let attempts = repository.callback_resume_attempts();
        let stored = repository
            .get_published_callback_task(callback.id)
            .await
            .unwrap()
            .unwrap();
        let mut source = source_request();
        let result = if variant == "tool_output" {
            "tampered"
        } else {
            "file"
        };
        match variant {
            "tool_output" => source["messages"][2]["content"][0]["content"] = json!(result),
            "prior_context" => source["messages"][0]["content"] = json!("rewritten predecessor"),
            "options" => source["max_tokens"] = json!(200),
            _ => {}
        }
        let mut retry = verified_command(
            &repository,
            &actor,
            &token,
            &run,
            callback.id,
            source,
            result,
        )
        .await;
        if variant == "source" {
            retry.source = PublishedCallbackResumeSource::OpenAiChat;
        }
        let admission = service.prepare_callback_resume(&retry).await;
        if matches!(
            variant,
            "tool_output" | "prior_context" | "options" | "source"
        ) {
            let expected_conflict = if variant == "source" {
                "anthropic_continuation_transport_mismatch"
            } else {
                "callback_resume_payload_conflict"
            };
            assert!(
                admission
                    .unwrap_err()
                    .to_string()
                    .contains(expected_conflict),
                "{variant}: changed retry must retain its admission conflict"
            );
        } else {
            assert!(
                matches!(
                    admission.unwrap(),
                    PreparedPublishedCallbackResume::Resume { .. }
                ),
                "{variant}"
            );
        }
        assert_eq!(consumer.call_count(), 1, "{variant}");
        assert_eq!(repository.callback_resume_attempts(), attempts, "{variant}");
        assert_eq!(
            repository
                .get_published_callback_task(callback.id)
                .await
                .unwrap()
                .unwrap(),
            stored,
            "{variant}"
        );
    }
}

#[tokio::test]
async fn anthropic_followup_after_partial_inference_failure_starts_new_turn_without_replaying_tools(
) {
    for variant in [
        "separate_user",
        "coalesced_user",
        "partial_assistant",
        "extended_user",
        "image_user",
        "extended_media",
    ] {
        let (repository, mut consumer, token, run) = callback_fixture().await;
        consumer.terminal_error = Some(json!({
            "error_code":"provider_timeout", "failed_after_first_token":true,
            "message":"stream failed after partial output"
        }));
        let callback = repository.seed_pending_llm_tool_callback_task(
            run.id,
            json!({"tool_calls":[{"id":"a","name":"Read","arguments":{}}]}),
        );
        let actor = ApplicationApiKeyService::new(repository.clone())
            .authenticate_bearer_token(&token)
            .await
            .unwrap();
        let mut source = source_request();
        if variant == "extended_user" || variant == "extended_media" {
            source["messages"][2]["content"]
                .as_array_mut()
                .unwrap()
                .push(json!({"type":"text","text":"original reminder"}));
        }
        let service =
            ApplicationPublishedCallbackResumeService::new(repository.clone(), consumer.clone());
        let original = verified_command(
            &repository,
            &actor,
            &token,
            &run,
            callback.id,
            source.clone(),
            "file",
        )
        .await;
        service.resume_callback(original).await.unwrap();
        let attempts = repository.callback_resume_attempts();
        let stored = repository
            .get_published_callback_task(callback.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, domain::CallbackTaskStatus::Completed);
        let image =
            json!({"type":"image","source":{"type":"url","url":"https://example.test/probe.png"}});
        if variant == "extended_media" {
            source["messages"][2]["content"]
                .as_array_mut()
                .unwrap()
                .push(image);
        } else if variant == "image_user" {
            source["messages"]
                .as_array_mut()
                .unwrap()
                .push(json!({"role":"user","content":[image]}));
        } else if variant == "coalesced_user" || variant == "extended_user" {
            source["messages"][2]["content"]
                .as_array_mut()
                .unwrap()
                .push(json!({"type":"text","text":"Only provide the prompt now."}));
        } else {
            if variant == "partial_assistant" {
                source["messages"].as_array_mut().unwrap().push(json!({
                    "role":"assistant","content":[{"type":"text","text":"Partial output before interruption."}]
                }));
            }
            source["messages"]
                .as_array_mut()
                .unwrap()
                .push(json!({"role":"user","content":"Only provide the prompt now."}));
        }
        let next = verified_command(
            &repository,
            &actor,
            &token,
            &run,
            callback.id,
            source,
            "file",
        )
        .await;
        assert_eq!(
            service
                .prepare_callback_resume(&next)
                .await
                .unwrap_or_else(|error| panic!(
                    "{variant}: {error}; admitted={}; incoming={}",
                    attempts[0].response_payload["anthropic_continuation"]["ordered_messages"],
                    next.response_payload["anthropic_continuation"]["ordered_messages"]
                )),
            PreparedPublishedCallbackResume::StartNewTurnFromHistory,
            "{variant}"
        );
        assert_eq!(
            consumer.call_count(),
            1,
            "old tool receipt must not execute again"
        );
        assert_eq!(repository.callback_resume_attempts(), attempts);
        assert_eq!(
            repository
                .get_published_callback_task(callback.id)
                .await
                .unwrap()
                .unwrap(),
            stored
        );
    }
}

#[tokio::test]
async fn anthropic_followup_cannot_replace_accepted_tool_output_or_rewrite_prior_context() {
    for variant in ["tool_output", "prior_context", "options_only"] {
        let (repository, mut consumer, token, run) = callback_fixture().await;
        consumer.terminal_error =
            Some(json!({"error_code":"provider_timeout","failed_after_first_token":true}));
        let callback = repository.seed_pending_llm_tool_callback_task(
            run.id,
            json!({"tool_calls":[{"id":"a","name":"Read","arguments":{}}]}),
        );
        let actor = ApplicationApiKeyService::new(repository.clone())
            .authenticate_bearer_token(&token)
            .await
            .unwrap();
        let service =
            ApplicationPublishedCallbackResumeService::new(repository.clone(), consumer.clone());
        let mut source = source_request();
        let command = verified_command(
            &repository,
            &actor,
            &token,
            &run,
            callback.id,
            source.clone(),
            "file",
        )
        .await;
        service.resume_callback(command.clone()).await.unwrap();
        assert!(matches!(
            service.prepare_callback_resume(&command).await.unwrap(),
            PreparedPublishedCallbackResume::Resume { .. }
        ));
        let attempts = repository.callback_resume_attempts();
        if variant == "prior_context" {
            source["messages"][0]["content"] = json!("rewritten predecessor");
        }
        if variant == "options_only" {
            source["max_tokens"] = json!(200);
        } else {
            source["messages"]
                .as_array_mut()
                .unwrap()
                .push(json!({"role":"user","content":"new input"}));
        }
        let result = if variant == "tool_output" {
            "tampered"
        } else {
            "file"
        };
        if variant == "tool_output" {
            source["messages"][2]["content"][0]["content"] = json!(result);
        }
        let next = verified_command(
            &repository,
            &actor,
            &token,
            &run,
            callback.id,
            source,
            result,
        )
        .await;
        assert!(
            service
                .prepare_callback_resume(&next)
                .await
                .unwrap_err()
                .to_string()
                .contains("callback_resume_payload_conflict"),
            "{variant}"
        );
        assert_eq!(consumer.call_count(), 1);
        assert_eq!(repository.callback_resume_attempts(), attempts);
    }
}

#[tokio::test]
async fn anthropic_followup_after_success_starts_new_turn_and_active_predecessor_stays_fenced() {
    for terminal in [true, false] {
        let (repository, consumer, token, run) = callback_fixture().await;
        let callback = repository.seed_pending_llm_tool_callback_task(
            run.id,
            json!({"tool_calls":[{"id":"a","name":"Read","arguments":{}}]}),
        );
        let actor = ApplicationApiKeyService::new(repository.clone())
            .authenticate_bearer_token(&token)
            .await
            .unwrap();
        let mut source = source_request();
        let original = verified_command(
            &repository,
            &actor,
            &token,
            &run,
            callback.id,
            source.clone(),
            "file",
        )
        .await;
        if terminal {
            ApplicationPublishedCallbackResumeService::new(
                repository.clone(),
                RunCompletingCallbackConsumer {
                    repository: repository.clone(),
                },
            )
            .resume_callback(original)
            .await
            .unwrap();
        } else {
            ApplicationPublishedCallbackResumeService::new(repository.clone(), consumer.clone())
                .resume_callback(original)
                .await
                .unwrap();
        }
        let attempts = repository.callback_resume_attempts();
        source["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"user","content":"new input"}));
        let next = verified_command(
            &repository,
            &actor,
            &token,
            &run,
            callback.id,
            source,
            "file",
        )
        .await;
        let service = ApplicationPublishedCallbackResumeService::new(repository.clone(), consumer);
        let result = service.prepare_callback_resume(&next).await;
        if terminal {
            assert_eq!(
                result.unwrap(),
                PreparedPublishedCallbackResume::StartNewTurnFromHistory
            );
        } else {
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("callback_resume_payload_conflict"));
        }
        assert_eq!(repository.callback_resume_attempts(), attempts);
    }
}
