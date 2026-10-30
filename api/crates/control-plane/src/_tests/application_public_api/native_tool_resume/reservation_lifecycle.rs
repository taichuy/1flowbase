use super::*;
use crate::application_public_api::callback_resume::{
    ApplicationPublishedCallbackResumeService, PreparedPublishedCallbackResume,
    PublishedCallbackReservation, PublishedCallbackResumeSource, PublishedCallbackResumeTarget,
    ResumePublishedCallbackCommand,
};
use crate::application_public_api::run_service::ApplicationPublishedRunControlRepository;

type ReservationService = ApplicationPublishedCallbackResumeService<
    ApplicationPublicApiTestRepository,
    RecordingNativeCallbackConsumer,
>;

async fn reservation_fixture() -> (
    ApplicationPublicApiTestRepository,
    ApplicationApiKeyActor,
    Uuid,
    ResumePublishedCallbackCommand,
    ReservationService,
) {
    let (repository, actor, run) =
        fixture_with_input(json!({"sys":{"requested_model_id":"fixture"}})).await;
    let (callback, body) = seed_proven_full_round(&repository, run, true);
    let (_, payload) = correlate_native_responses_callback(&repository, &actor, &body)
        .await
        .unwrap()
        .unwrap();
    let command = ResumePublishedCallbackCommand {
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
        response_payload: payload,
        response_mode: Some("streaming".into()),
    };
    let service = ApplicationPublishedCallbackResumeService::new(
        repository.clone(),
        RecordingNativeCallbackConsumer {
            repository: repository.clone(),
            calls: Default::default(),
        },
    );
    (repository, actor, callback.id, command, service)
}

#[tokio::test]
async fn owned_unstarted_release_reacquires_completed_callback_and_fences_old_receipt() {
    let (repository, actor, callback, mut command, service) = reservation_fixture().await;
    let PublishedCallbackReservation::Acquired(first) = service
        .reserve_native_callback_reservation_for_actor(actor.clone(), &command)
        .await
        .unwrap()
    else {
        panic!("first admission must be acquired");
    };
    repository.complete_callback_task_for_test(callback);
    assert!(service
        .release_unstarted_native_callback(&first)
        .await
        .unwrap());
    assert!(!service
        .release_unstarted_native_callback(&first)
        .await
        .unwrap());
    assert!(matches!(
        service
            .prepare_callback_resume_for_actor(actor.clone(), &command)
            .await
            .unwrap(),
        PreparedPublishedCallbackResume::Resume { .. }
    ));
    for source_conflict in [false, true] {
        let mut changed = command.clone();
        if source_conflict {
            changed.source = PublishedCallbackResumeSource::OpenAiChat;
        } else {
            changed.response_payload["tool_results"][0]["content"] = json!("changed");
        }
        assert!(service
            .reserve_native_callback_reservation_for_actor(actor.clone(), &changed)
            .await
            .is_err());
    }
    let PublishedCallbackReservation::Acquired(second) = service
        .reserve_native_callback_reservation_for_actor(actor.clone(), &command)
        .await
        .unwrap()
    else {
        panic!("released completed callback must be acquired immediately");
    };
    assert_eq!(first.attempt_id(), second.attempt_id());
    assert_ne!(first, second);
    assert!(!service
        .release_unstarted_native_callback(&first)
        .await
        .unwrap());
    assert_eq!(
        repository.callback_resume_attempts()[0].status,
        domain::FlowRunCallbackResumeAttemptStatus::Processing
    );
    command.reserved_attempt_id = Some(second.attempt_id());
    service
        .resume_callback_for_actor(actor.clone(), command.clone())
        .await
        .unwrap();
    assert!(!service
        .release_unstarted_native_callback(&second)
        .await
        .unwrap());
    assert!(
        matches!(service.reserve_native_callback_reservation_for_actor(actor, &command).await.unwrap(),
        PublishedCallbackReservation::SettledReplay { attempt_id } if attempt_id == second.attempt_id())
    );
    assert_eq!(repository.callback_resume_attempts().len(), 1);
}

