use super::*;
use control_plane::application_public_api::{
    callback_resume::{
        PreparedPublishedCallbackResume, PublishedCallbackReservationReceipt,
        ResumePublishedCallbackResult,
    },
    native_tool_resume::correlate_native_responses_callback,
};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use tokio::sync::{Barrier, Notify};

fn callback_id(fixture: &Fixture) -> Uuid {
    match fixture.command.target {
        PublishedCallbackResumeTarget::CallbackTask { callback_task_id } => callback_task_id,
        _ => unreachable!("fixture targets one callback"),
    }
}

async fn native_fixture(pool: Option<sqlx::PgPool>) -> Fixture {
    let mut fixture = match pool {
        Some(pool) => fixture_with_pool(pool, 1).await,
        None => fixture(1).await,
    };
    // Only this fixture's temporary provider advertises native support. The
    // production provider/installation and its version are never changed.
    let manifest = fixture._package.0.join("manifest.yaml");
    let text = std::fs::read_to_string(&manifest).unwrap();
    let marker = "  entry: bin/fixture_provider-provider\n";
    assert_eq!(text.matches(marker).count(), 1);
    std::fs::write(
        manifest,
        text.replace(marker, &format!("{marker}  capabilities:\n    - responses.native_passthrough\n    - responses.native_output.v1\n    - native_continuation_supported\n")),
    ).unwrap();
    let request = json!({"model":"gpt-5.4-mini","input":[{
        "type":"function_call_output","call_id":"call-1-0",
        "output":"complete retry output: first\nlast"
    }]});
    let transport = ProviderTransportPayload::openai_responses(request.clone()).unwrap();
    let id = callback_id(&fixture);
    let mut payload: Value =
        sqlx::query_scalar("select request_payload from flow_run_callback_tasks where id=$1")
            .bind(id)
            .fetch_one(fixture.store.pool())
            .await
            .unwrap();
    payload.as_object_mut().unwrap().remove("responses_round");
    payload["provider_metadata"] = json!({"native_response":{
        "response_id":"resp_pending_retry",
        "configuration_digest":transport.configuration_digest().unwrap(),
        "history":current_callback_history_fixture()
    }});
    sqlx::query("update flow_run_callback_tasks set request_payload=$2, raw_json_payloads=raw_json_payloads-'request_payload' where id=$1")
        .bind(id).bind(payload).execute(fixture.store.pool()).await.unwrap();
    let (callback, response_payload) =
        correlate_native_responses_callback(&fixture.store, &fixture.actor, &request)
            .await
            .unwrap()
            .expect("real native ingress must correlate the owned output-only request");
    assert_eq!(callback.id, id);
    fixture.command.responses_continuation = None;
    fixture.command.native_transport = Some(transport);
    fixture.command.response_payload = response_payload;
    // The checkpoint helper normally ends at the LLM. Keep an actual answer
    // consumer here so completion also proves the entire provider answer survives.
    let mut plan: Value = sqlx::query_scalar("select p.plan from flow_compiled_plans p join flow_runs r on r.compiled_plan_id=p.id where r.id=$1")
        .bind(callback.flow_run_id).fetch_one(fixture.store.pool()).await.unwrap();
    plan["topological_order"] = json!(["node-llm", "node-answer"]);
    plan["nodes"]["node-llm"]["downstream_node_ids"] = json!(["node-answer"]);
    plan["nodes"]["node-answer"] = json!({
        "node_id":"node-answer","node_type":"answer","alias":"Answer","container_id":null,
        "dependency_node_ids":["node-llm"],"downstream_node_ids":[],
        "bindings":{"answer_template":{"kind":"selector","selector_paths":[["node-llm","text"]],"raw_value":["node-llm","text"]}},
        "outputs":[{"key":"answer","title":"Answer","value_type":"string","selector":[]}],"config":{}
    });
    sqlx::query("update flow_compiled_plans p set plan=$2 from flow_runs r where r.id=$1 and p.id=r.compiled_plan_id")
        .bind(callback.flow_run_id).bind(plan).execute(fixture.store.pool()).await.unwrap();
    fixture
}

