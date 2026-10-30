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

async fn archive_head(store: &PgControlPlaneStore, request: Uuid) -> (String, Option<Uuid>, i64) {
    sqlx::query_as("select xmin::text,flow_run_id,persisted_through from client_trajectory_archive_heads where request_id=$1")
        .bind(request).fetch_one(store.pool()).await.unwrap()
}

async fn archive_original(
    store: &PgControlPlaneStore,
    request: Uuid,
    bytes: Vec<u8>,
) -> ClientTrajectoryArchiveReceipt {
    store
        .append_client_trajectory_archive(&AppendClientTrajectoryArchiveInput {
            request_id: request,
            part_id: Uuid::now_v7(),
            transport: ClientTrajectoryTransport::Http,
            frames: vec![ClientTrajectoryArchiveFrame {
                sequence: 0,
                kind: ClientTrajectoryFrameKind::Request,
                observed_at: AT.into(),
                bytes,
            }],
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn archive_head_first_bind_repeated_owner_raw_and_cleanup_preserve_versions() {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let database: String = sqlx::query_scalar("select current_database()")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(database.starts_with("test_backup_"));
    let contract: (String, bool, bool, bool, i64) = sqlx::query_as("select current_setting('transaction_isolation'),relrowsecurity,relforcerowsecurity,relhasrules,(select count(*) from pg_trigger where tgrelid=c.oid and not tgisinternal) from pg_class c where c.oid='client_trajectory_archive_heads'::regclass")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(contract, ("read committed".into(), false, false, false, 0));
    let request = Uuid::now_v7();
    let original = b" {\"input\":\"raw\\u0000 body\"} \n".to_vec();
    let receipt = archive_original(&store, request, original.clone()).await;
    assert_eq!(receipt.persisted_through, 1);
    let unbound = archive_head(&store, request).await;
    assert_eq!((unbound.1, unbound.2), (None, 1));
    begin(&store, flow, None, request).await;
    let bound = archive_head(&store, request).await;
    assert_eq!((bound.1, bound.2), (Some(flow), 1));
    assert_ne!(
        unbound.0, bound.0,
        "first NULL binding must create a row version"
    );
    for _ in 0..2 {
        begin(&store, flow, None, request).await;
        assert_eq!(
            archive_head(&store, request).await,
            bound,
            "same-owner semantic append must not rewrite the archive head"
        );
    }
    let foreign = Uuid::now_v7();
    sqlx::query("insert into flow_runs(id,application_id,flow_id,flow_draft_id,compiled_plan_id,run_mode,status,created_by) select $1,application_id,flow_id,flow_draft_id,compiled_plan_id,run_mode,status,created_by from flow_runs where id=$2")
        .bind(foreign).bind(flow).execute(&pool).await.unwrap();
    let rejected = store
        .append_client_trajectory(&AppendClientTrajectoryInput {
            flow_run_id: foreign,
            node_run_id: None,
            request_id: request,
            observed_at: AT.into(),
            fact: ClientTrajectoryFact::Integrity {
                status: "pending".into(),
                dropped_count: 0,
                persist_failed_count: 0,
            },
        })
        .await
        .unwrap_err();
    assert!(rejected.to_string().contains("capture scope mismatch"));
    assert_eq!(archive_head(&store, request).await, bound);
    let tail = vec![0, 255, b'X'];
    assert_eq!(
        archive_original(&store, request, tail.clone())
            .await
            .persisted_through,
        2
    );
    let watermark = archive_head(&store, request).await;
    assert_eq!((watermark.1, watermark.2), (Some(flow), 2));
    assert_ne!(
        watermark.0, bound.0,
        "raw append still advances the head watermark"
    );
    let frames = store
        .read_client_trajectory_archive(request, 0, 10)
        .await
        .unwrap();
    assert_eq!(
        frames
            .iter()
            .map(|frame| frame.sequence)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(frames[0].bytes, original);
    assert_eq!(frames[1].bytes, tail);
    store
        .discard_unbound_client_trajectory_archive(request)
        .await
        .unwrap();
    assert_eq!(
        archive_head(&store, request).await,
        watermark,
        "cleanup must preserve bound captures"
    );
    let unowned = Uuid::now_v7();
    archive_original(&store, unowned, b"unbound-only".to_vec()).await;
    store
        .discard_unbound_client_trajectory_archive(unowned)
        .await
        .unwrap();
    let remaining: (i64, i64) = sqlx::query_as("select (select count(*) from client_trajectory_archive_heads where request_id=$1),(select count(*) from client_trajectory_archive_parts where request_id=$1)")
        .bind(unowned).fetch_one(&pool).await.unwrap();
    assert_eq!(remaining, (0, 0));
}

#[tokio::test]
async fn archive_head_600_history_fact_replay_measures_only_one_physical_binding() {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let request = Uuid::now_v7();
    let items: Vec<Value> = (0..600)
        .map(|index| json!({"role":"user","content":format!("{index}:{}", "x".repeat(2048))}))
        .collect();
    let raw = serde_json::to_vec(
        &json!({"model":"gpt-5.6-luna","reasoning":{"effort":"max"},"input":items}),
    )
    .unwrap();
    archive_original(&store, request, raw.clone()).await;
    // Replay the recorder's fact shape: pending + initial root/timing +
    // decoded root/overview/timing + 600*(step/overview/result/timing) + complete.
    // This is a production-repository replay, not an invocation of the worker.
    let integrity = |status: &str| ClientTrajectoryFact::Integrity {
        status: status.into(),
        dropped_count: 0,
        persist_failed_count: 0,
    };
    let section_fact = |step_id, section: &str, value| ClientTrajectoryFact::Section {
        step_id,
        section: section.into(),
        value,
    };
    let root = step(flow, None, request, request, "submitted", "request");
    let timing = || json!({"observed_at":AT});
    let mut facts = vec![
        integrity("pending"),
        ClientTrajectoryFact::Step {
            step: Box::new(root.clone()),
        },
        section_fact(request, "timing", timing()),
        ClientTrajectoryFact::Step {
            step: Box::new(root),
        },
        section_fact(
            request,
            "overview",
            json!({"model":"gpt-5.6-luna","reasoning":{"effort":"max"}}),
        ),
        section_fact(request, "timing", timing()),
    ];
    for item in items {
        let id = Uuid::now_v7();
        let mut user = step(flow, None, request, id, "submitted", "user");
        user.call_id = None;
        facts.extend([
            ClientTrajectoryFact::Step {
                step: Box::new(user),
            },
            section_fact(id, "overview", item.clone()),
            section_fact(id, "result", item["content"].clone()),
            section_fact(id, "timing", timing()),
        ]);
    }
    facts.push(integrity("complete"));
    assert_eq!(facts.len(), 2407);
    let mut previous = archive_head(&store, request).await;
    let started = std::time::Instant::now();
    let mut physical_updates = 0;
    for fact in facts {
        append(&store, flow, None, request, fact).await;
        let current = archive_head(&store, request).await;
        physical_updates += usize::from(current.0 != previous.0);
        previous = current;
    }
    let wall = started.elapsed().as_secs_f64();
    let high_water = high_water(&store, flow).await;
    assert_eq!(high_water, 2407);
    assert_eq!((previous.1, previous.2), (Some(flow), 1));
    let archived = store
        .read_client_trajectory_archive(request, 0, 10)
        .await
        .unwrap();
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].bytes, raw);
    let users: i64 =
        sqlx::query_scalar("select count(*) from client_trajectory_steps where request_id=$1")
            .bind(request)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(users, 601);
    println!("G_HEAD_REPLAY semantic_appends=2407 head_update_statements=2407 append_transactions=2407 flow_lock_acquisitions=2407 head_xmin_changes={physical_updates} raw_frames=1 semantic_steps={users} wall_seconds={wall:.6} schema_isolation=owned_test_database");
    assert_eq!(
        physical_updates, 1,
        "all subsequent same-owner head updates must be physically absent"
    );
}
