use super::deletion::{delete_service, ingest_turn, range};
use super::*;

fn input(scope: AgentLogsDeleteScope) -> AgentLogsDeleteJobCreate {
    let mut scope = scope;
    match &mut scope {
        AgentLogsDeleteScope::AllTime { batch_size, .. }
        | AgentLogsDeleteScope::TimeRange { batch_size, .. } => {
            *batch_size = std::num::NonZeroU32::new(1)
        }
    }
    AgentLogsDeleteJobCreate {
        job_id: Uuid::now_v7(),
        scope,
    }
}
#[tokio::test]
async fn agent_logs_delete_job_fixed_workset_preview_idempotency_restart_and_legacy_conflict() {
    let (store, scope, app) = setup().await;
    let before = ingest_turn(&store, scope, app, "before", "2026-10-06T23:59:59Z").await;
    let first = ingest_turn(&store, scope, app, "first", "2026-10-07T00:00:00Z").await;
    let second = ingest_turn(&store, scope, app, "second", "2026-10-07T23:59:59Z").await;
    let end = ingest_turn(&store, scope, app, "end", "2026-10-08T00:00:00Z").await;
    let (service, actor) = delete_service(&store, scope, app).await;
    let request = input(range("2026-10-07T00:00:00Z", "2026-10-08T00:00:00Z"));
    assert_eq!(
        service
            .preview_deletion(&actor, app, request.scope.clone())
            .await
            .unwrap()
            .total_records,
        2
    );
    assert!(store
        .application_log_record(app, first)
        .await
        .unwrap()
        .is_some());
    let job = service
        .start_deletion(&actor, app, request.clone())
        .await
        .unwrap();
    assert_eq!(job.total_records, 2);
    assert_eq!(job.status, AgentLogsDeleteJobStatus::Queued);
    assert_eq!(
        service
            .start_deletion(&actor, app, request.clone())
            .await
            .unwrap()
            .job_id,
        job.job_id
    );
    assert!(service
        .start_deletion(&actor, app, input(AgentLogsDeleteScope::all_time()))
        .await
        .is_err());
    let mut conflicting = request.clone();
    conflicting.scope = AgentLogsDeleteScope::all_time();
    assert!(store
        .create_agent_logs_delete_job(app, scope, &conflicting)
        .await
        .is_err());
    assert!(service
        .delete(&actor, app, AgentLogsDeleteScope::all_time())
        .await
        .is_err());
    assert!(store
        .get_agent_logs_delete_job(app, Uuid::now_v7(), Some(job.job_id))
        .await
        .unwrap()
        .is_none());
    let late = ingest_turn(&store, scope, app, "late", "2026-10-07T06:00:00Z").await;
    // A projection change after the snapshot cannot bring an excluded existing record in.
    sqlx::query(
        "update application_run_log_tasks set started_at='2026-10-07T06:00:00Z' where id=$1",
    )
    .bind(before)
    .execute(store.pool())
    .await
    .unwrap();
    store
        .advance_agent_logs_delete_job(job.job_id)
        .await
        .unwrap();
    let partial = service
        .deletion_job(&actor, app, Some(job.job_id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!((partial.deleted_records, partial.total_records), (1, 2));
    assert_eq!(partial.status, AgentLogsDeleteJobStatus::Running);
    // Delayed failure from another attempt cannot overwrite committed progress.
    store
        .fail_agent_logs_delete_job(job.job_id, 0)
        .await
        .unwrap();
    let restarted = PgControlPlaneStore::new(store.pool().clone());
    assert!(AgentLogsService::new(restarted)
        .advance_next_deletion()
        .await
        .unwrap());
    let final_job = service
        .deletion_job(&actor, app, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(final_job.status, AgentLogsDeleteJobStatus::Succeeded);
    assert_eq!((final_job.deleted_records, final_job.total_records), (2, 2));
    for id in [first, second] {
        assert!(store
            .application_log_record(app, id)
            .await
            .unwrap()
            .is_none());
    }
    for id in [before, end, late] {
        assert!(store
            .application_log_record(app, id)
            .await
            .unwrap()
            .is_some());
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from application_log_deletion_records where job_id=$1"
        )
        .bind(job.job_id)
        .fetch_one(store.pool())
        .await
        .unwrap(),
        0
    );
}

#[tokio::test]
async fn agent_logs_delete_job_stop_preserves_committed_progress_and_permission_scope() {
    let (store, scope, app) = setup().await;
    for name in ["a", "b", "c"] {
        ingest_turn(&store, scope, app, name, "2026-10-07T00:00:00Z").await;
    }
    let (service, actor) = delete_service(&store, scope, app).await;
    let ungranted = domain::ActorContext::scoped_in_scope(
        actor.user_id,
        actor.tenant_id,
        scope,
        "member",
        Vec::<String>::new(),
    );
    assert!(service
        .preview_deletion(&ungranted, app, AgentLogsDeleteScope::all_time())
        .await
        .is_err());
    assert!(service
        .start_deletion(&ungranted, app, input(AgentLogsDeleteScope::all_time()))
        .await
        .is_err());
    let request = input(AgentLogsDeleteScope::all_time());
    let job = service
        .start_deletion(&actor, app, request.clone())
        .await
        .unwrap();
    assert!(service
        .deletion_job(&ungranted, app, Some(job.job_id))
        .await
        .is_err());
    assert!(service
        .stop_deletion(&ungranted, app, job.job_id)
        .await
        .is_err());
    store
        .advance_agent_logs_delete_job(job.job_id)
        .await
        .unwrap();
    assert!(
        service
            .stop_deletion(&actor, app, job.job_id)
            .await
            .unwrap()
            .stop_requested
    );
    AgentLogsService::new(store.clone())
        .advance_next_deletion()
        .await
        .unwrap();
    let stopped = service
        .deletion_job(&actor, app, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stopped.status, AgentLogsDeleteJobStatus::Stopped);
    assert_eq!((stopped.deleted_records, stopped.total_records), (1, 3));
    assert_eq!(
        service
            .start_deletion(&actor, app, request)
            .await
            .unwrap()
            .status,
        AgentLogsDeleteJobStatus::Stopped
    );
    let remaining = service
        .preview_deletion(&actor, app, AgentLogsDeleteScope::all_time())
        .await
        .unwrap()
        .total_records;
    assert_eq!(remaining, 2);
    let queued = service
        .start_deletion(&actor, app, input(AgentLogsDeleteScope::all_time()))
        .await
        .unwrap();
    service
        .stop_deletion(&actor, app, queued.job_id)
        .await
        .unwrap();
    store
        .advance_agent_logs_delete_job(queued.job_id)
        .await
        .unwrap();
    assert_eq!(
        service
            .deletion_job(&actor, app, None)
            .await
            .unwrap()
            .unwrap()
            .deleted_records,
        0
    );
}

#[tokio::test]
async fn agent_logs_delete_job_deferred_failure_rolls_back_progress_and_cascade() {
    let (store, scope, app) = setup().await;
    let id = ingest_turn(&store, scope, app, "kept", "2026-10-07T00:00:00Z").await;
    let counts = owned_log_counts(&store, app, &[id], &[]).await;
    let (service, actor) = delete_service(&store, scope, app).await;
    let job = service
        .start_deletion(&actor, app, input(AgentLogsDeleteScope::all_time()))
        .await
        .unwrap();
    sqlx::raw_sql("create function reject_job_section_delete() returns trigger language plpgsql as $$ begin raise exception 'fixture batch rollback'; end $$; create constraint trigger fixture_reject_job_delete after delete on client_trajectory_sections deferrable initially deferred for each row execute function reject_job_section_delete();").execute(store.pool()).await.unwrap();
    assert!(service.advance_next_deletion().await.is_err());
    let failed = service
        .deletion_job(&actor, app, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed.status, AgentLogsDeleteJobStatus::Failed);
    assert_eq!(failed.deleted_records, 0);
    assert_eq!(
        failed.error_code.as_deref(),
        Some("agent_logs_delete_batch_failed")
    );
    assert_eq!(owned_log_counts(&store, app, &[id], &[]).await, counts);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from application_log_deletion_records where job_id=$1"
        )
        .bind(job.job_id)
        .fetch_one(store.pool())
        .await
        .unwrap(),
        0
    );
}

