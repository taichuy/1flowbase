use super::*;
use sqlx::{postgres::PgPoolOptions, Row};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

fn input(flow: Uuid, request: Uuid, fact: ClientTrajectoryFact) -> AppendClientTrajectoryInput {
    AppendClientTrajectoryInput {
        flow_run_id: flow,
        node_run_id: None,
        request_id: request,
        observed_at: AT.into(),
        fact,
    }
}
fn section_input(flow: Uuid, request: Uuid, section: &str) -> AppendClientTrajectoryInput {
    input(
        flow,
        request,
        ClientTrajectoryFact::Section {
            step_id: request,
            section: section.into(),
            value: json!(format!("exact {section}\0\n")),
        },
    )
}
async fn ready() -> (
    sqlx::PgPool,
    PgControlPlaneStore,
    Uuid,
    Uuid,
    Arc<AtomicU64>,
) {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    // Keep the original pool alive: it owns the isolated schema's cleanup guard.
    let acquires = Arc::new(AtomicU64::new(0));
    let acquire_counter = acquires.clone();
    let single = PgPoolOptions::new()
        .max_connections(1)
        .before_acquire(move |_connection, _metadata| {
            acquire_counter.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(true) })
        })
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();
    let store = PgControlPlaneStore::new(single);
    let request = Uuid::now_v7();
    begin(&store, flow, None, request).await;
    (pool, store, flow, request, acquires)
}
async fn install_audit(pool: &sqlx::PgPool, reject_result: bool, lock: Option<i64>) {
    sqlx::query(
        "create table group_fact_audit (ordinal bigserial, kind text, pid int, xid bigint)",
    )
    .execute(pool)
    .await
    .unwrap();
    let action = if reject_result {
        "if tg_table_name='client_trajectory_sections' then if new.section='result' then raise exception 'fixture section rejected'; end if; end if;".to_owned()
    } else if let Some(lock) = lock {
        format!("if tg_table_name='client_trajectory_sections' then if new.section='result' then perform pg_advisory_xact_lock({lock}); end if; end if;")
    } else {
        String::new()
    };
    sqlx::query(&format!(r#"create function group_fact_audit_fn() returns trigger language plpgsql as $$
        begin
            {action}
            insert into group_fact_audit(kind,pid,xid) values(tg_table_name,pg_backend_pid(),txid_current());
            return new;
        end $$"#)).execute(pool).await.unwrap();
    for table in ["client_trajectory_steps", "client_trajectory_sections"] {
        sqlx::query(&format!("create trigger group_fact_audit_trigger after insert on {table} for each row execute function group_fact_audit_fn()"))
            .execute(pool).await.unwrap();
    }
}
fn root_input(flow: Uuid, request: Uuid) -> AppendClientTrajectoryInput {
    input(
        flow,
        request,
        ClientTrajectoryFact::Step {
            step: Box::new(step(flow, None, request, request, "submitted", "request")),
        },
    )
}

#[tokio::test]
async fn client_trajectory_group_reuses_backend_with_independent_transactions() {
    let (pool, store, flow, request, acquires) = ready().await;
    install_audit(&pool, false, None).await;
    let inputs = [
        root_input(flow, request),
        section_input(flow, request, "overview"),
        section_input(flow, request, "timing"),
    ];
    let before = acquires.load(Ordering::SeqCst);
    let results = tokio::time::timeout(
        Duration::from_secs(10),
        store.append_client_trajectory_group(&inputs),
    )
    .await
    .unwrap();
    assert_eq!(
        acquires.load(Ordering::SeqCst) - before,
        1,
        "one pool acquire for the entire successful natural group"
    );
    assert_eq!(results.len(), inputs.len());
    assert!(results.iter().all(Result::is_ok));
    let audit = sqlx::query("select pid,xid from group_fact_audit order by ordinal")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(audit.len(), 3);
    assert!(audit
        .iter()
        .all(|row| row.get::<i32, _>("pid") == audit[0].get::<i32, _>("pid")));
    let xids: std::collections::BTreeSet<i64> = audit.iter().map(|row| row.get("xid")).collect();
    assert_eq!(xids.len(), 3, "each fact has its own committed transaction");
}

#[tokio::test]
async fn client_trajectory_group_sql_error_keeps_prefix_suffix_and_single_pool_raw() {
    let (pool, store, flow, request, _acquires) = ready().await;
    install_audit(&pool, true, None).await;
    let raw = json!({"direction":"submitted","encoding":"utf8","body":"original\0\n","frame_kind":"request"});
    let inputs = [
        root_input(flow, request),
        section_input(flow, request, "overview"),
        input(
            flow,
            request,
            ClientTrajectoryFact::Section {
                step_id: request,
                section: "raw".into(),
                value: raw.clone(),
            },
        ),
        section_input(flow, request, "result"),
        section_input(flow, request, "timing"),
    ];
    let results = tokio::time::timeout(
        Duration::from_secs(10),
        store.append_client_trajectory_group(&inputs),
    )
    .await
    .unwrap();
    assert_eq!(results.len(), 5);
    assert!(
        results[0].is_ok()
            && results[1].is_ok()
            && results[2].is_ok()
            && results[3].is_err()
            && results[4].is_ok()
    );
    for name in ["overview", "timing"] {
        let section = store
            .client_trajectory_section(flow, None, request, name, None, 10)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(section.items.len(), 1);
        assert_eq!(section.items[0].value, json!(format!("exact {name}\0\n")));
    }
    assert_eq!(
        store
            .client_trajectory_section(flow, None, request, "result", None, 10)
            .await
            .unwrap()
            .unwrap()
            .items
            .len(),
        0
    );
    let archived = store
        .client_trajectory_section(flow, None, request, "raw", None, 10)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(archived.items[0].value, raw);
    let count: i64 = sqlx::query_scalar("select count(*) from group_fact_audit")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 3);
}

#[tokio::test]
async fn client_trajectory_group_cancel_blocked_second_fact_keeps_prefix_releases_pool() {
    let (pool, store, flow, request, _acquires) = ready().await;
    let lock = 710031i64;
    install_audit(&pool, false, Some(lock)).await;
    let mut blocker = pool.acquire().await.unwrap();
    sqlx::query("select pg_advisory_lock($1)")
        .bind(lock)
        .execute(&mut *blocker)
        .await
        .unwrap();
    let worker_store = store.clone();
    let task = tokio::spawn(async move {
        worker_store
            .append_client_trajectory_group(&[
                root_input(flow, request),
                section_input(flow, request, "result"),
                section_input(flow, request, "timing"),
            ])
            .await
    });
    // A different connection must see the committed prefix while fact two is active.
    let pid = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let pid: Option<i32> = sqlx::query_scalar("select pid from group_fact_audit order by ordinal limit 1").fetch_optional(&pool).await.unwrap();
            if let Some(pid) = pid {
                let waiting: bool = sqlx::query_scalar("select exists(select 1 from pg_stat_activity where pid=$1 and wait_event='advisory')").bind(pid).fetch_one(&pool).await.unwrap();
                if waiting { break pid; }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    let prefix: i64 =
        sqlx::query_scalar("select count(*) from client_trajectory_steps where id=$1")
            .bind(request)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(prefix, 1);
    assert!(!task.is_finished());
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    sqlx::query("select pg_advisory_unlock($1)")
        .bind(lock)
        .execute(&mut *blocker)
        .await
        .unwrap();
    drop(blocker);
    // Reusing the single pool forces queued rollback to finish before another fact.
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        store.append_client_trajectory(&section_input(flow, request, "overview")),
    )
    .await
    .unwrap();
    result.unwrap();
    let sections: Vec<String> = sqlx::query_scalar(
        "select section from client_trajectory_sections where step_id=$1 order by section",
    )
    .bind(request)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(sections, ["overview"]);
    let count: i64 = sqlx::query_scalar("select count(*) from group_fact_audit where pid=$1")
        .bind(pid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(count >= 1);
    store.pool().close().await;
}
