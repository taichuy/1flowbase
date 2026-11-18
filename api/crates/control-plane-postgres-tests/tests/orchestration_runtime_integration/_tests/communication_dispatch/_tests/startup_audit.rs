use super::*;
use control_plane::orchestration_runtime::{ContinueFlowDebugRunCommand, StartFlowDebugRunCommand};

#[tokio::test]
async fn workflow_observation_starts_and_executes_nodes_during_unrelated_cleanup() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let (runtime, _, _package) =
        super::super::callback_resume::semantic_runtime::seed_runtime_consumer(
            &store, &seeded, &compiled, None,
        )
        .await;
    let old = seed_flow_run(&store, &seeded, &compiled, OffsetDateTime::now_utc()).await;
    store
        .append_audit_hash(old.id, "runtime_events", Uuid::now_v7(), json!({}))
        .await
        .unwrap();
    let mut cleanup = store.pool().begin().await.unwrap();
    sqlx::query("delete from flow_runs where id = $1")
        .bind(old.id)
        .execute(&mut *cleanup)
        .await
        .unwrap();
    let document = json!({
        "schemaVersion":"1flowbase.flow/v2",
        "meta":{"flowId":seeded.flow_id,"name":"Startup audit","description":"","tags":[]},
        "graph":{
            "nodes":[
                {"id":"start","type":"start","alias":"Start","description":"","containerId":null,"position":{"x":0,"y":0},"configVersion":1,"config":{},"bindings":{},"outputs":[]},
                {"id":"answer","type":"answer","alias":"Answer","description":"","containerId":null,"position":{"x":240,"y":0},"configVersion":1,"config":{},"bindings":{"answer_template":{"kind":"templated_text","value":"started during cleanup"}},"outputs":[{"key":"answer","title":"Answer","valueType":"string"}]}
            ],
            "edges":[{"id":"edge","source":"start","target":"answer","sourceHandle":null,"targetHandle":null,"containerId":null,"points":[]}]
        },
        "editor":{"viewport":{"x":0,"y":0,"zoom":1},"annotations":[],"activeContainerPath":[]}
    });
    let execution = async {
        let started = runtime
            .start_flow_debug_run(StartFlowDebugRunCommand {
                actor_user_id: seeded.actor_user_id,
                application_id: seeded.application_id,
                input_payload: json!({}),
                document_snapshot: Some(document),
                debug_session_id: None,
            })
            .await
            .unwrap();
        let audit_count: i64 =
            sqlx::query_scalar("select count(*) from runtime_audit_hashes where flow_run_id = $1")
                .bind(started.flow_run.id)
                .fetch_one(store.pool())
                .await
                .unwrap();
        assert_eq!(
            audit_count, 3,
            "startup must retain all billing audit hashes"
        );
        runtime
            .continue_flow_debug_run(ContinueFlowDebugRunCommand {
                application_id: seeded.application_id,
                flow_run_id: started.flow_run.id,
                workspace_id: seeded.workspace_id,
            })
            .await
            .unwrap()
    };
    let outcome = tokio::time::timeout(std::time::Duration::from_secs(5), execution).await;
    // Release only our isolated fixture transaction, even if startup timed out.
    cleanup.rollback().await.unwrap();
    let detail = outcome.expect("cleanup of another run must not prevent node execution");
    assert_eq!(detail.flow_run.status, FlowRunStatus::Succeeded);
    assert_eq!(detail.node_runs.len(), 2);
    assert!(detail
        .node_runs
        .iter()
        .all(|n| n.status == NodeRunStatus::Succeeded));
    assert_eq!(
        detail.flow_run.output_payload["answer"],
        "started during cleanup"
    );
    let visible = store
        .get_run_node_last_run(seeded.application_id, detail.flow_run.id, "answer")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(visible.node_run.status, NodeRunStatus::Succeeded);
}
