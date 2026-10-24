use super::*;
use control_plane::application_public_api::{
    api_keys::ApplicationApiKeyActor,
    callback_resume::{
        ApplicationPublishedCallbackResumeService, PreparedPublishedCallbackResume,
        PublishedCallbackReservation, PublishedCallbackResumeSource, PublishedCallbackResumeTarget,
        ResumePublishedCallbackCommand,
    },
    native_tool_resume::correlate_native_responses_callback,
};
use control_plane_contracts::{
    application_public_runtime::ApplicationPublishedCallbackAttemptRepository,
    ports::{ProviderTransportPayload, RenewResumeClaimInput},
};

#[tokio::test]
async fn aged_native_receipt_attaches_with_live_execution_lease_and_reclaims_only_after_expiry() {
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
    let waiting = persist_callback_wait(&store, &seeded, &run, &node, 1, None, None).await;
    let callback = waiting.callback_task.unwrap();
    let original = json!({"model":"fixture","input":[{"role":"user","content":"Read files"}]});
    let output =
        vec![json!({"type":"function_call","call_id":"call-1-0","name":"Bash","arguments":"{}"})];
    // Frozen v3 evidence for the user message and function-call prefix below.
    // Generated from history.rs canonical JSON + length-delimited SHA-256 chain;
    // v1 cannot attest full-context retries. The real service validates this proof.
    let history = json!({"version":3,"item_count":2,"digest":"80fd1237d37a782b3cf5a940e1163e372c6875248002141fbb70c64facd3aaa6"});
    let transport = ProviderTransportPayload::openai_responses(original.clone()).unwrap();
    let mut request_payload = callback.request_payload.clone();
    request_payload["provider_metadata"] = json!({"native_response":{
        "response_id":"resp_round",
        "configuration_digest":transport.configuration_digest().unwrap(),
        "user_messages_digest":transport.user_messages_digest().unwrap(),
        "history":history
    }});
    sqlx::query("update flow_run_callback_tasks set request_payload=$2 where id=$1")
        .bind(callback.id)
        .bind(request_payload)
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query("update flow_runs set api_key_id=$2, input_payload=$3 where id=$1")
        .bind(run.id)
        .bind(api_key_id)
        .bind(json!({"sys":{"requested_model_id":"fixture"}}))
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
    let delta = json!({"model":"fixture","previous_response_id":"resp_round","input":[
        {"type":"function_call_output","call_id":"call-1-0","output":"ok"}
    ]});
    let (_, payload) = correlate_native_responses_callback(&store, &actor, &delta)
        .await
        .unwrap()
        .unwrap();
    let (runtime, provider, _package) = semantic_runtime::seed_runtime_consumer(
        &store,
        &seeded,
        &compiled,
        Some(&waiting.checkpoint),
    )
    .await;
    let service = ApplicationPublishedCallbackResumeService::new(store.clone(), runtime);
    let command = ResumePublishedCallbackCommand {
        responses_continuation: None,
        transport_connection_scope: None,
        observation_context: None,
        reserved_attempt_id: None,
        native_transport: Some(ProviderTransportPayload::openai_responses(delta.clone()).unwrap()),
        bearer_token: String::new(),
        target: PublishedCallbackResumeTarget::CallbackTask {
            callback_task_id: callback.id,
        },
        source: PublishedCallbackResumeSource::OpenAiResponses,
        response_payload: payload.clone(),
        response_mode: Some("streaming".into()),
    };
    let PublishedCallbackReservation::Acquired(first) = service
        .reserve_native_callback_reservation_for_actor(actor.clone(), &command)
        .await
        .unwrap()
    else {
        panic!("new admission must be acquired");
    };
    assert!(service
        .release_unstarted_native_callback(&first)
        .await
        .unwrap());
    assert!(!service
        .release_unstarted_native_callback(&first)
        .await
        .unwrap());
    let PublishedCallbackReservation::Acquired(second) = service
        .reserve_native_callback_reservation_for_actor(actor.clone(), &command)
        .await
        .unwrap()
    else {
        panic!("released admission must be acquired");
    };
    assert_eq!(first.attempt_id(), second.attempt_id());
    assert_ne!(first, second);
    assert!(!service
        .release_unstarted_native_callback(&first)
        .await
        .unwrap());
    let attempt_id = second.attempt_id();
    let committed = store
        .commit_tool_callback_results(&CommitToolCallbackResultsInput {
            responses_continuation: None,
            scope_id: seeded.workspace_id,
            application_id: seeded.application_id,
            flow_run_id: run.id,
            checkpoint_id: waiting.checkpoint.id,
            callback_task_id: callback.id,
            results: vec![tool_result("call-1-0", json!("ok"))],
        })
        .await
        .unwrap();
    let claim = committed.claim.unwrap();
    assert!(!service
        .release_unstarted_native_callback(&second)
        .await
        .unwrap());

    sqlx::query("update flow_run_callback_resume_attempts set updated_at=now()-interval '6 minutes' where id=$1")
        .bind(attempt_id).execute(store.pool()).await.unwrap();
    // Reproduce a slow owner that renews its execution lease without rewriting
    // the callback receipt. Receipt age must not change retry admission.
    assert!(store
        .renew_resume_claim(&RenewResumeClaimInput {
            claim_id: claim.id,
            claim_token: claim.claim_token,
            expected_generation: claim.generation
        })
        .await
        .unwrap());
    assert!(store
        .has_active_published_callback_resume_claim(callback.id)
        .await
        .unwrap());
    let mut full = original;
    full["input"].as_array_mut().unwrap().extend(output);
    full["input"]
        .as_array_mut()
        .unwrap()
        .extend(delta["input"].as_array().unwrap().iter().cloned());
    let mut full_retry = command.clone();
    full_retry.native_transport = Some(ProviderTransportPayload::openai_responses(full).unwrap());
    for retry in [&command, &full_retry] {
        assert!(matches!(
            service
                .prepare_callback_resume_for_actor(actor.clone(), retry)
                .await
                .unwrap(),
            PreparedPublishedCallbackResume::Attach { .. }
        ));
    }
    let mut conflicting = command.clone();
    conflicting.response_payload["tool_results"][0]["content"] = json!("changed");
    assert!(matches!(
        service
            .prepare_callback_resume_for_actor(actor.clone(), &conflicting)
            .await
            .unwrap_err()
            .downcast_ref::<control_plane::errors::ControlPlaneError>(),
        Some(control_plane::errors::ControlPlaneError::Conflict(
            "callback_resume_payload_conflict"
        ))
    ));
    assert!(store
        .reclaim_expired_callback_resume_attempt(attempt_id, payload.clone())
        .await
        .unwrap()
        .is_none());
    let retained = store
        .get_published_callback_resume_attempt(callback.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.id, attempt_id);
    assert!(retained.updated_at + Duration::minutes(5) <= OffsetDateTime::now_utc());
    assert_eq!(
        provider
            .executions
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    assert!(provider.inputs.lock().unwrap().is_empty());

    let mut successor = full_retry.clone();
    let mut body = successor
        .native_transport
        .as_ref()
        .unwrap()
        .wire_body()
        .clone();
    body["input"]
        .as_array_mut()
        .unwrap()
        .push(json!({"role":"user","content":"Continue with new context"}));
    successor.native_transport = Some(ProviderTransportPayload::openai_responses(body).unwrap());
    let (reclassified, ()) = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        tokio::join!(
            service.prepare_callback_resume_for_actor(actor.clone(), &successor),
            async {
                tokio::time::sleep(std::time::Duration::from_millis(30)).await;
                sqlx::query("update flow_run_resume_claims set lease_expires_at=now()-interval '1 second' where id=$1")
                    .bind(claim.id).execute(store.pool()).await.unwrap();
            },
        )
    }).await.expect("expired owner must release the predecessor waiter");
    assert!(matches!(
        reclassified.unwrap(),
        PreparedPublishedCallbackResume::Resume { .. }
    ));
    assert!(!store
        .has_active_published_callback_resume_claim(callback.id)
        .await
        .unwrap());
    assert!(matches!(
        service
            .prepare_callback_resume_for_actor(actor.clone(), &command)
            .await
            .unwrap(),
        PreparedPublishedCallbackResume::Resume { .. }
    ));
    assert!(service
        .prepare_callback_resume_for_actor(actor.clone(), &conflicting)
        .await
        .is_err());
    let (left, right) = tokio::join!(
        service.reserve_native_callback_reservation_for_actor(actor.clone(), &command),
        service.reserve_native_callback_reservation_for_actor(actor.clone(), &command),
    );
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    let PublishedCallbackReservation::Acquired(reclaimed) = left.or(right).unwrap() else {
        panic!("expired admission must be acquired");
    };
    assert_eq!(reclaimed.attempt_id(), attempt_id);
    assert!(!service
        .release_unstarted_native_callback(&second)
        .await
        .unwrap());
    assert!(service
        .release_unstarted_native_callback(&reclaimed)
        .await
        .unwrap());
    assert!(matches!(
        service
            .prepare_callback_resume_for_actor(actor.clone(), &command)
            .await
            .unwrap(),
        PreparedPublishedCallbackResume::Resume { .. }
    ));
    let PublishedCallbackReservation::Acquired(retry) = service
        .reserve_native_callback_reservation_for_actor(actor.clone(), &command)
        .await
        .unwrap()
    else {
        panic!("completed released receipt must re-acquire without an age delay");
    };
    assert_ne!(retry, reclaimed);
    assert!(!service
        .release_unstarted_native_callback(&reclaimed)
        .await
        .unwrap());
    store
        .finish_published_callback_resume_attempt(&FinishFlowRunCallbackResumeAttemptInput {
            attempt_id,
            status: domain::FlowRunCallbackResumeAttemptStatus::Succeeded,
            error_payload: None,
            completed_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    assert!(!service
        .release_unstarted_native_callback(&retry)
        .await
        .unwrap());
    assert!(
        matches!(service.reserve_native_callback_reservation_for_actor(actor, &command).await.unwrap(),
        PublishedCallbackReservation::SettledReplay { attempt_id: settled } if settled == attempt_id)
    );
    assert_eq!(
        store
            .get_published_callback_resume_attempt(callback.id)
            .await
            .unwrap()
            .unwrap()
            .id,
        attempt_id
    );
}
