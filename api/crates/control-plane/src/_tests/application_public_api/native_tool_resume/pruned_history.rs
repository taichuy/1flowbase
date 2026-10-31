use super::*;
use crate::application_public_api::compat::openai::{
    history::{completed_history, prove_full_context_input},
    translate_response_request,
};

async fn historical_round(
    semantic: bool,
) -> (
    ApplicationPublicApiTestRepository,
    ApplicationApiKeyActor,
    CallbackTaskRecord,
    Value,
) {
    let mut items = vec![json!({"role":"user","content":"Inspect the repository"})];
    // Seventeen closed historical rounds, then the owned eighteenth round. Like
    // DSH's wire request, the final full input has 54 items and no assistant tail.
    for round in 0..17 {
        let id = format!("old_{round}");
        items.extend([
            json!({"type":"function_call","call_id":id,"name":"read",
                "arguments":format!("{{\"path\":\"file_{round}.rs\"}}")}),
            json!({"type":"function_call_output","call_id":id,
                "output":format!("File {round}: original detailed tool result\n{}", "source line\n".repeat(32))}),
            json!({"role":"user","content":format!("Inspect file {} next", round + 1)}),
        ]);
    }
    let first = json!({"model":"fixture","tools":[],"input":items});
    let digest = ProviderTransportPayload::openai_responses(first.clone())
        .unwrap()
        .configuration_digest()
        .unwrap();
    let (repository, actor, run) = fixture_with_input(json!({"sys":{
        "requested_model_id":"fixture","responses_configuration_digest":digest
    }}))
    .await;
    let calls = json!([{"id":"latest","name":"read","arguments":{"path":"latest.rs"}}]);
    let output = vec![
        json!({"type":"function_call","call_id":"latest","name":"read",
        "arguments":"{\"path\":\"latest.rs\"}"}),
    ];
    let history = completed_history(&first, None, &output).unwrap().unwrap();
    let evidence = if semantic {
        json!({"tool_calls":calls,"responses_round":{
            "response_id":"resp_latest","history":history,"output":output
        }})
    } else {
        json!({"tool_calls":calls,"provider_metadata":{"native_response":{
            "response_id":"resp_latest","history":history,"configuration_digest":digest
        }}})
    };
    let callback = repository.seed_pending_llm_tool_callback_task(run, evidence);
    let mut full = first;
    full["input"].as_array_mut().unwrap().extend(output);
    full["input"].as_array_mut().unwrap().push(json!({
        "type":"function_call_output","call_id":"latest","output":"Unchanged latest result"
    }));
    (repository, actor, callback, full)
}

fn prune_old_outputs(mut full: Value) -> Value {
    for position in [17, 29, 35] {
        assert_eq!(full["input"][position]["type"], "function_call_output");
        full["input"][position]["output"] = json!("[Older tool result pruned by DSH]");
    }
    full
}

async fn has_grant(
    semantic: bool,
    repository: &ApplicationPublicApiTestRepository,
    actor: &ApplicationApiKeyActor,
    body: &Value,
) -> anyhow::Result<bool> {
    if semantic {
        Ok(correlate_semantic_responses_callback(
            repository,
            actor,
            &OpenAiResponsesEnvelope::capture(body.clone()).unwrap(),
        )
        .await?
        .is_some())
    } else {
        Ok(correlate_native_responses_callback(repository, actor, body)
            .await?
            .is_some())
    }
}

// Public callback reads intentionally omit private continuation evidence. Use the
// existing owned correlation query when asserting the entire stored receipt.
async fn owned_receipt(
    semantic: bool,
    repository: &ApplicationPublicApiTestRepository,
    actor: &ApplicationApiKeyActor,
) -> CallbackTaskRecord {
    let receipts = if semantic {
        repository
            .find_semantic_responses_callbacks_by_response_id(
                actor.workspace_id,
                actor.application_id,
                actor.api_key_id,
                actor.creator_user_id,
                "resp_latest",
            )
            .await
            .unwrap()
    } else {
        repository
            .find_native_responses_callbacks_by_response_id(
                actor.workspace_id,
                actor.application_id,
                actor.api_key_id,
                actor.creator_user_id,
                "resp_latest",
            )
            .await
            .unwrap()
    };
    assert_eq!(receipts.len(), 1);
    receipts.into_iter().next().unwrap()
}