#[tokio::test]
async fn unstarted_release_refuses_new_active_execution_claim() {
    let (repository, actor, callback, command, service) = reservation_fixture().await;
    let PublishedCallbackReservation::Acquired(receipt) = service
        .reserve_native_callback_reservation_for_actor(actor, &command)
        .await
        .unwrap()
    else {
        panic!("first admission must be acquired");
    };
    repository.set_callback_resume_claim_lease_for_test(
        callback,
        OffsetDateTime::now_utc() + time::Duration::minutes(5),
    );
    assert!(!service
        .release_unstarted_native_callback(&receipt)
        .await
        .unwrap());
    assert_eq!(
        repository.callback_resume_attempts()[0].status,
        domain::FlowRunCallbackResumeAttemptStatus::Processing
    );
    repository.set_callback_resume_claim_lease_for_test(
        callback,
        OffsetDateTime::now_utc() - time::Duration::seconds(1),
    );
    assert!(service
        .release_unstarted_native_callback(&receipt)
        .await
        .unwrap());
}

#[tokio::test]
async fn waiting_predecessor_reclassifies_when_aged_execution_owner_expires() {
    let (repository, actor, callback, mut command, service) = reservation_fixture().await;
    service
        .reserve_native_callback_reservation_for_actor(actor.clone(), &command)
        .await
        .unwrap();
    repository.complete_callback_task_for_test(callback);
    repository.expire_callback_resume_attempt_for_test(callback);
    repository.set_callback_resume_claim_lease_for_test(
        callback,
        OffsetDateTime::now_utc() + time::Duration::minutes(5),
    );
    let mut body = command
        .native_transport
        .as_ref()
        .unwrap()
        .wire_body()
        .clone();
    body["input"]
        .as_array_mut()
        .unwrap()
        .push(json!({"role":"user","content":"Continue with new context"}));
    command.native_transport = Some(ProviderTransportPayload::openai_responses(body).unwrap());
    let waiter = tokio::spawn(async move {
        service
            .prepare_callback_resume_for_actor(actor, &command)
            .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    assert!(!waiter.is_finished());
    repository.set_callback_resume_claim_lease_for_test(
        callback,
        OffsetDateTime::now_utc() - time::Duration::seconds(1),
    );
    assert!(matches!(
        tokio::time::timeout(std::time::Duration::from_secs(2), waiter)
            .await
            .expect("expired owner must reclassify without the 240s predecessor timeout")
            .unwrap()
            .unwrap(),
        PreparedPublishedCallbackResume::Resume { .. }
    ));
}

#[tokio::test]
async fn native_tool_read_plan_reuses_parked_attempt_without_changing_generic_replay_rules() {
    let (repository, actor, callback, mut command, service) = reservation_fixture().await;
    command.source = PublishedCallbackResumeSource::NativeAgent;
    command.native_transport = None;
    command.responses_continuation = None;
    let flow_run_id = repository
        .get_published_callback_task(callback)
        .await
        .unwrap()
        .unwrap()
        .flow_run_id;
    command.target = PublishedCallbackResumeTarget::FlowRun {
        flow_run_id,
        callback_task_id: callback,
    };
    assert!(matches!(
        service
            .prepare_callback_resume_for_actor(actor.clone(), &command)
            .await
            .unwrap(),
        PreparedPublishedCallbackResume::NativeToolResume { .. }
    ));
    assert!(
        repository.callback_resume_attempts().is_empty(),
        "classification is read only"
    );
    let PublishedCallbackReservation::Acquired(first) = service
        .reserve_native_callback_reservation_for_actor(actor.clone(), &command)
        .await
        .unwrap()
    else {
        panic!("existing CAS must own Native tool admission")
    };
    let processing = repository.callback_resume_attempts();
    assert!(service
        .prepare_callback_resume_for_actor(actor.clone(), &command)
        .await
        .is_err());
    assert_eq!(repository.callback_resume_attempts(), processing);
    assert!(service
        .release_unstarted_native_callback(&first)
        .await
        .unwrap());
    for changed_payload in [false, true] {
        let mut next = command.clone();
        if changed_payload {
            next.response_payload =
                json!({"tool_results":[{"tool_call_id":"custom","content":"later"}]});
        }
        let parked = repository.callback_resume_attempts();
        assert!(matches!(
            service
                .prepare_callback_resume_for_actor(actor.clone(), &next)
                .await
                .unwrap(),
            PreparedPublishedCallbackResume::NativeToolResume { .. }
        ));
        assert_eq!(
            repository.callback_resume_attempts(),
            parked,
            "the plan never changes request/fence"
        );
        let PublishedCallbackReservation::Acquired(owner) = service
            .reserve_native_callback_reservation_for_actor(actor.clone(), &next)
            .await
            .unwrap()
        else {
            panic!("the original reservation owner must reacquire")
        };
        assert_eq!(first.attempt_id(), owner.attempt_id());
        assert_ne!(first, owner);
        assert!(!service
            .release_unstarted_native_callback(&first)
            .await
            .unwrap());
        assert!(service
            .release_unstarted_native_callback(&owner)
            .await
            .unwrap());
    }
    // Both read classifications may succeed, but only the existing CAS can win.
    for _ in 0..2 {
        assert!(matches!(
            service
                .prepare_callback_resume_for_actor(actor.clone(), &command)
                .await
                .unwrap(),
            PreparedPublishedCallbackResume::NativeToolResume { .. }
        ));
    }
    let (one, two) = tokio::join!(
        service.reserve_native_callback_reservation_for_actor(actor.clone(), &command),
        service.reserve_native_callback_reservation_for_actor(actor.clone(), &command),
    );
    assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
    let PublishedCallbackReservation::Acquired(winner) = one.or(two).unwrap() else {
        panic!("only a fenced acquisition grants the prepared route")
    };
    assert!(!service
        .release_unstarted_native_callback(&first)
        .await
        .unwrap());
    assert!(service
        .release_unstarted_native_callback(&winner)
        .await
        .unwrap());
    let mut foreign = command.clone();
    foreign.source = PublishedCallbackResumeSource::OpenAiChat;
    let before = repository.callback_resume_attempts();
    assert!(service
        .prepare_callback_resume_for_actor(actor, &foreign)
        .await
        .is_err());
    assert_eq!(repository.callback_resume_attempts(), before);
}

#[tokio::test]
async fn native_tool_read_plan_excludes_non_tools_other_sources_and_non_pending_runs() {
    for source in [
        PublishedCallbackResumeSource::NativeAgent,
        PublishedCallbackResumeSource::OpenAiResponses,
        PublishedCallbackResumeSource::OpenAiChat,
        PublishedCallbackResumeSource::AnthropicMessages,
    ] {
        let (repository, actor, run) = fixture().await;
        let callback = repository.seed_pending_callback_task(run);
        let service = ApplicationPublishedCallbackResumeService::new(
            repository.clone(),
            RecordingNativeCallbackConsumer {
                repository: repository.clone(),
                calls: Default::default(),
            },
        );
        let command = ResumePublishedCallbackCommand {
            responses_continuation: None,
            transport_connection_scope: None,
            observation_context: None,
            reserved_attempt_id: None,
            native_transport: None,
            bearer_token: String::new(),
            target: PublishedCallbackResumeTarget::FlowRun {
                flow_run_id: run,
                callback_task_id: callback.id,
            },
            source,
            response_payload: json!({"answer":"yes"}),
            response_mode: Some("streaming".into()),
        };
        assert!(
            matches!(
                service
                    .prepare_callback_resume_for_actor(actor, &command)
                    .await
                    .unwrap(),
                PreparedPublishedCallbackResume::Resume { .. }
            ),
            "non-tool callbacks retain ordinary execution"
        );
        assert!(repository.callback_resume_attempts().is_empty());
    }
    for variant in ["responses", "chat", "anthropic", "running", "completed"] {
        let (repository, actor, callback, mut command, service) = reservation_fixture().await;
        let flow_run_id = repository
            .get_published_callback_task(callback)
            .await
            .unwrap()
            .unwrap()
            .flow_run_id;
        command.source = PublishedCallbackResumeSource::NativeAgent;
        command.native_transport = None;
        match variant {
            "responses" => command.source = PublishedCallbackResumeSource::OpenAiResponses,
            "chat" => command.source = PublishedCallbackResumeSource::OpenAiChat,
            "anthropic" => command.source = PublishedCallbackResumeSource::AnthropicMessages,
            "running" => {
                repository.set_flow_run_status_for_test(flow_run_id, domain::FlowRunStatus::Running)
            }
            "completed" => repository.complete_callback_task_for_test(callback),
            _ => unreachable!(),
        }
        assert!(
            matches!(
                service
                    .prepare_callback_resume_for_actor(actor, &command)
                    .await
                    .unwrap(),
                PreparedPublishedCallbackResume::Resume { .. }
            ),
            "{variant}"
        );
        assert!(repository.callback_resume_attempts().is_empty());
    }
}
