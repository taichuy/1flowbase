use super::native_snapshots::event;
use super::*;
use std::time::Duration;

async fn sibling(store: &PgControlPlaneStore, run: Uuid) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("insert into flow_runs(id,application_id,flow_id,flow_draft_id,compiled_plan_id,run_mode,status,created_by) select $1,application_id,flow_id,flow_draft_id,compiled_plan_id,run_mode,'running',created_by from flow_runs where id=$2")
        .bind(id).bind(run).execute(store.pool()).await.unwrap();
    id
}
const BODY: &str = r#"{"messages":[{"role":"tool","content":"exact\u0000result"}],"tools":[]}"#;

#[tokio::test]
async fn first_snapshot_writes_are_atomic_and_existing_owners_do_not_wait_for_application_lock() {
    let (store, run, app, _) = seed().await;
    let mut runs = vec![run];
    for _ in 1..16 {
        runs.push(sibling(&store, run).await);
    }
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(runs.len()));
    let mut tasks = Vec::new();
    for run in &runs {
        let store = store.clone();
        let barrier = barrier.clone();
        let run = *run;
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            store.append_runtime_event(&event(run, BODY)).await.unwrap()
        }));
    }
    for task in tasks {
        assert_eq!(task.await.unwrap().payload["body"], BODY);
    }
    let count: i64 = sqlx::query_scalar("select count(*) from runtime_native_snapshot_manifests")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(
        count, 1,
        "concurrent first writers publish a single immutable owner"
    );
    let mut maintenance = store.pool().begin().await.unwrap();
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("native-snapshots:{app}"))
        .execute(&mut *maintenance)
        .await
        .unwrap();
    let mut tasks = Vec::new();
    for run in runs {
        let store = store.clone();
        tasks.push(tokio::spawn(async move {
            store
                .append_runtime_events(&[event(run, BODY), event(run, BODY)])
                .await
                .unwrap()
        }));
    }
    tokio::time::timeout(Duration::from_secs(5), async {
        for task in tasks {
            for row in task.await.unwrap() {
                assert_eq!(row.payload["body"], BODY);
            }
        }
    })
    .await
    .expect("immutable reuse must progress while first-write/maintenance lock is held");
    maintenance.rollback().await.unwrap();
    let count: i64 = sqlx::query_scalar("select count(*) from runtime_native_snapshot_manifests")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn last_owner_reclamation_and_partial_batch_retry_preserve_exact_snapshot() {
    let (store, run, app, _) = seed().await;
    let other = sibling(&store, run).await;
    let old = store.append_runtime_event(&event(run, BODY)).await.unwrap();
    let mut deleting = store.pool().begin().await.unwrap();
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("native-snapshots:{app}"))
        .execute(&mut *deleting)
        .await
        .unwrap();
    sqlx::query("delete from runtime_events where id=$1")
        .bind(old.id)
        .execute(&mut *deleting)
        .await
        .unwrap();
    let writer = store.clone();
    let changed = r#"{"messages":[{"content":"new version"}]}"#;
    let task = tokio::spawn(async move {
        writer
            .append_runtime_events(&[event(other, BODY), event(other, changed)])
            .await
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    deleting.commit().await.unwrap();
    let rows = tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(rows[0].payload["body"], BODY);
    assert_eq!(rows[1].payload["body"], changed);
    assert_eq!(store.list_runtime_events(other, 0).await.unwrap().len(), 2);
}

#[tokio::test]
async fn failed_first_publication_rolls_back_and_retry_survives_store_recreation() {
    let (store, run, _, _) = seed().await;
    sqlx::raw_sql("create function reject_snapshot_event() returns trigger language plpgsql as $$ begin raise exception 'controlled publication failure'; end $$; create trigger reject_snapshot_event before insert on runtime_events for each row execute function reject_snapshot_event()")
        .execute(store.pool()).await.unwrap();
    assert!(store.append_runtime_event(&event(run, BODY)).await.is_err());
    let counts: (i64,i64,i64) = sqlx::query_as("select (select count(*) from runtime_native_snapshot_manifests),(select count(*) from runtime_native_snapshot_items),(select count(*) from runtime_native_snapshot_references)")
        .fetch_one(store.pool()).await.unwrap();
    assert_eq!(counts, (0, 0, 0));
    sqlx::query("drop trigger reject_snapshot_event on runtime_events")
        .execute(store.pool())
        .await
        .unwrap();
    let recreated = PgControlPlaneStore::new(store.pool().clone());
    let saved = recreated
        .append_runtime_event(&event(run, BODY))
        .await
        .unwrap();
    assert_eq!(saved.payload["body"], BODY);
    assert_eq!(
        recreated.list_runtime_events(run, 0).await.unwrap()[0].payload,
        saved.payload
    );
}