async fn reserve(fixture: &Fixture) -> PublishedCallbackReservationReceipt {
    let PublishedCallbackReservation::Acquired(receipt) = fixture
        .service
        .reserve_native_callback_reservation_for_actor(fixture.actor.clone(), &fixture.command)
        .await
        .unwrap()
    else {
        panic!("pending retry must acquire its durable admission");
    };
    receipt
}

async fn prepare(fixture: &Fixture) {
    assert!(matches!(
        fixture
            .service
            .prepare_callback_resume_for_actor(fixture.actor.clone(), &fixture.command)
            .await
            .expect("Pending/Received retry must reach reservation, rather than terminal replay"),
        PreparedPublishedCallbackResume::Resume { .. }
    ));
}

async fn assert_unstarted(fixture: &Fixture) {
    let row: (String, String, i64, i64) = sqlx::query_as(
        "select c.status,a.status,(select count(*) from flow_run_resume_claims r where r.callback_task_id=c.id),(select count(*) from flow_run_tool_callback_inbox i where i.callback_task_id=c.id and i.status='received') from flow_run_callback_tasks c join flow_run_callback_resume_attempts a on a.callback_task_id=c.id where c.id=$1",
    ).bind(callback_id(fixture)).fetch_one(fixture.store.pool()).await.unwrap();
    assert_eq!(row, ("pending".into(), "received".into(), 0, 0));
    assert!(fixture.provider.inputs.lock().unwrap().is_empty());
}

async fn execute_until_provider(
    fixture: &Fixture,
    receipt: PublishedCallbackReservationReceipt,
) -> tokio::task::JoinHandle<anyhow::Result<ResumePublishedCallbackResult>> {
    let PreparedPublishedCallback::Execution(owner) = fixture
        .service
        .prepare_reserved_native_callback_for_actor(
            fixture.actor.clone(),
            fixture.command.clone(),
            receipt,
        )
        .await
        .unwrap()
    else {
        panic!("complete native result must prepare execution");
    };
    let service = fixture.service.clone();
    let task = tokio::spawn(async move { service.execute_prepared_callback(*owner).await });
    fixture.provider.wait_until_entered().await;
    let counts: (i64, i64, i64) = sqlx::query_as(
        "select (select count(*) from flow_run_callback_resume_attempts where callback_task_id=$1),(select count(*) from flow_run_resume_claims where callback_task_id=$1 and status='processing' and lease_expires_at>now()),(select count(*) from flow_run_tool_callback_inbox where callback_task_id=$1 and status='received')",
    ).bind(callback_id(fixture)).fetch_one(fixture.store.pool()).await.unwrap();
    assert_eq!(counts, (1, 1, 1));
    let inputs = fixture.provider.inputs.lock().unwrap();
    assert_eq!(
        inputs.len(),
        1,
        "one real provider continuation before release"
    );
    assert_eq!(
        inputs[0].native_transport.as_ref().unwrap().wire_body,
        *fixture
            .command
            .native_transport
            .as_ref()
            .unwrap()
            .wire_body(),
        "the complete accepted native output must reach the provider unchanged"
    );
    task
}

async fn finish(
    fixture: &Fixture,
    task: tokio::task::JoinHandle<anyhow::Result<ResumePublishedCallbackResult>>,
    attempt_id: Uuid,
) {
    fixture.provider.release.add_permits(1);
    let result = tokio::time::timeout(std::time::Duration::from_secs(20), task)
        .await
        .expect("released provider must complete")
        .unwrap()
        .unwrap();
    assert_eq!(result.attempt.id, attempt_id);
    assert_eq!(
        result.attempt.status,
        domain::FlowRunCallbackResumeAttemptStatus::Succeeded
    );
    assert_eq!(result.run.answer.as_deref(), Some("recovered"));
    let accepted: Value = sqlx::query_scalar("select runtime_original_json(result_payload,raw_json_payloads,'result_payload') from flow_run_tool_callback_inbox where callback_task_id=$1")
        .bind(callback_id(fixture)).fetch_one(fixture.store.pool()).await.unwrap();
    assert_eq!(
        accepted,
        fixture.command.response_payload["tool_results"][0]
    );
    // The attached contender observes the same complete durable terminal; it
    // never invokes the consumer a second time.
    prepare(fixture).await;
    let replay = fixture
        .service
        .reserve_native_callback_reservation_for_actor(fixture.actor.clone(), &fixture.command)
        .await
        .unwrap();
    assert!(
        matches!(replay, PublishedCallbackReservation::SettledReplay { attempt_id: id } if id == attempt_id)
    );
    let replay = fixture
        .service
        .resume_callback_for_actor(fixture.actor.clone(), fixture.command.clone())
        .await
        .unwrap();
    assert_eq!(replay.run.answer, result.run.answer);
    assert_eq!(fixture.provider.inputs.lock().unwrap().len(), 1);
    assert_eq!(fixture.provider.executions.load(Ordering::SeqCst), 1);
    let counts: (i64, i64, i64) = sqlx::query_as(
        "select (select count(*) from flow_run_callback_resume_attempts where callback_task_id=$1),(select count(*) from flow_run_resume_claims where callback_task_id=$1),(select count(*) from flow_run_tool_callback_inbox where callback_task_id=$1 and status='received')",
    ).bind(callback_id(fixture)).fetch_one(fixture.store.pool()).await.unwrap();
    assert_eq!(counts, (1, 1, 1));
}