#[tokio::test]
async fn pruned_owned_history_starts_sampling_without_mutating_receipts() {
    for semantic in [false, true] {
        for completed in [false, true] {
            let (repository, actor, callback, full) = historical_round(semantic).await;
            if completed {
                repository.complete_callback_task_for_test(callback.id);
            }
            let stored = owned_receipt(semantic, &repository, &actor).await;
            let attempts = repository.callback_resume_attempts();
            let history = if semantic {
                &stored.request_payload["responses_round"]["history"]
            } else {
                &stored.request_payload["provider_metadata"]["native_response"]["history"]
            };
            let ids = vec!["latest".to_owned()];
            assert_eq!(full["input"].as_array().unwrap().len(), 54);
            assert!(prove_full_context_input(&full["input"], history, &ids).is_ok());
            assert!(has_grant(semantic, &repository, &actor, &full)
                .await
                .unwrap());
            let pruned = prune_old_outputs(full.clone());
            assert_eq!(pruned["input"][52], full["input"][52]);
            assert_eq!(pruned["input"][53], full["input"][53]);
            assert!(is_closed_tool_history(pruned["input"].as_array().unwrap()));
            // Classification changes; the predecessor proof itself must still fail.
            assert!(prove_full_context_input(&pruned["input"], history, &ids).is_err());
            for refresh_configuration in [false, true] {
                let mut candidate = pruned.clone();
                if refresh_configuration {
                    candidate["model"] = json!("new-sampling-model");
                    candidate["instructions"] = json!("Use the self-contained pruned history");
                }
                for parent in [None, Some(Value::Null)] {
                    let mut candidate = candidate.clone();
                    if let Some(parent) = parent {
                        candidate["previous_response_id"] = parent;
                    }
                    assert!(!has_grant(semantic, &repository, &actor, &candidate)
                        .await
                        .unwrap());
                    let mut translated = translate_response_request(candidate.clone()).unwrap();
                    assert_eq!(
                        translated
                            .request
                            .metadata
                            .take_provider_transport_payload()
                            .unwrap()
                            .wire_body(),
                        &candidate
                    );
                }
            }
            assert_eq!(owned_receipt(semantic, &repository, &actor).await, stored);
            for mutation in ["user", "latest_arguments"] {
                let mut candidate = full.clone();
                if mutation == "user" {
                    candidate["input"][0]["content"] = json!("Rebuilt full user context");
                } else {
                    candidate["input"][52]["arguments"] = json!("{\"path\":\"changed.rs\"}");
                }
                assert!(is_closed_tool_history(
                    candidate["input"].as_array().unwrap()
                ));
                assert!(prove_full_context_input(&candidate["input"], history, &ids).is_err());
                assert!(!has_grant(semantic, &repository, &actor, &candidate)
                    .await
                    .unwrap());
            }
            assert_eq!(owned_receipt(semantic, &repository, &actor).await, stored);
            assert_eq!(repository.callback_resume_attempts(), attempts);
        }
    }
}

#[tokio::test]
async fn pruned_history_does_not_relax_cursor_configuration_or_round_guards() {
    for semantic in [false, true] {
        let (repository, actor, callback, full) = historical_round(semantic).await;
        let stored = owned_receipt(semantic, &repository, &actor).await;
        let attempts = repository.callback_resume_attempts();
        let mut delta = full.clone();
        delta["input"] = json!([full["input"][53].clone()]);
        for cursor in [None, Some("resp_latest")] {
            let mut candidate = delta.clone();
            if let Some(cursor) = cursor {
                candidate["previous_response_id"] = json!(cursor);
            }
            assert!(has_grant(semantic, &repository, &actor, &candidate)
                .await
                .unwrap());
        }
        let mut changed_configuration = full.clone();
        changed_configuration["model"] = json!("other-model");
        assert!(
            has_grant(semantic, &repository, &actor, &changed_configuration)
                .await
                .is_err()
        );
        let pruned = prune_old_outputs(full.clone());
        let mut wrong_cursor = pruned.clone();
        wrong_cursor["previous_response_id"] = json!("resp_wrong");
        assert!(has_grant(semantic, &repository, &actor, &wrong_cursor)
            .await
            .is_err());
        if !semantic {
            let mut changed_delta = delta;
            changed_delta["input"].as_array_mut().unwrap().insert(
                0,
                json!({"role":"user","content":"Unproven context without matching calls"}),
            );
            // Semantic's existing delta path accepts user context; native must
            // still reject unproven non-output context without matching calls.
            assert!(has_grant(semantic, &repository, &actor, &changed_delta)
                .await
                .is_err());
        }
        for mutation in ["duplicate", "partial", "malformed", "orphan", "family"] {
            let mut candidate = pruned.clone();
            match mutation {
                "duplicate" => candidate["input"].as_array_mut().unwrap().push(full["input"][53].clone()),
                "partial" => { candidate["input"].as_array_mut().unwrap().pop(); }
                "malformed" => candidate["input"][53]["call_id"] = json!(7),
                "orphan" => candidate["input"].as_array_mut().unwrap().insert(0,
                    json!({"type":"function_call_output","call_id":"orphan","output":"missing call"})),
                "family" => candidate["input"][53]["type"] = json!("custom_tool_call_output"),
                _ => unreachable!(),
            }
            assert!(
                !matches!(
                    has_grant(semantic, &repository, &actor, &candidate).await,
                    Ok(true)
                ),
                "{mutation}"
            );
        }
        let mut other = actor.clone();
        other.api_key_id = Uuid::now_v7();
        assert!(!matches!(
            has_grant(semantic, &repository, &other, &pruned).await,
            Ok(true)
        ));
        assert_eq!(owned_receipt(semantic, &repository, &actor).await, stored);
        assert_eq!(repository.callback_resume_attempts(), attempts);
        repository.seed_pending_llm_tool_callback_task(
            callback.flow_run_id,
            callback.request_payload.clone(),
        );
        assert!(has_grant(semantic, &repository, &actor, &pruned)
            .await
            .is_err());
        assert_eq!(repository.callback_resume_attempts(), attempts);
    }
}

#[tokio::test]
async fn closed_pruned_history_does_not_hide_missing_or_duplicate_owned_calls() {
    for semantic in [false, true] {
        let (_, _, original, full) = historical_round(semantic).await;
        let pruned = prune_old_outputs(full);
        for extra_id in ["latest", "missing"] {
            let (repository, actor, run) = fixture().await;
            let mut evidence = original.request_payload.clone();
            evidence["tool_calls"]
                .as_array_mut()
                .unwrap()
                .push(json!({"id":extra_id,"name":"read","arguments":{}}));
            let callback = repository.seed_pending_llm_tool_callback_task(run, evidence);
            assert!(is_closed_tool_history(pruned["input"].as_array().unwrap()));
            assert!(has_grant(semantic, &repository, &actor, &pruned)
                .await
                .is_err());
            assert_eq!(owned_receipt(semantic, &repository, &actor).await, callback);
            assert!(repository.callback_resume_attempts().is_empty());
        }
    }
}
