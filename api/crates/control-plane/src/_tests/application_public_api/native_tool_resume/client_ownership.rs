use super::*;

async fn public_admission(
    repository: &ApplicationPublicApiTestRepository,
    actor: &ApplicationApiKeyActor,
    body: Value,
) -> Result<Option<(CallbackTaskRecord, Value)>> {
    let envelope = OpenAiResponsesEnvelope::capture(body).unwrap();
    correlate_host_owned_responses_callback(repository, actor, &envelope).await
}

fn seed_host_round(
    repository: &ApplicationPublicApiTestRepository,
    run: Uuid,
    body: &Value,
) -> CallbackTaskRecord {
    let old = seed_round(repository, run, body);
    let mut payload = old.request_payload;
    payload["host_tool_continuation_owner"] = json!("agentflow");
    repository.seed_pending_llm_tool_callback_task(run, payload)
}

#[tokio::test]
async fn old_native_receipts_cannot_claim_client_history_or_warmup_cursors() {
    for completed in [false, true] {
        let (repository, actor, run) = fixture().await;
        let body = request();
        let old = seed_round(&repository, run, &body);
        if completed {
            repository.complete_callback_task_for_test(old.id);
        }
        // Controlled negative: the former public ingress rejects this legal
        // warmup cursor solely because old call IDs select another receipt.
        let mut old_ingress = body.clone();
        old_ingress["previous_response_id"] = json!("resp_warmup");
        assert!(
            correlate_native_responses_callback(&repository, &actor, &old_ingress)
                .await
                .unwrap_err()
                .to_string()
                .contains("native_tool_output_response_mismatch")
        );
        let before = repository
            .get_published_callback_task(old.id)
            .await
            .unwrap();
        for cursor in [None, Some("resp_round"), Some("resp_warmup")] {
            for generate in [true, false] {
                let mut candidate = body.clone();
                candidate["generate"] = json!(generate);
                if let Some(cursor) = cursor {
                    candidate["previous_response_id"] = json!(cursor);
                }
                // A client cannot manufacture host ownership in public JSON.
                candidate["host_tool_continuation_owner"] = json!("agentflow");
                assert!(public_admission(&repository, &actor, candidate)
                    .await
                    .unwrap()
                    .is_none());
            }
        }
        assert_eq!(
            repository
                .get_published_callback_task(old.id)
                .await
                .unwrap(),
            before
        );
        assert!(repository.callback_resume_attempts().is_empty());
    }
}

#[tokio::test]
async fn host_owned_round_retains_complete_set_validation_without_claiming_other_cursors() {
    let (repository, actor, run) = fixture().await;
    let body = request();
    let host = seed_host_round(&repository, run, &body);
    for cursor in [None, Some("resp_round")] {
        let mut candidate = body.clone();
        if let Some(cursor) = cursor {
            candidate["previous_response_id"] = json!(cursor);
        }
        let (actual, result) = public_admission(&repository, &actor, candidate.clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(actual.id, host.id);
        assert_eq!(result["tool_results"].as_array().unwrap().len(), 2);
        candidate["input"].as_array_mut().unwrap().pop();
        assert!(public_admission(&repository, &actor, candidate)
            .await
            .unwrap_err()
            .to_string()
            .contains("native_tool_output_incomplete_round"));
    }
    let mut warmup = body.clone();
    warmup["generate"] = json!(false);
    assert!(public_admission(&repository, &actor, warmup)
        .await
        .unwrap()
        .is_none());
    let mut other_cursor = body;
    other_cursor["previous_response_id"] = json!("resp_warmup");
    assert!(public_admission(&repository, &actor, other_cursor)
        .await
        .unwrap()
        .is_none());
    assert!(repository.callback_resume_attempts().is_empty());
}

#[tokio::test]
async fn unowned_native_outputs_do_not_enter_workflow_payload_validation() {
    let (repository, actor, _) = fixture().await;
    for input in [
        json!([{"type":"custom_tool_call_output","call_id":"unknown","output":{"future":"opaque"}}]),
        json!([{"type":"function_call_output","call_id":"unknown","output":"a"},
               {"type":"function_call_output","call_id":"unknown","output":"b"}]),
    ] {
        let body = json!({"model":"fixture","input":input});
        assert!(public_admission(&repository, &actor, body)
            .await
            .unwrap()
            .is_none());
    }
}

#[tokio::test]
async fn host_ownership_is_scoped_to_the_authenticated_key() {
    let (repository, actor, run) = fixture().await;
    let body = request();
    seed_host_round(&repository, run, &body);
    let mut other_actor = actor;
    other_actor.api_key_id = Uuid::now_v7();
    assert!(public_admission(&repository, &other_actor, body)
        .await
        .unwrap()
        .is_none());
}
