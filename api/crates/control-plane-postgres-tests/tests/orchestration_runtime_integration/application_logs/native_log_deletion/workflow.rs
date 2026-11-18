use super::*;
use control_plane_contracts::ports::CreateWorkflowTriggerConfig;

async fn setup_workflow() -> (
    PgControlPlaneStore,
    RuntimeSeedState,
    domain::CompiledPlanRecord,
    domain::ActorContext,
) {
    let (store, mut seeded, _, actor) = setup().await;
    let application = store
        .create_application(&CreateApplicationInput {
            actor_user_id: seeded.actor_user_id,
            workspace_id: seeded.workspace_id,
            application_type: ApplicationType::Workflow,
            workflow_trigger_type: Some(domain::WorkflowTriggerType::Extension),
            workflow_trigger_config: Some(CreateWorkflowTriggerConfig::Extension {
                subpath: format!("deletion-fixture-{}", Uuid::now_v7()),
                http_method: "POST".into(),
                response_mode: "sync".into(),
            }),
            name: "Workflow deletion fixture".into(),
            description: "Same native log deletion contract as Agent Flow".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();
    let editor = store
        .get_or_create_editor_state(seeded.workspace_id, application.id, seeded.actor_user_id)
        .await
        .unwrap();
    seeded.application_id = application.id;
    seeded.flow_id = editor.flow.id;
    seeded.draft_id = editor.draft.id;
    seeded.draft_updated_at = editor.draft.updated_at;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    (store, seeded, compiled, actor)
}

#[tokio::test]
async fn workflow_log_deletion_runs_hundred_record_batches_to_completion_and_preserves_live_work() {
    let (store, seeded, compiled, actor) = setup_workflow().await;
    let at = datetime!(2026-10-07 12:00 UTC);
    let mut expired = Vec::new();
    for _ in 0..101 {
        expired.push(run(&store, &seeded, &compiled, true, at).await);
    }
    let active = run(&store, &seeded, &compiled, false, at).await;
    let boundary = run(
        &store,
        &seeded,
        &compiled,
        true,
        datetime!(2026-10-08 00:00 UTC),
    )
    .await;
    let exclusive = context(&store, &seeded, expired[0], json!({"expired":"body"})).await;
    let shared = context(&store, &seeded, active, json!({"shared":"body"})).await;
    assert_eq!(
        context(&store, &seeded, expired[100], json!({"shared":"body"})).await,
        shared
    );
    let financial = financial_snapshot(&store).await;
    let service = AgentLogsService::new(store.for_actor(actor.clone()));
    let scope = AgentLogsDeleteScope::TimeRange {
        started_at_from: "2026-10-07T00:00:00Z".into(),
        started_at_to: "2026-10-08T00:00:00Z".into(),
        batch_size: std::num::NonZeroU32::new(100),
        ingested_at_before: None,
    };
    assert_eq!(
        service
            .preview_deletion(&actor, seeded.application_id, scope.clone())
            .await
            .unwrap()
            .total_records,
        101
    );
    let job = service
        .start_deletion(
            &actor,
            seeded.application_id,
            AgentLogsDeleteJobCreate {
                job_id: Uuid::now_v7(),
                scope,
            },
        )
        .await
        .unwrap();
    // A later imported/native write cannot expand this job's fixed workset.
    let late = run(&store, &seeded, &compiled, true, at).await;
    store
        .advance_agent_logs_delete_job(job.job_id)
        .await
        .unwrap();
    let progress = service
        .deletion_job(&actor, seeded.application_id, Some(job.job_id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (progress.deleted_records, progress.total_records),
        (100, 101)
    );
    assert_ne!(progress.status, AgentLogsDeleteJobStatus::Succeeded);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from flow_runs where id=any($1)")
            .bind(&expired)
            .fetch_one(store.pool())
            .await
            .unwrap(),
        1
    );
    store
        .advance_agent_logs_delete_job(job.job_id)
        .await
        .unwrap();
    let completed = service
        .deletion_job(&actor, seeded.application_id, Some(job.job_id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completed.status, AgentLogsDeleteJobStatus::Succeeded);
    assert_eq!(
        (completed.deleted_records, completed.total_records),
        (101, 101)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from flow_runs where id=any($1)")
            .bind(&expired)
            .fetch_one(store.pool())
            .await
            .unwrap(),
        0
    );
    assert!(
        exists(&store, active).await
            && exists(&store, boundary).await
            && exists(&store, late).await
    );
    assert!(!sqlx::query_scalar::<_, bool>(
        "select exists(select 1 from runtime_canonical_contents where id=$1)"
    )
    .bind(exclusive)
    .fetch_one(store.pool())
    .await
    .unwrap());
    assert!(sqlx::query_scalar::<_, bool>(
        "select exists(select 1 from runtime_canonical_contents where id=$1)"
    )
    .bind(shared)
    .fetch_one(store.pool())
    .await
    .unwrap());
    assert_eq!(financial_snapshot(&store).await, financial);
}
