use super::*;
use control_plane_contracts::ports::{
    CommitContainerTransitionInput, ContainerNodeFact, ContainerRoundFact,
};

fn start(flow_run_id: Uuid, frame_id: Uuid, round_index: u64) -> CommitContainerTransitionInput {
    CommitContainerTransitionInput {
        flow_run_id,
        resume_claim_owner: None,
        control_failure: None,
        execution_identity: domain::NodeExecutionIdentity {
            execution_id: Uuid::now_v7(),
            node_id: "same-node".into(),
            parent_execution_id: Some(frame_id),
            round_path: vec![domain::ContainerRoundPath {
                container_execution_id: frame_id,
                container_node_id: "loop".into(),
                round_index,
            }],
        },
        checkpoint: json!({"started_rounds":round_index+1,"shared_context":{"large_value":"retained once"}}),
        node: Some(ContainerNodeFact::Started {
            node_type: "assign".into(),
            node_alias: "Assign".into(),
            input_payload: json!({"round":round_index}),
        }),
        rounds: vec![ContainerRoundFact {
            execution_id: Uuid::now_v7(),
            container_execution_id: frame_id,
            round_index,
            deadline_unix_ms: 2000000000000,
            status: "running".into(),
            error_payload: None,
            output_payload: None,
            variables: None,
        }],
    }
}

fn complete(mut input: CommitContainerTransitionInput) -> CommitContainerTransitionInput {
    input.checkpoint["committed"] = json!(true);
    input.node = Some(ContainerNodeFact::Updated {
        status: NodeRunStatus::Succeeded,
        output_payload: json!({"result":"committed"}),
        error_payload: None,
        metrics_payload: json!({}),
        debug_payload: json!({}),
        terminal: true,
    });
    input.rounds[0].status = "succeeded".into();
    input
}

#[tokio::test]
async fn container_commit_keeps_repeated_node_ids_and_rolls_back_node_round_and_head_together() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let run = seed_flow_run(&store, &seeded, &compiled, OffsetDateTime::now_utc()).await;
    let frame_id = Uuid::now_v7();
    let first = start(run.id, frame_id, 0);
    let first_record = store
        .commit_container_transition(&first)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        first_record.execution_identity.as_ref(),
        Some(&first.execution_identity)
    );
    let second = start(run.id, frame_id, 1);
    let second_record = store
        .commit_container_transition(&second)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(first_record.id, second_record.id);
    assert_eq!(first_record.node_id, second_record.node_id);
    assert!(
        store.commit_container_transition(&second).await.is_err(),
        "duplicate starts must not consume another receipt"
    );
    // Failure is injected after both terminal facts are written, at the head update.
    sqlx::query("create function reject_container_head() returns trigger language plpgsql as $$ begin raise exception 'injected checkpoint failure'; end $$")
        .execute(store.pool()).await.unwrap();
    sqlx::query("create trigger reject_container_head before update on flow_container_checkpoints for each row execute function reject_container_head()")
        .execute(store.pool()).await.unwrap();
    let terminal = complete(second.clone());
    assert!(store.commit_container_transition(&terminal).await.is_err());
    let unchanged = store
        .get_container_node_run(run.id, second.execution_identity.execution_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unchanged.status, NodeRunStatus::Running);
    let status: String = sqlx::query_scalar(
        "select status from flow_container_rounds where flow_run_id=$1 and execution_id=$2",
    )
    .bind(run.id)
    .bind(second.rounds[0].execution_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(status, "running");
    assert_eq!(
        store.load_container_checkpoint(run.id).await.unwrap(),
        Some(second.checkpoint.clone())
    );
    sqlx::query("drop trigger reject_container_head on flow_container_checkpoints")
        .execute(store.pool())
        .await
        .unwrap();
    let mut expired = terminal.clone();
    expired.rounds[0].deadline_unix_ms = 1;
    let error = store
        .commit_container_transition(&expired)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("container_deadline_expired"));
    assert_eq!(
        store.load_container_checkpoint(run.id).await.unwrap(),
        Some(second.checkpoint.clone())
    );
    let mut extended = terminal.clone();
    extended.rounds[0].deadline_unix_ms += 1;
    assert!(
        store.commit_container_transition(&extended).await.is_err(),
        "recovery cannot renew a round deadline"
    );
    let finished = store
        .commit_container_transition(&terminal)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(finished.status, NodeRunStatus::Succeeded);
    assert!(
        store.commit_container_transition(&terminal).await.is_err(),
        "one logical execution has one admitted terminal"
    );
    let counts:(i64,i64,i64)=sqlx::query_as("select (select count(*) from node_runs where flow_run_id=$1),(select count(*) from flow_container_rounds where flow_run_id=$1),(select count(*) from flow_container_checkpoints where flow_run_id=$1)")
        .bind(run.id).fetch_one(store.pool()).await.unwrap();
    assert_eq!(counts, (2, 2, 1));
    // Clearing a debug body cannot erase canonical scope identity.
    sqlx::query("update node_run_records set debug_payload='{}' where id=$1")
        .bind(finished.id)
        .execute(store.pool())
        .await
        .unwrap();
    let reread = store
        .get_container_node_run(run.id, second.execution_identity.execution_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reread.execution_identity, Some(second.execution_identity));
}

