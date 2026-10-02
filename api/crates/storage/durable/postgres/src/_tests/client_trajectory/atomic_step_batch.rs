use super::*;

fn inputs(flow: Uuid, request: Uuid, id: Uuid) -> Vec<AppendClientTrajectoryInput> {
    vec![
        ClientTrajectoryFact::Step {
            step: Box::new(step(flow, None, request, id, "submitted", "tool_call")),
        },
        ClientTrajectoryFact::Section {
            step_id: id,
            section: "overview".into(),
            value: json!({"exact":"北京\0🌍","optional":null,"enabled":true}),
        },
        ClientTrajectoryFact::Section {
            step_id: id,
            section: "schema".into(),
            value: json!({"const":"atomic rollback\0body"}),
        },
        ClientTrajectoryFact::Section {
            step_id: id,
            section: "timing".into(),
            value: json!({"observed_at":AT}),
        },
    ]
    .into_iter()
    .map(|fact| AppendClientTrajectoryInput {
        flow_run_id: flow,
        node_run_id: None,
        request_id: request,
        observed_at: AT.into(),
        fact,
    })
    .collect()
}
async fn state(store: &PgControlPlaneStore, flow: Uuid) -> (i64, i64, i64, i64) {
    sqlx::query_as(
        "select runtime_event_sequence_high_water,
        (select count(*) from client_trajectory_steps where flow_run_id=$1),
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
async fn atomic_step_batch_rolls_back_every_row_and_reuses_range_after_late_failure() {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool);
    let request = Uuid::now_v7();
    let id = Uuid::now_v7();
    begin(&store, flow, None, request).await;
    let before = state(&store, flow).await;
    sqlx::query("alter table client_trajectory_sections add constraint atomic_step_fixture_failure check (section <> 'schema')")
        .execute(store.pool()).await.unwrap();
    let batch = inputs(flow, request, id);
    assert!(store.append_client_trajectory_batch(&batch).await.is_err());
    assert_eq!(
        state(&store, flow).await,
        before,
        "late failure must rollback step, earlier sections, canonical bodies and high-water"
    );
    sqlx::query(
        "alter table client_trajectory_sections drop constraint atomic_step_fixture_failure",
    )
    .execute(store.pool())
    .await
    .unwrap();
    assert!(store.append_client_trajectory_batch(&batch).await.unwrap());
    let sequences: Vec<i64> = sqlx::query_scalar("select event_sequence from client_trajectory_steps where id=$1
        union all select event_sequence from client_trajectory_sections where step_id=$1 order by event_sequence")
        .bind(id).fetch_all(store.pool()).await.unwrap();
    assert_eq!(
        sequences,
        ((before.0 + 1)..=(before.0 + 4)).collect::<Vec<_>>()
    );
    assert_eq!(state(&store, flow).await.0, before.0 + 4);
    let page = store
        .client_trajectory_page(flow, None, None, 100)
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, id);
    let restored = store
        .client_trajectory_section(flow, None, id, "overview", None, 100)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(restored.items.len(), 1);
    assert_eq!(
        restored.items[0].value,
        json!({"exact":"北京\0🌍","optional":null,"enabled":true})
    );
    assert_eq!(restored.items[0].sequence, before.0 + 2);
}

#[tokio::test]
async fn atomic_step_batch_rejects_cross_capture_and_section_owner_without_writes() {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool);
    let request = Uuid::now_v7();
    begin(&store, flow, None, request).await;
    let before = state(&store, flow).await;
    let mut batch = inputs(flow, request, Uuid::now_v7());
    batch[1].request_id = Uuid::now_v7();
    assert!(store.append_client_trajectory_batch(&batch).await.is_err());
    assert_eq!(state(&store, flow).await, before);
    batch[1].request_id = request;
    if let ClientTrajectoryFact::Section { step_id, .. } = &mut batch[2].fact {
        *step_id = Uuid::now_v7();
    }
    assert!(store.append_client_trajectory_batch(&batch).await.is_err());
    assert_eq!(state(&store, flow).await, before);
}

#[tokio::test]
async fn atomic_step_batches_serialize_with_single_writers_and_fk_readers() {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool);
    let request = Uuid::now_v7();
    begin(&store, flow, None, request).await;
    let mut reader = store.pool().begin().await.unwrap();
    sqlx::query("select id from flow_runs where id=$1 for key share")
        .bind(flow)
        .fetch_one(&mut *reader)
        .await
        .unwrap();
    let outcome = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        let mut tasks = Vec::new();
        for _ in 0..4 {
            let store = store.clone();
            tasks.push(tokio::spawn(async move {
                for _ in 0..4 {
                    assert!(store
                        .append_client_trajectory_batch(&inputs(flow, request, Uuid::now_v7()))
                        .await
                        .unwrap());
                    append(
                        &store,
                        flow,
                        None,
                        request,
                        ClientTrajectoryFact::ResponseLink {
                            response_id: Uuid::now_v7().to_string(),
                        },
                    )
                    .await;
                }
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
    })
    .await;
    reader.rollback().await.unwrap();
    outcome.expect("atomic batches must coexist with FK readers");
    let (count, unique, min, max): (i64, i64, i64, i64) = sqlx::query_as(
        "select count(*),count(distinct sequence),min(sequence),max(sequence) from (
        select sequence from runtime_events where flow_run_id=$1 union all
        select event_sequence from client_trajectory_steps where flow_run_id=$1 union all
        select event_sequence from client_trajectory_sections where flow_run_id=$1) facts",
    )
    .bind(flow)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!((count, unique, min, max), (81, 81, 1, 81));
    assert_eq!(state(&store, flow).await.0, max);
    assert_eq!(
        store
            .client_trajectory_page(flow, None, None, 100)
            .await
            .unwrap()
            .items
            .len(),
        16
    );
}
