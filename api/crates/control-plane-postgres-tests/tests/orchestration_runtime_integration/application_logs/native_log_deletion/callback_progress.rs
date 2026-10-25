use super::*;
use std::time::Duration as StdDuration;

// Dedicated single-connection pools give each real repository operation a stable
// PID without replacing its transaction or any of its production SQL.
async fn operation_store(store: &PgControlPlaneStore) -> (PgControlPlaneStore, i32) {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .after_connect(|connection, _| {
            Box::pin(async move {
                sqlx::query("set statement_timeout='15s'")
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect_with((*store.pool().connect_options()).clone())
        .await
        .unwrap();
    let pid = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&pool)
        .await
        .unwrap();
    (PgControlPlaneStore::new(pool), pid)
}

async fn backend_states(store: &PgControlPlaneStore, waiter: i32, owners: &[i32]) -> Value {
    let mut pids = owners.to_vec();
    pids.push(waiter);
    sqlx::query_scalar(
        "select coalesce(jsonb_agg(jsonb_build_object(
            'pid',pid,'state',state,'wait_event_type',wait_event_type,
            'wait_event',wait_event,'blocking_pids',pg_blocking_pids(pid),
            'query',query) order by pid),'[]'::jsonb)
         from pg_stat_activity where pid=any($1)
            or pid=any(pg_blocking_pids($2))",
    )
    .bind(&pids)
    .bind(waiter)
    .fetch_one(store.pool())
    .await
    .unwrap()
}

async fn wait_for_blocker<T: std::fmt::Debug>(
    store: &PgControlPlaneStore,
    stage: &str,
    waiter: i32,
    owners: &[i32],
    worker: &mut tokio::task::JoinHandle<T>,
) -> String {
    let observe = async {
        loop {
            let state: Option<(String, Vec<i32>)> = sqlx::query_as(
                "select query,pg_blocking_pids(pid) from pg_stat_activity where pid=$1
                 and state='active' and wait_event_type='Lock'",
            )
            .bind(waiter)
            .fetch_optional(store.pool())
            .await
            .unwrap();
            if let Some((query, blockers)) = state {
                if owners.iter().any(|owner| blockers.contains(owner)) {
                    return query;
                }
            }
            tokio::task::yield_now().await;
        }
    };
    tokio::select! {
        result = &mut *worker => {
            let state = backend_states(store, waiter, owners).await;
            panic!("{stage}: worker exited before its barrier; waiter={waiter}, expected_owners={owners:?}, result={result:?}, pg_state={state}");
        }
        result = tokio::time::timeout(StdDuration::from_secs(10), observe) => {
            let state = backend_states(store, waiter, owners).await;
            match result {
                Ok(query) => {
                    eprintln!("{stage}: waiter={waiter}, expected_owners={owners:?}, pg_state={state}");
                    query
                }
                Err(error) => panic!("{stage}: barrier timed out; waiter={waiter}, expected_owners={owners:?}, pg_state={state}, error={error}"),
            }
        }
    }
}

async fn claim_lock_available(store: &PgControlPlaneStore, claim: Uuid) -> bool {
    let mut probe = store.pool().begin().await.unwrap();
    let result = sqlx::query_scalar::<_, Uuid>(
        "select id from flow_run_resume_claims where id=$1 for update nowait",
    )
    .bind(claim)
    .fetch_one(&mut *probe)
    .await;
    probe.rollback().await.unwrap();
    match result {
        Ok(id) => {
            assert_eq!(id, claim);
            true
        }
        Err(error) => {
            assert_eq!(
                error
                    .as_database_error()
                    .and_then(|error| error.code())
                    .as_deref(),
                Some("55P03"),
                "only actual lock contention counts as an unavailable claim"
            );
            false
        }
    }
}