#[tokio::test]
async fn container_commit_rejects_expired_generation_and_old_initial_owner() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let run = seed_flow_run(&store, &seeded, &compiled, OffsetDateTime::now_utc()).await;
    let initial = start(run.id, Uuid::now_v7(), 0);
    let node = store
        .commit_container_transition(&initial)
        .await
        .unwrap()
        .unwrap();
    sqlx::query("update flow_runs set status='waiting_human' where id=$1")
        .bind(run.id)
        .execute(store.pool())
        .await
        .unwrap();
    let checkpoint = store
        .create_checkpoint(&CreateCheckpointInput {
            flow_run_id: run.id,
            node_run_id: Some(node.id),
            status: "waiting_human".into(),
            reason: "container claim fixture".into(),
            locator_payload: json!({"container_state":true}),
            variable_snapshot: json!({}),
            external_ref_payload: None,
        })
        .await
        .unwrap();
    let admission = AcquireResumeClaimInput {
        scope_id: seeded.workspace_id,
        application_id: seeded.application_id,
        flow_run_id: run.id,
        checkpoint_id: checkpoint.id,
        callback_task_id: None,
        kind: ResumeClaimKind::Human,
        request_payload: json!({"answer":"yes"}),
    };
    let first = store.acquire_resume_claim(&admission).await.unwrap();
    let mut terminal = complete(initial.clone());
    assert!(
        store.commit_container_transition(&terminal).await.is_err(),
        "initial dispatch owner lost its fence"
    );
    terminal.resume_claim_owner = Some(RenewResumeClaimInput {
        claim_id: first.claim.id,
        claim_token: first.claim.claim_token,
        expected_generation: first.claim.generation,
    });
    sqlx::query(
        "update flow_run_resume_claims set lease_expires_at=now()-interval '1 second' where id=$1",
    )
    .bind(first.claim.id)
    .execute(store.pool())
    .await
    .unwrap();
    assert!(
        store.commit_container_transition(&terminal).await.is_err(),
        "expired owner cannot complete before a successor even claims"
    );
    let successor = store.acquire_resume_claim(&admission).await.unwrap();
    assert!(successor.claim.generation > first.claim.generation);
    assert!(
        store.commit_container_transition(&terminal).await.is_err(),
        "stale generation cannot overwrite the new owner"
    );
    assert_eq!(
        store.load_container_checkpoint(run.id).await.unwrap(),
        Some(initial.checkpoint)
    );
    terminal.resume_claim_owner = Some(RenewResumeClaimInput {
        claim_id: successor.claim.id,
        claim_token: successor.claim.claim_token,
        expected_generation: successor.claim.generation,
    });
    let result = store
        .commit_container_transition(&terminal)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.id, node.id);
    assert_eq!(result.status, NodeRunStatus::Succeeded);
}

#[tokio::test]
async fn container_round_stop_closes_inflight_but_preserves_committed_siblings() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let run = seed_flow_run(&store, &seeded, &compiled, OffsetDateTime::now_utc()).await;
    let first = start(run.id, Uuid::now_v7(), 0);
    store.commit_container_transition(&first).await.unwrap();
    let mut terminal = complete(first.clone());
    terminal.rounds.clear();
    store.commit_container_transition(&terminal).await.unwrap();
    let mut sibling = first.clone();
    sibling.execution_identity.execution_id = Uuid::now_v7();
    store.commit_container_transition(&sibling).await.unwrap();
    let mut stop = first.clone();
    stop.node = None;
    stop.rounds[0].status = "cancelled".into();
    stop.rounds[0].error_payload = Some(json!({"code":"cancelled"}));
    store.commit_container_transition(&stop).await.unwrap();
    let first_record = store
        .get_container_node_run(run.id, first.execution_identity.execution_id)
        .await
        .unwrap()
        .unwrap();
    let sibling_record = store
        .get_container_node_run(run.id, sibling.execution_identity.execution_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first_record.status, NodeRunStatus::Succeeded);
    assert_eq!(sibling_record.status, NodeRunStatus::Cancelled);
    assert!(
        store
            .commit_container_transition(&complete(sibling))
            .await
            .is_err(),
        "late result cannot reopen a stopped round"
    );
}

