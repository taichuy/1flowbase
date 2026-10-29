use super::*;
use control_plane_contracts::ports::{
    CommitContainerTransitionInput, ContainerCommitAdmission, ContainerCommitDeadline,
    ContainerControlFailureFact, ContainerObservationInput,
};

async fn db_now(store: &PgControlPlaneStore) -> i64 {
    sqlx::query_scalar("select floor(extract(epoch from clock_timestamp())*1000)::bigint")
        .fetch_one(store.pool())
        .await
        .unwrap()
}

fn admit(input: &mut CommitContainerTransitionInput, deadline: i64) {
    input.rounds[0].deadline_unix_ms = deadline;
    input.admission = ContainerCommitAdmission::AdmitWork {
        effective_deadline: Some(ContainerCommitDeadline {
            deadline_unix_ms: deadline,
            owner_execution_id: input.rounds[0].container_execution_id,
        }),
    };
}

// Synchronize on an observed PostgreSQL blocker, not a sleep that guesses when
// the contender reached storage. Only the expiry clock is then allowed to pass.
async fn locked_commit(
    store: Arc<PgControlPlaneStore>,
    input: CommitContainerTransitionInput,
    expire_at: Option<i64>,
) -> anyhow::Result<Option<domain::NodeRunRecord>> {
    let mut blocker = store.pool().begin().await.unwrap();
    sqlx::query("select id from flow_runs where id=$1 for update")
        .bind(input.flow_run_id)
        .execute(&mut *blocker)
        .await
        .unwrap();
    let blocker_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let contender_store = store.clone();
    let contender_barrier = barrier.clone();
    let contender = tokio::spawn(async move {
        contender_barrier.wait().await;
        contender_store.commit_container_transition(&input).await
    });
    barrier.wait().await;
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let blocked: bool = sqlx::query_scalar(
                "select exists(select 1 from pg_stat_activity where $1=any(pg_blocking_pids(pid)))",
            )
            .bind(blocker_pid)
            .fetch_one(store.pool())
            .await
            .unwrap();
            if blocked {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("contender must reach the real database lock");
    if let Some(deadline) = expire_at {
        assert!(db_now(&store).await < deadline, "must queue BEFORE expiry");
        sqlx::query("select pg_sleep(greatest(0.0, ($1::bigint::double precision - extract(epoch from clock_timestamp())*1000)/1000.0)+0.01)")
            .bind(deadline).execute(&mut *blocker).await.unwrap();
        assert!(db_now(&store).await >= deadline);
    }
    blocker.commit().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), contender)
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn container_round_and_node_admission_uses_db_clock_after_lock_wait() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = Arc::new(PgControlPlaneStore::new(pool));
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    for action in [
        "round_started",
        "round_succeeded",
        "node_started",
        "node_succeeded",
    ] {
        for expires in [false, true] {
            let run = seed_flow_run(&store, &seeded, &compiled, OffsetDateTime::now_utc()).await;
            let mut input = super::container_commits::start(run.id, Uuid::now_v7(), 0);
            if action.starts_with("round_") {
                input.node = None;
            }
            if action.ends_with("succeeded") {
                // The original round and receipt exist before the terminal contender.
                store.commit_container_transition(&input).await.unwrap();
                input = super::container_commits::complete(input);
                if action.starts_with("round_") {
                    input.node = None;
                }
                input.rounds[0].output_payload = Some(json!({"value":"admitted"}));
                input.rounds[0].variables = Some(json!({"total":1}));
            }
            let before = store.load_container_checkpoint(run.id).await.unwrap();
            let deadline = db_now(&store).await + if expires { 2000 } else { 60000 };
            admit(&mut input, deadline);
            input.checkpoint["proposal"] = json!(action);
            let result =
                locked_commit(store.clone(), input.clone(), expires.then_some(deadline)).await;
            if expires {
                assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains("container_deadline_expired"),
                    "{action}"
                );
                assert_eq!(
                    store.load_container_checkpoint(run.id).await.unwrap(),
                    before,
                    "{action}"
                );
                let status: Option<String> = sqlx::query_scalar("select status from flow_container_rounds where flow_run_id=$1 and execution_id=$2")
                    .bind(run.id).bind(input.rounds[0].execution_id).fetch_optional(store.pool()).await.unwrap();
                assert_eq!(
                    status.as_deref(),
                    action.ends_with("succeeded").then_some("running")
                );
                if action.ends_with("succeeded") {
                    let unchanged: (Option<Value>, Option<Value>, i64) = sqlx::query_as(
                        "select output_payload,variables,deadline_unix_ms from flow_container_rounds where flow_run_id=$1 and execution_id=$2",
                    ).bind(run.id).bind(input.rounds[0].execution_id).fetch_one(store.pool()).await.unwrap();
                    assert_eq!(
                        unchanged,
                        (None, None, 2000000000000),
                        "result, variables and deadline must roll back with head"
                    );
                    // The runtime converts the rejected proposal to a failure;
                    // this receipt must still commit after the same deadline.
                    let mut failed = input.clone();
                    failed.node = None;
                    failed.admission = ContainerCommitAdmission::ClosureOrObservation;
                    failed.rounds[0].status = "failed".into();
                    failed.rounds[0].output_payload = None;
                    failed.rounds[0].variables = None;
                    failed.rounds[0].error_payload = Some(json!({"code":"deadline_expired"}));
                    store.commit_container_transition(&failed).await.unwrap();
                }
                let node = store
                    .get_container_node_run(run.id, input.execution_identity.execution_id)
                    .await
                    .unwrap();
                if action == "node_succeeded" {
                    assert_eq!(node.unwrap().status, NodeRunStatus::Failed);
                } else {
                    assert!(node.is_none());
                }
            } else {
                result.unwrap();
                assert_eq!(
                    store.load_container_checkpoint(run.id).await.unwrap(),
                    Some(input.checkpoint)
                );
            }
        }
    }
}