#[tokio::test]
async fn native_log_deletion_serializes_terminal_callback_claim_replay_before_child_locks() {
    let (store, seeded, compiled, actor) = setup().await;
    let at = datetime!(2026-10-07 12:00 UTC);
    let run_id = run(&store, &seeded, &compiled, false, at).await;
    let conversation = Uuid::now_v7();
    sqlx::query(
        "insert into application_conversations(id,scope_id,application_id,external_conversation_id)
         values($1,$2,$3,'callback-claim-deletion-order')",
    )
    .bind(conversation)
    .bind(seeded.workspace_id)
    .bind(seeded.application_id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        "update flow_runs set log_context=jsonb_build_object(
            'log_conversation_id',$2::text,'log_task_run_id',id::text) where id=$1",
    )
    .bind(run_id)
    .bind(conversation)
    .execute(store.pool())
    .await
    .unwrap();
    let record = store
        .get_flow_run(seeded.application_id, run_id)
        .await
        .unwrap()
        .unwrap();
    let node = seed_node_run(&store, &record, at).await;
    let checkpoint = store
        .create_checkpoint(&CreateCheckpointInput {
            flow_run_id: run_id,
            node_run_id: Some(node.id),
            status: "waiting_callback".into(),
            reason: "native deletion callback lock ordering".into(),
            locator_payload: json!({}),
            variable_snapshot: json!({}),
            external_ref_payload: None,
        })
        .await
        .unwrap();
    let callback = store.create_callback_task(&CreateCallbackTaskInput {
        flow_run_id: run_id,
        node_run_id: node.id,
        callback_kind: "llm_tool_calls".into(),
        request_payload: json!({"tool_calls":[{"id":"deletion-order","name":"Read","arguments":{}}]}),
        external_ref_payload: None,
    }).await.unwrap();
    store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id: run_id,
            status: FlowRunStatus::WaitingCallback,
            output_payload: json!({}),
            error_payload: None,
            finished_at: None,
        })
        .await
        .unwrap();
    let replay_input = AcquireResumeClaimInput {
        scope_id: seeded.workspace_id,
        application_id: seeded.application_id,
        flow_run_id: run_id,
        checkpoint_id: checkpoint.id,
        callback_task_id: Some(callback.id),
        kind: ResumeClaimKind::Callback,
        request_payload: json!({"tool_results":[{"id":"deletion-order","output":"done"}]}),
    };
    let acquired = store.acquire_resume_claim(&replay_input).await.unwrap();
    assert_eq!(acquired.disposition, ResumeClaimDisposition::Acquired);
    store
        .finish_resume_claim(&FinishResumeClaimInput {
            claim_id: acquired.claim.id,
            claim_token: acquired.claim.claim_token,
            expected_generation: acquired.claim.generation,
            status: ResumeClaimStatus::Succeeded,
            error_payload: None,
            completed_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    store
        .complete_callback_task(&CompleteCallbackTaskInput {
            callback_task_id: callback.id,
            response_payload: replay_input.request_payload.clone(),
            completed_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    finish(&store, run_id).await;
    assert_eq!(
        store
            .acquire_resume_claim(&replay_input)
            .await
            .unwrap()
            .disposition,
        ResumeClaimDisposition::Completed,
        "the concurrent input is a real terminal replay"
    );
    let projected_conversation: Option<Uuid> =
        sqlx::query_scalar("select log_conversation_id from application_run_log_tasks where id=$1")
            .bind(run_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(projected_conversation, Some(conversation));
    let service = AgentLogsService::new(store.for_actor(actor.clone()));
    let job = service
        .start_deletion(&actor, seeded.application_id, input())
        .await
        .unwrap();
    assert_eq!(job.total_records, 1);

    // Stop real deletion after it owns conversation and flow, before task and
    // cascade claim locks. No scheduler delay is used to establish this order.
    let mut task_barrier = store.pool().begin().await.unwrap();
    let barrier_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *task_barrier)
        .await
        .unwrap();
    sqlx::query("select id from application_run_log_tasks where id=$1 for update")
        .bind(run_id)
        .execute(&mut *task_barrier)
        .await
        .unwrap();
    let (delete_store, delete_pid) = operation_store(&store).await;
    let mut deletion =
        tokio::spawn(async move { delete_store.advance_agent_logs_delete_job(job.job_id).await });
    let deletion_query = wait_for_blocker(
        &store,
        "deletion waits for task after locking native parents",
        delete_pid,
        &[barrier_pid],
        &mut deletion,
    )
    .await;
    assert!(deletion_query.contains("application_run_log_tasks"));

    // Controlled negative: the former terminal replay order owns the claim
    // before its progress trigger can finish updating the task. The upsert can
    // wait on either deletion's parent FK locks or our task barrier; requiring
    // only the latter assumes a PostgreSQL constraint-check order. Both expose
    // the same inversion: a child is held while a parent/derived owner is awaited.
    // The green path below still requires the exact early conversation guard.
    let mut negative_tx = store.pool().begin().await.unwrap();
    let negative_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *negative_tx)
        .await
        .unwrap();
    sqlx::query("set local statement_timeout='15s'")
        .execute(&mut *negative_tx)
        .await
        .unwrap();
    sqlx::query("select id from flow_run_resume_claims where id=$1 for update")
        .bind(acquired.claim.id)
        .execute(&mut *negative_tx)
        .await
        .unwrap();
    let claim_id = acquired.claim.id;
    let mut negative = tokio::spawn(async move {
        let result = sqlx::query(
            "update flow_run_resume_claims set status='succeeded',
             completed_at=coalesce(completed_at,now()),updated_at=now() where id=$1",
        )
        .bind(claim_id)
        .execute(&mut *negative_tx)
        .await;
        negative_tx.rollback().await.unwrap();
        result
    });
    let negative_query = wait_for_blocker(
        &store,
        "negative child-first progress waits for deletion parent or task",
        negative_pid,
        &[delete_pid, barrier_pid],
        &mut negative,
    )
    .await;
    assert!(negative_query.contains("update flow_run_resume_claims"));
    assert!(
        !claim_lock_available(&store, claim_id).await,
        "the controlled negative must expose the child-first lock inversion"
    );
    let cancelled: bool = sqlx::query_scalar("select pg_cancel_backend($1)")
        .bind(negative_pid)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert!(cancelled);
    let negative_error = tokio::time::timeout(StdDuration::from_secs(10), negative)
        .await
        .unwrap()
        .unwrap()
        .expect_err("cancel the blocked negative and roll it back");
    assert_eq!(
        negative_error
            .as_database_error()
            .and_then(|error| error.code())
            .as_deref(),
        Some("57014")
    );

    let (replay_store, replay_pid) = operation_store(&store).await;
    let mut replay =
        tokio::spawn(async move { replay_store.acquire_resume_claim(&replay_input).await });
    let replay_query = wait_for_blocker(
        &store,
        "real replay waits for deletion before locking claim",
        replay_pid,
        &[delete_pid],
        &mut replay,
    )
    .await;
    assert!(
        replay_query.contains("application_run_lock_native_projection"),
        "real replay must wait on the conversation owner before touching claims"
    );
    assert!(
        claim_lock_available(&store, claim_id).await,
        "waiting replay must leave the cascade child unlocked"
    );
    task_barrier.commit().await.unwrap();
    tokio::time::timeout(StdDuration::from_secs(10), deletion)
        .await
        .expect("real deletion must complete without deadlock")
        .unwrap()
        .unwrap();
    let replay_error = tokio::time::timeout(StdDuration::from_secs(10), replay)
        .await
        .expect("replay must finish after deletion")
        .unwrap()
        .expect_err("a deleted terminal run cannot acquire a new claim");
    assert!(
        replay_error
            .to_string()
            .contains("resume_claim_not_waiting"),
        "{replay_error:#}"
    );
    assert!(!exists(&store, run_id).await);
    let remaining: (i64, i64, i64) = sqlx::query_as(
        "select (select count(*) from flow_run_resume_claims where id=$1),
                (select count(*) from flow_run_callback_tasks where id=$2),
                (select count(*) from application_run_log_tasks where id=$3)",
    )
    .bind(claim_id)
    .bind(callback.id)
    .bind(run_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        remaining,
        (0, 0, 0),
        "replay cannot resurrect deleted callback or task facts"
    );
    let progress = service
        .deletion_job(&actor, seeded.application_id, Some(job.job_id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(progress.deleted_records, 1);
}
