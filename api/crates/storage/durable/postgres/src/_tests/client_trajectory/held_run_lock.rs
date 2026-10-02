use super::*;

#[tokio::test]
async fn client_trajectory_held_run_lock_serializes_nonraw_facts_and_allows_fk_reader() {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool);
    let request = Uuid::now_v7();
    begin(&store, flow, None, request).await;
    // Already terminal observations remain writable; this is not the execution append fence.
    sqlx::query("update flow_runs set status='succeeded' where id=$1")
        .bind(flow)
        .execute(store.pool())
        .await
        .unwrap();
    let mut reader = store.pool().begin().await.unwrap();
    sqlx::query("select id from flow_runs where id=$1 for key share")
        .bind(flow)
        .fetch_one(&mut *reader)
        .await
        .unwrap();
    let write_store = store.clone();
    let completed = tokio::time::timeout(std::time::Duration::from_secs(10), async move {
        let mut writers = Vec::new();
        for _ in 0..4 {
            let store = write_store.clone();
            writers.push(tokio::spawn(async move {
                for _ in 0..8 {
                    let id = Uuid::now_v7();
                    append(
                        &store,
                        flow,
                        None,
                        request,
                        ClientTrajectoryFact::Step {
                            step: Box::new(step(flow, None, request, id, "submitted", "message")),
                        },
                    )
                    .await;
                    section(
                        &store,
                        flow,
                        None,
                        request,
                        id,
                        "timing",
                        json!({"observed_at": AT}),
                    )
                    .await;
                }
            }));
        }
        for writer in writers {
            writer.await.unwrap();
        }
    })
    .await;
    reader.rollback().await.unwrap();
    completed.expect("nonraw facts must coexist with FK readers and finish without deadlock");
    let (count, unique, minimum, maximum): (i64, i64, i64, i64) = sqlx::query_as(
        "select count(*),count(distinct sequence),min(sequence),max(sequence) from (
            select sequence from runtime_events where flow_run_id=$1
            union all select event_sequence from client_trajectory_steps where flow_run_id=$1
            union all select event_sequence from client_trajectory_sections where flow_run_id=$1
        ) facts",
    )
    .bind(flow)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!((count, unique, minimum, maximum), (65, 65, 1, 65));
    let high: i64 =
        sqlx::query_scalar("select runtime_event_sequence_high_water from flow_runs where id=$1")
            .bind(flow)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(high, maximum);
    let page = store
        .client_trajectory_page(flow, None, None, 100)
        .await
        .unwrap();
    assert_eq!(
        page.items.len(),
        32,
        "all nonraw steps remain visible to the UI"
    );
}

async fn committed_state(store: &PgControlPlaneStore, flow: Uuid) -> (i64, i64, i64) {
    sqlx::query_as(
        "select runtime_event_sequence_high_water,
        (select count(*) from client_trajectory_sections where flow_run_id=$1),
        (select count(*) from runtime_canonical_contents where application_id=f.application_id)
        from flow_runs f where id=$1",
    )
    .bind(flow)
    .fetch_one(store.pool())
    .await
    .unwrap()
}

#[tokio::test]
async fn client_trajectory_held_run_lock_rolls_back_failed_section_and_reuses_sequence() {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool);
    let request = Uuid::now_v7();
    let id = Uuid::now_v7();
    begin(&store, flow, None, request).await;
    append(
        &store,
        flow,
        None,
        request,
        ClientTrajectoryFact::Step {
            step: Box::new(step(flow, None, request, id, "submitted", "message")),
        },
    )
    .await;
    let before = committed_state(&store, flow).await;
    // This formal fixture owns an isolated schema. Fail the section INSERT after
    // its sequence reservation and canonical body write, rather than before validation.
    sqlx::query("alter table client_trajectory_sections add constraint held_run_lock_fixture_failure check (section <> 'schema')")
        .execute(store.pool()).await.unwrap();
    let value = json!({"request":request,"original":"rollback\u{0000}body"});
    let result = store
        .append_client_trajectory(&AppendClientTrajectoryInput {
            flow_run_id: flow,
            node_run_id: None,
            request_id: request,
            observed_at: AT.into(),
            fact: ClientTrajectoryFact::Section {
                step_id: id,
                section: "schema".into(),
                value: value.clone(),
            },
        })
        .await;
    assert!(result.is_err());
    assert_eq!(
        committed_state(&store, flow).await,
        before,
        "failed fact must not commit its sequence, directory or canonical body"
    );
    section(&store, flow, None, request, id, "overview", value.clone()).await;
    let after = committed_state(&store, flow).await;
    assert_eq!(after, (before.0 + 1, before.1 + 1, before.2 + 1));
    let sequence: i64 = sqlx::query_scalar("select event_sequence from client_trajectory_sections where flow_run_id=$1 and step_id=$2 and section='overview'")
        .bind(flow).bind(id).fetch_one(store.pool()).await.unwrap();
    assert_eq!(sequence, before.0 + 1);
    let read = store
        .client_trajectory_section(flow, None, id, "overview", None, 10)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        read.items[0].value, value,
        "exact original remains readable after rollback and retry"
    );
}