#[tokio::test]
async fn container_expired_closure_and_historical_facts_do_not_block_new_round() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let run = seed_flow_run(&store, &seeded, &compiled, OffsetDateTime::now_utc()).await;
    let frame = Uuid::now_v7();
    let initial = super::container_commits::start(run.id, frame, 0);
    store.commit_container_transition(&initial).await.unwrap();
    for status in ["failed", "cancelled"] {
        let mut closure = initial.clone();
        closure.node = None;
        if status == "cancelled" {
            // Failed admission attempts may close without ever dispatching a node.
            closure.rounds[0].execution_id = Uuid::now_v7();
            closure.rounds[0].round_index = 1;
        }
        closure.rounds[0].deadline_unix_ms = 1;
        closure.rounds[0].status = status.into();
        closure.rounds[0].error_payload = Some(json!({"code":"container_round_admission_timeout"}));
        closure.admission = ContainerCommitAdmission::ClosureOrObservation;
        closure.control_failure = Some(ContainerControlFailureFact {
            after_round_index: closure.rounds[0].round_index,
            error_payload: json!({"code":"expired_control"}),
        });
        closure.observation = Some(ContainerObservationInput {
            observation_key: status.into(),
            trace_payload: json!({"status":status}),
        });
        store.commit_container_transition(&closure).await.unwrap();
        // Replaying identical terminal facts remains legal, without reopening a
        // node or relaxing its separate duplicate-start/terminal constraints.
        store.commit_container_transition(&closure).await.unwrap();
        let mut next =
            super::container_commits::start(run.id, frame, if status == "failed" { 2 } else { 3 });
        next.node = None;
        next.rounds.extend(closure.rounds.clone());
        store.commit_container_transition(&next).await.unwrap();
    }
    let mut historical = super::container_commits::start(run.id, frame, 4);
    historical.node = None;
    let historical_deadline = db_now(&store).await + 2000;
    admit(&mut historical, historical_deadline);
    store
        .commit_container_transition(&historical)
        .await
        .unwrap();
    historical.rounds[0].status = "succeeded".into();
    historical.rounds[0].output_payload = Some(json!("already committed"));
    store
        .commit_container_transition(&historical)
        .await
        .unwrap();
    sqlx::query("select pg_sleep(greatest(0.0, ($1::bigint::double precision - extract(epoch from clock_timestamp())*1000)/1000.0)+0.01)")
        .bind(historical_deadline).execute(store.pool()).await.unwrap();
    let mut next = super::container_commits::start(run.id, frame, 5);
    next.rounds.extend(historical.rounds);
    store.commit_container_transition(&next).await.unwrap();
    let node = store
        .get_container_node_run(run.id, initial.execution_identity.execution_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(node.status, NodeRunStatus::Failed);
    assert_eq!(
        store
            .list_pending_container_observations(run.id, None, 10)
            .await
            .unwrap()
            .len(),
        2
    );
}