#[tokio::test]
async fn native_container_successor_maps_receipts_without_changing_logical_parent_or_round() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let source = seed_flow_run(&store, &seeded, &compiled, OffsetDateTime::now_utc()).await;
    let frame_id = Uuid::now_v7();
    let mut child = start(source.id, frame_id, 0);
    let mut frame = child.clone();
    frame.execution_identity = domain::NodeExecutionIdentity {
        execution_id: frame_id,
        node_id: "loop".into(),
        parent_execution_id: None,
        round_path: vec![],
    };
    frame.rounds.clear();
    store.commit_container_transition(&frame).await.unwrap();
    child.checkpoint = json!({"frames":{frame_id.to_string():{
        "identity":frame.execution_identity,
        "rounds":{"0":{"nodes":{"same-node":{"identity":child.execution_identity}}}}
    }}});
    let source_node = store
        .commit_container_transition(&child)
        .await
        .unwrap()
        .unwrap();
    child.node = Some(ContainerNodeFact::Updated {
        status: NodeRunStatus::Failed,
        output_payload: json!({}),
        error_payload: Some(json!({"code":"retryable_provider_failure"})),
        metrics_payload: json!({}),
        debug_payload: json!({}),
        terminal: true,
    });
    child.rounds[0].status = "failed".into();
    store.commit_container_transition(&child).await.unwrap();
    frame.checkpoint = child.checkpoint.clone();
    frame.node = child.node.clone();
    store.commit_container_transition(&frame).await.unwrap();
    sqlx::query("update flow_runs set status='failed' where id=$1")
        .bind(source.id)
        .execute(store.pool())
        .await
        .unwrap();
    let target = seed_flow_run(&store, &seeded, &compiled, OffsetDateTime::now_utc()).await;
    sqlx::query("update flow_runs set input_payload=$2 where id=$1")
        .bind(target.id)
        .bind(json!({"sys":{"native_inference_recovery":{"failed_flow_run_id":source.id}}}))
        .execute(store.pool())
        .await
        .unwrap();
    store
        .fork_container_checkpoint(source.id, target.id, child.execution_identity.execution_id)
        .await
        .unwrap();
    let target_node = store
        .get_container_node_run(target.id, child.execution_identity.execution_id)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(target_node.id, source_node.id);
    assert_eq!(
        target_node.execution_identity,
        source_node.execution_identity
    );
    assert_eq!(target_node.status, NodeRunStatus::Running);
    assert_eq!(
        store
            .get_container_node_run(target.id, frame_id)
            .await
            .unwrap()
            .unwrap()
            .status,
        NodeRunStatus::Running
    );
    assert_eq!(
        store
            .get_container_node_run(source.id, child.execution_identity.execution_id)
            .await
            .unwrap()
            .unwrap()
            .status,
        NodeRunStatus::Failed
    );
    assert!(!store
        .container_native_recovery_started(target.id)
        .await
        .unwrap());
    child.flow_run_id = target.id;
    child.rounds[0].status = "running".into();
    child.node = Some(ContainerNodeFact::Updated {
        status: NodeRunStatus::Running,
        output_payload: json!({}),
        error_payload: None,
        metrics_payload: json!({}),
        debug_payload: json!({}),
        terminal: false,
    });
    child.checkpoint["native_recovery_progress"] = json!("began");
    store.commit_container_transition(&child).await.unwrap();
    assert!(store
        .container_native_recovery_started(target.id)
        .await
        .unwrap());
    store
        .fork_container_checkpoint(source.id, target.id, child.execution_identity.execution_id)
        .await
        .unwrap();
    assert_eq!(
        store.load_container_checkpoint(target.id).await.unwrap(),
        Some(child.checkpoint)
    );
}
