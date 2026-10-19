use super::*;

const CUTOFF: i64 = 20261019210000;

async fn old_event(
    pool: &sqlx::PgPool,
    flow: Uuid,
    id: Uuid,
    sequence: i64,
    payload: Value,
) -> anyhow::Result<()> {
    sqlx::query("insert into runtime_events(id,flow_run_id,sequence,event_type,layer,source,trust_level,payload,visibility,durability) values($1,$2,$3,'client_protocol_trajectory','runtime_item','host','host_fact',$4,'internal','durable')")
        .bind(id).bind(flow).bind(sequence).bind(payload).execute(pool).await?;
    Ok(())
}

#[tokio::test]
async fn current_schema_rejects_old_step_and_section_events_but_keeps_current_integrity() {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let request = Uuid::now_v7();
    let event = Uuid::now_v7();
    let store = PgControlPlaneStore::new(pool.clone());
    begin(&store, flow, None, request).await;
    for fact in [
        json!({"kind":"step", "step":step(flow,None,request,request,"emitted","assistant")}),
        json!({"kind":"section", "step_id":request,"section":"result","value":"old"}),
    ] {
        let result = old_event(
            &pool,
            flow,
            event,
            2,
            json!({"request_id":request,"fact":fact}),
        )
        .await;
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("legacy client trajectory event format is unsupported"));
    }
    let count: i64 = sqlx::query_scalar("select count(*) from runtime_events where id=$1")
        .bind(event)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let count: i64 =
        sqlx::query_scalar("select count(*) from client_trajectory_captures where request_id=$1")
            .bind(request)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn cutoff_retains_old_rows_but_refuses_their_read_format_and_preserves_current_nul_sections()
{
    let (pool, flow) =
        super::super::provider_protocol_capsule_store_tests::seeded_flow_run_before(Some(CUTOFF))
            .await;
    let store = PgControlPlaneStore::new(pool.clone());
    let request = Uuid::now_v7();
    begin(&store, flow, None, request).await;
    let current_step = Uuid::now_v7();
    append(
        &store,
        flow,
        None,
        request,
        ClientTrajectoryFact::Step {
            step: Box::new(step(
                flow,
                None,
                request,
                current_step,
                "emitted",
                "assistant",
            )),
        },
    )
    .await;
    let original = json!({"key\0":"current body\0", "literal":"\\u0000"});
    section(
        &store,
        flow,
        None,
        request,
        current_step,
        "result",
        original.clone(),
    )
    .await;
    let old_step = Uuid::now_v7();
    let step_event = Uuid::now_v7();
    let section_event = Uuid::now_v7();
    let old = step(flow, None, request, old_step, "emitted", "assistant");
    old_event(
        &pool,
        flow,
        step_event,
        100,
        json!({"request_id":request,"observed_at":AT,"fact":{"kind":"step","step":old}}),
    )
    .await
    .unwrap();
    let payload = json!({"request_id":request,"observed_at":AT,"fact":{"kind":"section","step_id":current_step,"section":"schema","value":"old result"}});
    old_event(&pool, flow, section_event, 101, payload.clone())
        .await
        .unwrap();
    let before: Value =
        sqlx::query_scalar("select client_trajectory_section_original_value($1,$2)")
            .bind(section_event)
            .bind(flow)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(before, json!("old result"));

    storage_durable_postgres::run_migrations(&pool)
        .await
        .unwrap();
    let retained: Value = sqlx::query_scalar("select payload from runtime_events where id=$1")
        .bind(section_event)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(retained, payload);
    assert!(store
        .client_trajectory_section(flow, None, current_step, "schema", None, 1)
        .await
        .is_err());
    assert!(store
        .client_trajectory_page(flow, None, None, 10)
        .await
        .is_err());
    let restored = store
        .client_trajectory_section(flow, None, current_step, "result", None, 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(restored.items[0].value, original);
    let high: i64 =
        sqlx::query_scalar("select runtime_event_sequence_high_water from flow_runs where id=$1")
            .bind(flow)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(high, 101);
    section(
        &store,
        flow,
        None,
        request,
        current_step,
        "timing",
        json!({"observed_at":AT}),
    )
    .await;
    let timing = store
        .client_trajectory_section(flow, None, current_step, "timing", None, 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(timing.items[0].sequence, 102);
    assert_eq!(timing.items[0].value, json!({"observed_at":AT}));
}