#[tokio::test]
async fn agent_logs_delete_job_parallel_workers_cannot_double_count() {
    let (store, scope, app) = setup().await;
    for name in ["a", "b"] {
        ingest_turn(&store, scope, app, name, "2026-10-07T00:00:00Z").await;
    }
    let (service, actor) = delete_service(&store, scope, app).await;
    let job = service
        .start_deletion(&actor, app, input(AgentLogsDeleteScope::all_time()))
        .await
        .unwrap();
    let (one, two) = tokio::join!(
        store.advance_agent_logs_delete_job(job.job_id),
        store.advance_agent_logs_delete_job(job.job_id)
    );
    one.unwrap();
    two.unwrap();
    store
        .advance_agent_logs_delete_job(job.job_id)
        .await
        .unwrap();
    let finished = service
        .deletion_job(&actor, app, Some(job.job_id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(finished.deleted_records, 2);
    assert_eq!(finished.status, AgentLogsDeleteJobStatus::Succeeded);
}

#[tokio::test]
async fn agent_logs_delete_job_stop_does_not_wait_for_inflight_batch_commit() {
    use std::time::Duration;
    let (store, scope, app) = setup().await;
    for name in ["a", "b"] {
        ingest_turn(&store, scope, app, name, "2026-10-07T00:00:00Z").await;
    }
    let (service, actor) = delete_service(&store, scope, app).await;
    let job = service
        .start_deletion(&actor, app, input(AgentLogsDeleteScope::all_time()))
        .await
        .unwrap();
    let mut blocker = store.pool().begin().await.unwrap();
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("fixture-delete-stop:{}", job.job_id))
        .execute(&mut *blocker)
        .await
        .unwrap();
    let trigger=format!("create function delay_job_delete() returns trigger language plpgsql as $$ begin perform pg_advisory_xact_lock(hashtextextended('fixture-delete-stop:{}',0)); return null; end $$; create constraint trigger fixture_delay_job_delete after delete on client_trajectory_sections deferrable initially deferred for each row execute function delay_job_delete();",job.job_id);
    sqlx::raw_sql(&trigger).execute(store.pool()).await.unwrap();
    let worker_store = store.clone();
    let worker =
        tokio::spawn(async move { worker_store.advance_agent_logs_delete_job(job.job_id).await });
    tokio::time::timeout(Duration::from_secs(10),async {
        loop {
            let blocked:bool=sqlx::query_scalar("select exists(select 1 from pg_locks where locktype='advisory' and not granted and pid in (select pid from pg_stat_activity where datname=current_database()))").fetch_one(store.pool()).await.unwrap();
            if blocked {break;} tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    let stop = tokio::time::timeout(
        Duration::from_secs(2),
        service.stop_deletion(&actor, app, job.job_id),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(stop.stop_requested);
    assert_eq!(stop.deleted_records, 0); // in-flight deletion is not yet committed
    blocker.commit().await.unwrap();
    worker.await.unwrap().unwrap();
    AgentLogsService::new(PgControlPlaneStore::new(store.pool().clone()))
        .advance_next_deletion()
        .await
        .unwrap();
    let stopped = service
        .deletion_job(&actor, app, Some(job.job_id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stopped.status, AgentLogsDeleteJobStatus::Stopped);
    assert_eq!(stopped.deleted_records, 1);
}
