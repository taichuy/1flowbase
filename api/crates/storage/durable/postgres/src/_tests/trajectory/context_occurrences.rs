use super::*;
use control_plane_contracts::ports::TrajectorySelection;

#[tokio::test]
async fn context_manifest_restores_external_results_order_paging_body_scope_and_integrity() {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool);
    let node = Uuid::now_v7();
    sqlx::query("insert into node_runs(id,scope_id,flow_run_id,node_id,node_type,node_alias,status) select $1,scope_id,id,'llm','llm','LLM','running' from flow_runs where id=$2")
        .bind(node).bind(flow).execute(store.pool()).await.unwrap();
    let mut expected = Vec::new();
    for version in ["first\0result", "retry result"] {
        let key = Uuid::now_v7();
        // Same bytes, different fact identities; same call_id on another invocation
        // with a different result version. No locally produced completion is needed.
        let messages = json!([{ "role":"tool","tool_call_id":"one","content":version },
            {"role":"tool","tool_call_id":"two","content":version}]);
        let entries:Vec<_> = (0..2).map(|index| {
            let id = Uuid::now_v7(); expected.push((id,messages[index].clone()));
            json!({"event_id":id,"metadata":{"step_key":format!("submitted:{index}"),
                "kind":"tool_result","status":"recorded","direction":"prepared","tool_call_id":if index==0 {"one"} else {"two"},
                "preview":version,"body_ref":{"step_key":key,"pointer":format!("/messages/{index}")}}})
        }).collect();
        native::append(&store,flow,node,"provider_semantic_step",json!({"source":"ai_native","invocation_id":version,"provider_attempt_index":0,
            "step_key":key,"kind":"model_call","status":"recorded","direction":"prepared",
            "body":json!({"messages":messages}).to_string(),"_context_occurrences":{"version":1,"entries":entries}})).await;
        native::append(
            &store,
            flow,
            node,
            "native_trajectory_integrity",
            json!({"invocation_id":version,"provider_attempt_index":0,
            "status":"complete","observed_count":3,"persist_failed_count":0,"dropped_count":0}),
        )
        .await;
    }
    let counts:(i64,i64,i64) = sqlx::query_as("select (select count(*) from runtime_events where flow_run_id=$1),(select count(*) from provider_semantic_trajectory_steps where flow_run_id=$1),(select count(*) from provider_semantic_trajectory_read_steps where flow_run_id=$1)")
        .bind(flow).fetch_one(store.pool()).await.unwrap();
    assert_eq!(counts, (4, 2, 6));
    let mut cursor = None;
    let mut items = Vec::new();
    loop {
        let page = store
            .provider_trajectory_page(flow, node, cursor, 1)
            .await
            .unwrap();
        assert_eq!(page.integrity, "complete");
        items.extend(page.items);
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(
        items.iter().map(|v| v.event_sequence).collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5, 6]
    );
    assert!(items
        .iter()
        .all(|v| v.metadata.get("_context_occurrences").is_none()));
    for ((id, wanted), step) in expected
        .iter()
        .zip(items.iter().filter(|v| v.metadata["kind"] == "tool_result"))
    {
        assert_eq!(*id, step.event_id);
        let body = store
            .provider_trajectory_body(flow, node, *id, None, 1, ProviderTrajectoryView::Semantic)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&body.items[0].body).unwrap(),
            *wanted
        );
        assert_eq!(body.items[0].sequence, step.event_sequence);
        assert!(store
            .provider_trajectory_body(
                flow,
                Uuid::now_v7(),
                *id,
                None,
                1,
                ProviderTrajectoryView::Semantic
            )
            .await
            .unwrap()
            .is_none());
        let targeted = store
            .provider_trajectory_filtered_page(
                flow,
                Some(node),
                None,
                1,
                TrajectorySelection {
                    target_id: Some(*id),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(targeted.items[0].event_id, *id);
    }
    let reopened = PgControlPlaneStore::new(store.pool().clone());
    assert_eq!(
        reopened
            .provider_trajectory_page(flow, node, None, 100)
            .await
            .unwrap()
            .items
            .len(),
        6
    );
    let application: Uuid = sqlx::query_scalar("select application_id from flow_runs where id=$1")
        .bind(flow)
        .fetch_one(store.pool())
        .await
        .unwrap();
    let before = serde_json::to_value(
        reopened
            .provider_trajectory_page(flow, node, None, 100)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(store
        .restore_native_context_occurrences(Uuid::now_v7(), flow)
        .await
        .is_err());
    assert_eq!(
        store
            .restore_native_context_occurrences(application, flow)
            .await
            .unwrap(),
        4
    );
    assert_eq!(
        store
            .restore_native_context_occurrences(application, flow)
            .await
            .unwrap(),
        0
    );
    let after = serde_json::to_value(
        reopened
            .provider_trajectory_page(flow, node, None, 100)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        before, after,
        "old physical journal and virtual read projection must have identical DTOs"
    );
    for (id, wanted) in expected {
        let body = store
            .provider_trajectory_body(flow, node, id, None, 1, ProviderTrajectoryView::Semantic)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&body.items[0].body).unwrap(),
            wanted
        );
    }
}

#[tokio::test]
async fn retried_invocation_keeps_first_child_cursor_id_and_latest_body_across_rollback() {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool);
    let node = Uuid::now_v7();
    sqlx::query("insert into node_runs(id,scope_id,flow_run_id,node_id,node_type,node_alias,status) select $1,scope_id,id,'llm','llm','LLM','running' from flow_runs where id=$2")
        .bind(node).bind(flow).execute(store.pool()).await.unwrap();
    let mut first_id = None;
    for version in ["first", "retry\0version"] {
        let key = Uuid::now_v7();
        let id = Uuid::now_v7();
        first_id.get_or_insert(id);
        native::append(&store,flow,node,"provider_semantic_step",json!({"source":"ai_native","invocation_id":"same-invocation","provider_attempt_index":0,
            "step_key":key,"kind":"model_call","status":"recorded","direction":"prepared",
            "body":json!({"messages":[{"role":"tool","tool_call_id":"shared-call","content":version}]}).to_string(),
            "_context_occurrences":{"version":1,"entries":[{"event_id":id,"metadata":{
                "kind":"tool_result","step_key":"submitted:0","status":"recorded","direction":"prepared","preview":version,
                "tool_call_id":"shared-call","body_ref":{"step_key":key,"pointer":"/messages/0"}}}]}})).await;
    }
    let before = store
        .provider_trajectory_page(flow, node, None, 100)
        .await
        .unwrap();
    assert_eq!(
        before.items.len(),
        3,
        "the old projection coalesces retries in one invocation"
    );
    let child = before
        .items
        .iter()
        .find(|item| item.metadata["kind"] == "tool_result")
        .unwrap();
    assert_eq!(child.event_id, first_id.unwrap());
    assert_eq!(child.event_sequence, 2);
    let body = store
        .provider_trajectory_body(
            flow,
            node,
            child.event_id,
            None,
            1,
            ProviderTrajectoryView::Semantic,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body.items[0].body).unwrap()["content"],
        "retry\0version"
    );
    let app: Uuid = sqlx::query_scalar("select application_id from flow_runs where id=$1")
        .bind(flow)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(
        store
            .restore_native_context_occurrences(app, flow)
            .await
            .unwrap(),
        2
    );
    let after = store
        .provider_trajectory_page(flow, node, None, 100)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(after).unwrap()
    );
}
