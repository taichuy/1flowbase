use super::*;

async fn high_water(store: &PgControlPlaneStore, flow: Uuid) -> i64 {
    sqlx::query_scalar("select runtime_event_sequence_high_water from flow_runs where id=$1")
        .bind(flow)
        .fetch_one(store.pool())
        .await
        .unwrap()
}

#[tokio::test]
async fn client_trajectory_concurrent_semantic_facts_reserve_unique_sequences() {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool);
    let request = Uuid::now_v7();
    begin(&store, flow, None, request).await;
    let before = high_water(&store, flow).await;
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(8));
    let mut writers = Vec::new();
    for _ in 0..8 {
        let store = store.clone();
        let barrier = barrier.clone();
        writers.push(tokio::spawn(async move {
            let id = Uuid::now_v7();
            barrier.wait().await;
            append(
                &store,
                flow,
                None,
                request,
                ClientTrajectoryFact::Step {
                    step: Box::new(step(flow, None, request, id, "emitted", "tool_call")),
                },
            )
            .await;
            for name in ["overview", "timing"] {
                section(
                    &store,
                    flow,
                    None,
                    request,
                    id,
                    name,
                    json!({"observed_at":AT}),
                )
                .await;
            }
        }));
    }
    for writer in writers {
        writer.await.unwrap();
    }
    // Check the shared sequence space, including the capture's real runtime event.
    let mut sequences: Vec<i64> = sqlx::query_scalar(
        "select sequence from runtime_events where flow_run_id=$1 union all select event_sequence from client_trajectory_steps where flow_run_id=$1 union all select event_sequence from client_trajectory_sections where flow_run_id=$1",
    )
    .bind(flow)
    .fetch_all(store.pool())
    .await
    .unwrap();
    sequences.sort_unstable();
    let appended: Vec<_> = sequences.into_iter().filter(|seq| *seq > before).collect();
    assert_eq!(appended, ((before + 1)..=(before + 24)).collect::<Vec<_>>());
    assert_eq!(high_water(&store, flow).await, before + 24);
    let page = store
        .client_trajectory_page(flow, None, None, 50)
        .await
        .unwrap();
    assert_eq!(page.items.len(), 8);
    for item in page.items {
        for name in ["overview", "timing"] {
            let stored = store
                .client_trajectory_section(flow, None, item.id, name, None, 10)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(stored.items.len(), 1);
            assert!(stored.items[0].sequence > item.sequence);
        }
    }
}

#[tokio::test]
async fn client_trajectory_invalid_section_preserves_committed_facts_and_later_timing() {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let request = Uuid::now_v7();
    begin(&store, flow, None, request).await;
    append(
        &store,
        flow,
        None,
        request,
        ClientTrajectoryFact::Step {
            step: Box::new(step(flow, None, request, request, "submitted", "request")),
        },
    )
    .await;
    let overview = json!({"model":"fixture", "input":"committed request"});
    section(
        &store,
        flow,
        None,
        request,
        request,
        "overview",
        overview.clone(),
    )
    .await;
    let before = high_water(&store, flow).await;
    let error = store
        .append_client_trajectory(&AppendClientTrajectoryInput {
            flow_run_id: flow,
            node_run_id: None,
            request_id: request,
            observed_at: AT.into(),
            fact: ClientTrajectoryFact::Section {
                step_id: request,
                section: "invalid_section".into(),
                value: json!({"must_not_persist":true}),
            },
        })
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("client trajectory section invalid"));
    // A fresh adapter reads only committed facts after the failed append rolls back.
    let reader = PgControlPlaneStore::new(pool);
    assert_eq!(high_water(&reader, flow).await, before);
    let page = reader
        .client_trajectory_page(flow, None, None, 10)
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, request);
    let stable_sequence = page.items[0].sequence;
    let stored = reader
        .client_trajectory_section(flow, None, request, "overview", None, 10)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.items.len(), 1);
    assert_eq!(stored.items[0].value, overview);
    assert_eq!(stored.items[0].sequence, before);
    let section_count: i64 =
        sqlx::query_scalar("select count(*) from client_trajectory_sections where flow_run_id=$1")
            .bind(flow)
            .fetch_one(reader.pool())
            .await
            .unwrap();
    assert_eq!(section_count, 1);
    let timing = json!({"observed_at":AT});
    section(
        &reader,
        flow,
        None,
        request,
        request,
        "timing",
        timing.clone(),
    )
    .await;
    let stored = reader
        .client_trajectory_section(flow, None, request, "timing", None, 10)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.items.len(), 1);
    assert_eq!(stored.items[0].value, timing);
    assert_eq!(stored.items[0].sequence, before + 1);
    assert_eq!(high_water(&reader, flow).await, before + 1);
    let page = reader
        .client_trajectory_page(flow, None, None, 10)
        .await
        .unwrap();
    assert_eq!(page.items[0].sequence, stable_sequence);
}
