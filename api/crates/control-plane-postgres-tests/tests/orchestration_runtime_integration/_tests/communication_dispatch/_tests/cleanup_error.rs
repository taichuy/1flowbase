use super::*;
use control_plane::orchestration_runtime::{ContinueFlowDebugRunCommand, StartFlowDebugRunCommand};

#[tokio::test]
async fn workflow_cleanup_conflict_reason_reaches_flow_and_node_details() {
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
    let document = json!({
        "schemaVersion":"1flowbase.flow/v2",
        "meta":{"flowId":seeded.flow_id,"name":"Cleanup error","description":"","tags":[]},
        "graph":{
            "nodes":[
                {"id":"start","type":"start","alias":"Start","description":"","containerId":null,"position":{"x":0,"y":0},"configVersion":1,"config":{},"bindings":{},"outputs":[]},
                {"id":"receipt","type":"code","alias":"Confirm cleanup receipt","description":"","containerId":null,"position":{"x":240,"y":0},"configVersion":1,
                 "config":{"language":"javascript","entrypoint":"main","source":"function main({variables:inputs}){console.warn('validating receipt');if(inputs.status<200||inputs.status>=300)throw Error('Cleanup '+inputs.application_name+' HTTP '+inputs.status+': '+JSON.parse(inputs.body).code);return {}}"},
                 "bindings":{"variables":{"kind":"named_bindings","value":[{"name":"status","value":{"kind":"constant","value":409}},{"name":"body","value":{"kind":"constant","value":"{\"code\":\"agent_logs_delete_job_active\"}"}},{"name":"application_name","value":{"kind":"constant","value":"codex"}}]}},"outputs":[]}
            ],
            "edges":[{"id":"edge","source":"start","target":"receipt","sourceHandle":null,"targetHandle":null,"containerId":null,"points":[]}]
        },
        "editor":{"viewport":{"x":0,"y":0,"zoom":1},"annotations":[],"activeContainerPath":[]}
    });
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
    let finished = runtime
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: seeded.workspace_id,
        })
        .await
        .unwrap();
    let expected = "Cleanup codex HTTP 409: agent_logs_delete_job_active";
    assert_eq!(finished.flow_run.status, FlowRunStatus::Failed);
    assert_eq!(
        finished.flow_run.error_payload.as_ref().unwrap()["message"],
        expected
    );
    let node = store
        .get_run_node_last_run(seeded.application_id, finished.flow_run.id, "receipt")
        .await
        .unwrap()
        .unwrap()
        .node_run;
    assert_eq!(node.status, NodeRunStatus::Failed);
    assert_eq!(node.error_payload.unwrap()["message"], expected);
    assert_eq!(node.input_payload["variables"]["application_name"], "codex");
    assert_eq!(node.input_payload["variables"]["status"], 409);
    assert_eq!(
        node.debug_payload["console_logs"][0]["message"],
        "validating receipt"
    );
}