#[tokio::test]
async fn pending_native_retry_after_preclaim_release_executes_once() {
    let fixture = native_fixture(None).await;
    prepare(&fixture).await;
    let stale = reserve(&fixture).await;
    // A host setup failure occurs after admission but before preparing any
    // execution. The API fixture exercises the actual failing invoker await.
    let failure = Notify::new();
    let released = Notify::new();
    let release = async {
        failure.notified().await;
        assert!(fixture
            .service
            .release_unstarted_native_callback(&stale)
            .await
            .unwrap());
        released.notify_one();
    };
    let retry = async {
        failure.notify_one();
        released.notified().await;
        assert_unstarted(&fixture).await;
        for (field, expected) in [
            ("payload", "callback_resume_payload_conflict"),
            ("source", "callback_resume_source_conflict"),
        ] {
            let mut changed = fixture.command.clone();
            if field == "payload" {
                changed.response_payload["tool_results"][0]["content"] = json!("different");
            } else {
                changed.source = PublishedCallbackResumeSource::NativeAgent;
            }
            let error = fixture
                .service
                .prepare_callback_resume_for_actor(fixture.actor.clone(), &changed)
                .await
                .unwrap_err();
            assert_eq!(
                error.downcast_ref::<control_plane::errors::ControlPlaneError>(),
                Some(&control_plane::errors::ControlPlaneError::Conflict(
                    expected
                ))
            );
        }
        prepare(&fixture).await;
        let receipt = reserve(&fixture).await;
        assert_eq!(receipt.attempt_id(), stale.attempt_id());
        assert!(!fixture
            .service
            .release_unstarted_native_callback(&stale)
            .await
            .unwrap());
        let execution = execute_until_provider(&fixture, receipt).await;
        finish(&fixture, execution, stale.attempt_id()).await;
    };
    tokio::join!(release, retry);
}

