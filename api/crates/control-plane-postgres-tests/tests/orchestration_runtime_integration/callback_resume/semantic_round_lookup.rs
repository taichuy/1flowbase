use super::*;
use control_plane::application_public_api::{
    api_keys::ApplicationApiKeyActor, compat::openai::OpenAiResponsesEnvelope,
    native_tool_resume::correlate_semantic_responses_callback,
};
use control_plane_contracts::application_public_runtime::ApplicationPublishedRunControlRepository;

// Public round two IDs use the preceding callback ID. A mutable flow output must
// never make that completed callback appear to own the next round's evidence.
#[tokio::test]
async fn semantic_two_round_lookup_preserves_completed_receipt_and_rejects_real_duplicates() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = OffsetDateTime::now_utc();
    let run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        started_at,
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    let api_key_id = seed_application_api_key(&store, &seeded).await;
    let node = seed_node_run(&store, &run, started_at).await;
    let first = persist_callback_wait(&store, &seeded, &run, &node, 1, None, None).await;
    let first_callback = first.callback_task.unwrap();
    let first_claim = store
        .acquire_resume_claim(&AcquireResumeClaimInput {
            scope_id: seeded.workspace_id,
            application_id: seeded.application_id,
            flow_run_id: run.id,
            checkpoint_id: first.checkpoint.id,
            callback_task_id: Some(first_callback.id),
            kind: ResumeClaimKind::Callback,
            request_payload: json!({
                "tool_results":[{"tool_call_id":"call-1-0","content":"ok"}]
            }),
        })
        .await
        .unwrap();
    assert_eq!(first_claim.disposition, ResumeClaimDisposition::Acquired);
    let second = persist_callback_wait(
        &store,
        &seeded,
        &run,
        &node,
        2,
        Some(first.recovery_history.context_version_id),
        Some((first_claim.claim.id, first_claim.claim.claim_token)),
    )
    .await;
    let second_callback = second.callback_task.unwrap();
    let first_response_id = format!("resp_{}", run.id);
    let second_response_id = format!("resp_{}", first_callback.id);
    let request = |response_id: &str, call_id: &str| {
        json!({
            "model":"fixture", "previous_response_id":response_id,
            "input":[{"type":"function_call_output","call_id":call_id,"output":"ok"}]
        })
    };
    let first_request = request(&first_response_id, "call-1-0");
    let second_request = request(&second_response_id, "call-2-0");
    let digest = control_plane_contracts::ports::ProviderTransportPayload::openai_responses(
        first_request.clone(),
    )
    .unwrap()
    .configuration_digest()
    .unwrap();
    let round = |response_id: &str| {
        json!({
            "response_id":response_id,
            "history":{"version":2,"item_count":2,"digest":"0".repeat(64)},
            "output":[]
        })
    };
    let receipt = json!({"tool_results":[{"tool_call_id":"call-1-0","content":"ok"}]});
    for (callback, response_id) in [
        (&first_callback, &first_response_id),
        (&second_callback, &second_response_id),
    ] {
        let mut payload = callback.request_payload.clone();
        payload["responses_round"] = round(response_id);
        sqlx::query("update flow_run_callback_tasks set request_payload=$2 where id=$1")
            .bind(callback.id)
            .bind(payload)
            .execute(store.pool())
            .await
            .unwrap();
    }
    sqlx::query("update flow_run_callback_tasks set status='completed', response_payload=$2, completed_at=now() where id=$1")
        .bind(first_callback.id).bind(&receipt).execute(store.pool()).await.unwrap();
    sqlx::query(
        "update flow_runs set api_key_id=$2, input_payload=$3, output_payload=$4 where id=$1",
    )
    .bind(run.id)
    .bind(api_key_id)
    .bind(json!({"sys":{"responses_configuration_digest":digest}}))
    .bind(json!({"responses_round":round(&second_response_id)}))
    .execute(store.pool())
    .await
    .unwrap();
    let actor = ApplicationApiKeyActor {
        api_key_id,
        application_id: seeded.application_id,
        creator_user_id: seeded.actor_user_id,
        tenant_id: root_tenant_id(&store).await,
        workspace_id: seeded.workspace_id,
        actor: domain::ActorContext::scoped_in_scope(
            seeded.actor_user_id,
            Uuid::nil(),
            seeded.workspace_id,
            "application_api_key",
            Vec::<String>::new(),
        ),
    };
    for (response_id, expected, status) in [
        (
            &second_response_id,
            second_callback.id,
            CallbackTaskStatus::Pending,
        ),
        (
            &first_response_id,
            first_callback.id,
            CallbackTaskStatus::Completed,
        ),
    ] {
        let found = store
            .find_semantic_responses_callbacks_by_response_id(
                actor.workspace_id,
                actor.application_id,
                actor.api_key_id,
                actor.creator_user_id,
                response_id,
            )
            .await
            .unwrap();
        assert_eq!(
            found.len(),
            1,
            "each response belongs to exactly one immutable callback"
        );
        assert_eq!(found[0].id, expected);
        assert_eq!(found[0].status, status);
        if expected == first_callback.id {
            assert_eq!(found[0].response_payload.as_ref(), Some(&receipt));
        }
    }
    for (wire, expected) in [
        (first_request, first_callback.id),
        (second_request.clone(), second_callback.id),
    ] {
        let envelope = OpenAiResponsesEnvelope::capture(wire).unwrap();
        let grant = correlate_semantic_responses_callback(&store, &actor, &envelope)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(grant.callback_task_id(), expected);
    }
    // All ownership filters continue to apply to immutable evidence, including retries.
    for index in 0..4 {
        let mut scope = [
            actor.workspace_id,
            actor.application_id,
            actor.api_key_id,
            actor.creator_user_id,
        ];
        scope[index] = Uuid::now_v7();
        for response_id in [&first_response_id, &second_response_id] {
            assert!(store
                .find_semantic_responses_callbacks_by_response_id(
                    scope[0],
                    scope[1],
                    scope[2],
                    scope[3],
                    response_id,
                )
                .await
                .unwrap()
                .is_empty());
        }
    }
    // Genuine duplicate immutable evidence must still fail closed, even when one
    // callback is completed; filtering to pending would hide this corruption.
    sqlx::query("update flow_run_callback_tasks set request_payload=jsonb_set(request_payload, '{responses_round}', $2) where id=$1")
        .bind(first_callback.id).bind(round(&second_response_id))
        .execute(store.pool()).await.unwrap();
    let envelope = OpenAiResponsesEnvelope::capture(second_request).unwrap();
    let error = correlate_semantic_responses_callback(&store, &actor, &envelope)
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<control_plane::errors::ControlPlaneError>(),
        Some(control_plane::errors::ControlPlaneError::Conflict(
            "responses_tool_output_ambiguous_round"
        ))
    ));
}