#[tokio::test]
async fn pending_native_retry_processing_follower_refreshes_after_release() {
    // Observe real SELECTs, without replacing the repository or adding a
    // production hook. The second attempt read can only be the predecessor
    // polling loop: initial context/attempt reads have already completed.
    let owner_pool = isolated_database().await.connect().await.unwrap();
    let enabled = Arc::new(AtomicBool::new(false));
    let pid = Arc::new(AtomicI32::new(0));
    let reads = Arc::new(AtomicUsize::new(0));
    let waiting = Arc::new(Notify::new());
    let unblock = Arc::new(Notify::new());
    let observed = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .after_release({
            let (enabled, pid, reads, waiting, unblock, monitor) = (
                enabled.clone(),
                pid.clone(),
                reads.clone(),
                waiting.clone(),
                unblock.clone(),
                owner_pool.clone(),
            );
            move |_, _| {
                let (enabled, pid, reads, waiting, unblock, monitor) = (
                    enabled.clone(),
                    pid.clone(),
                    reads.clone(),
                    waiting.clone(),
                    unblock.clone(),
                    monitor.clone(),
                );
                Box::pin(async move {
                    if enabled.load(Ordering::SeqCst) {
                        let query: String =
                            sqlx::query_scalar("select query from pg_stat_activity where pid=$1")
                                .bind(pid.load(Ordering::SeqCst))
                                .fetch_one(&monitor)
                                .await
                                .unwrap();
                        if query.contains("from flow_run_callback_resume_attempts")
                            && query.contains("where callback_task_id = $1")
                            && reads.fetch_add(1, Ordering::SeqCst) == 1
                        {
                            waiting.notify_one();
                            unblock.notified().await;
                        }
                    }
                    Ok(true)
                })
            }
        })
        .connect_with((*owner_pool.connect_options()).clone())
        .await
        .unwrap();
    let fixture = native_fixture(Some(observed.clone())).await;
    let stale = reserve(&fixture).await;
    let backend: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&observed)
        .await
        .unwrap();
    pid.store(backend, Ordering::SeqCst);
    let release_store = PgControlPlaneStore::new(owner_pool.clone());
    let release_runtime = control_plane::orchestration_runtime::OrchestrationRuntimeService::new(
        release_store.clone(),
        fixture.provider.clone(),
        Arc::new(runtime_core::runtime_engine::RuntimeEngine::for_tests()),
        "fixture-key",
        fixture.provider.transport_store.clone(),
        false,
    )
    .with_node_artifact_context("semantic-fixture", fixture._package.0.clone());
    let release_service = Service::new(release_store, release_runtime);
    enabled.store(true, Ordering::SeqCst);
    let service = fixture.service.clone();
    let actor = fixture.actor.clone();
    let command = fixture.command.clone();
    let follower = tokio::spawn(async move {
        service
            .prepare_callback_resume_for_actor(actor, &command)
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), waiting.notified())
        .await
        .expect("follower must enter the real Processing predecessor poll before release");
    enabled.store(false, Ordering::SeqCst);
    assert!(release_service
        .release_unstarted_native_callback(&stale)
        .await
        .unwrap());
    unblock.notify_one();
    let prepared = tokio::time::timeout(std::time::Duration::from_secs(10), follower)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(matches!(
        prepared,
        PreparedPublishedCallbackResume::Resume { .. }
    ));
    assert_unstarted(&fixture).await;
    let receipt = reserve(&fixture).await;
    let execution = execute_until_provider(&fixture, receipt).await;
    finish(&fixture, execution, stale.attempt_id()).await;
}

#[tokio::test]
async fn pending_native_retry_concurrent_admission_attaches_to_one_execution() {
    let fixture = native_fixture(None).await;
    let stale = reserve(&fixture).await;
    assert!(fixture
        .service
        .release_unstarted_native_callback(&stale)
        .await
        .unwrap());
    assert_unstarted(&fixture).await;
    let barrier = Barrier::new(2);
    let contend = || async {
        prepare(&fixture).await;
        barrier.wait().await;
        fixture
            .service
            .reserve_native_callback_reservation_for_actor(fixture.actor.clone(), &fixture.command)
            .await
    };
    let (left, right) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::join!(contend(), contend())
    })
    .await
    .expect("both classified retries must reach the durable CAS");
    let (winner, loser) = match (left, right) {
        (Ok(winner), Err(loser)) | (Err(loser), Ok(winner)) => (winner, loser),
        _ => panic!("exactly one simultaneous reservation must win"),
    };
    assert_eq!(
        loser.downcast_ref::<control_plane::errors::ControlPlaneError>(),
        Some(&control_plane::errors::ControlPlaneError::Conflict(
            "callback_resume_already_admitted"
        ))
    );
    let PublishedCallbackReservation::Acquired(winner) = winner else {
        panic!("CAS winner must own admission");
    };
    assert_eq!(winner.attempt_id(), stale.attempt_id());
    let execution = execute_until_provider(&fixture, winner).await;
    // This is the same reclassification used by the ingress reservation-race
    // branch. The provider barrier proves an execution claim is still live.
    assert!(matches!(
        fixture
            .service
            .prepare_callback_resume_for_actor(fixture.actor.clone(), &fixture.command)
            .await
            .unwrap(),
        PreparedPublishedCallbackResume::Attach { .. }
    ));
    assert!(!fixture
        .service
        .release_unstarted_native_callback(&stale)
        .await
        .unwrap());
    finish(&fixture, execution, stale.attempt_id()).await;
}
