use super::*;
use control_plane_contracts::ports::{
    InvalidWorkflowTrajectoryQuery, WorkflowEventCategory, WorkflowTrajectoryQuery,
};

async fn clone_run(store: &PgControlPlaneStore, original: Uuid, application: Uuid) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("insert into flow_runs(id,scope_id,application_id,flow_id,flow_draft_id,compiled_plan_id,run_mode,status,created_by) select $1,scope_id,$3,flow_id,flow_draft_id,compiled_plan_id,run_mode,'running',created_by from flow_runs where id=$2")
        .bind(id).bind(original).bind(application).execute(store.pool()).await.unwrap();
    id
}
async fn node(store: &PgControlPlaneStore, flow: Uuid, alias: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("insert into node_runs(id,scope_id,flow_run_id,node_id,node_type,node_alias,status,started_at,finished_at,input_payload,output_payload) select $1,scope_id,id,$1::text,'llm',$3,'succeeded','2026-09-22T00:00:00Z','2026-09-22T00:01:00Z','{\"input\":\"selected only\"}','{\"output\":\"selected only\"}' from flow_runs where id=$2")
        .bind(id).bind(flow).bind(alias).execute(store.pool()).await.unwrap();
    id
}
async fn task(store: &PgControlPlaneStore, flow: Uuid, parent: Option<Uuid>) {
    sqlx::query("insert into application_run_log_summaries(flow_run_id,application_id,run_mode,status,title,input_payload,started_at,created_at,updated_at,scope_id) select id,application_id,run_mode,status,title,input_payload,started_at,created_at,updated_at,scope_id from flow_runs where id=$1")
        .bind(flow).execute(store.pool()).await.unwrap();
    sqlx::query("select application_run_log_task_refresh($1)")
        .bind(flow)
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query("update application_run_log_tasks set parent_task_run_id=$2 where id=$1")
        .bind(flow)
        .bind(parent)
        .execute(store.pool())
        .await
        .unwrap();
}
async fn app(store: &PgControlPlaneStore, flow: Uuid) -> Uuid {
    sqlx::query_scalar("select application_id from flow_runs where id=$1")
        .bind(flow)
        .fetch_one(store.pool())
        .await
        .unwrap()
}

#[tokio::test]
async fn workflow_trajectory_scope_filters_keyset_and_selected_bodies() {
    let (pool, root) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool);
    let application = app(&store, root).await;
    let root_node = node(&store, root, "Original node snapshot").await;
    task(&store, root, None).await;
    let round = clone_run(&store, root, application).await;
    task(&store, round, None).await;
    sqlx::query("update application_run_log_summaries set log_task_run_id=$2,call_kind='compact' where flow_run_id=$1").bind(round).bind(root).execute(store.pool()).await.unwrap();
    sqlx::query("update application_run_log_tasks set member_run_ids=array[$1,$2] where id=$1")
        .bind(root)
        .bind(round)
        .execute(store.pool())
        .await
        .unwrap();
    let round_node = node(&store, round, "Compactor").await;
    let child = clone_run(&store, root, application).await;
    task(&store, child, Some(root)).await;
    let child_round = clone_run(&store, root, application).await;
    task(&store, child_round, None).await;
    sqlx::query("update application_run_log_tasks set member_run_ids=array[$1,$2] where id=$1")
        .bind(child)
        .bind(child_round)
        .execute(store.pool())
        .await
        .unwrap();
    let child_node = node(&store, child_round, "Child continuation").await;
    let grandchild = clone_run(&store, root, application).await;
    task(&store, grandchild, Some(child)).await;
    let grandchild_node = node(&store, grandchild, "Grandchild").await;
    // Equal wall-clock and local sequences cannot collapse events from distinct sources or runs.
    let request = Uuid::now_v7();
    let native=super::native::append(&store,root,root_node,"provider_semantic_step",json!({"source":"ai_native","invocation_id":"unified","provider_attempt_index":0,"step_key":"call","kind":"model_call","trigger_request_id":request,"body":"private body"})).await;
    let tool = super::native::append(
        &store,
        root,
        root_node,
        "assistant_tool_call_finished",
        json!({"result":"selected tool result"}),
    )
    .await;
    super::native::append(
        &store,
        root,
        root_node,
        "text_delta",
        json!({"text":"must not appear"}),
    )
    .await;
    sqlx::query("update runtime_events set created_at='2026-09-22T00:00:00Z' where id=$1")
        .bind(tool)
        .execute(store.pool())
        .await
        .unwrap();
    let earlier_tool = super::native::append(
        &store,
        root,
        root_node,
        "assistant_tool_call_started",
        json!({"name":"read"}),
    )
    .await;
    sqlx::query("update runtime_events set sequence=100 where id=$1")
        .bind(tool)
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query(
        "update runtime_events set sequence=99,created_at='2026-09-22T00:00:00Z' where id=$1",
    )
    .bind(earlier_tool)
    .execute(store.pool())
    .await
    .unwrap();
    let all = store
        .workflow_trajectory_page(application, root, WorkflowTrajectoryQuery::default())
        .await
        .unwrap();
    assert_eq!(all.nodes.len(), 4);
    assert_eq!(all.items.len(), 11);
    let tool_positions = all
        .items
        .iter()
        .filter(|e| e.category == "tools")
        .map(|e| e.event_id.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        tool_positions,
        vec![format!("runtime:{earlier_tool}"), format!("runtime:{tool}")]
    );
    assert!(all
        .items
        .iter()
        .any(|e| e.event_id == format!("native:{native}")));
    assert!(all.items.iter().all(|e| e.event_type != "text_delta"));
    assert_eq!(
        all.items
            .iter()
            .find(|e| e.node_run_id == Some(root_node))
            .unwrap()
            .node_alias
            .as_deref(),
        Some("Original node snapshot")
    );
    assert!(
        all.items
            .iter()
            .find(|e| e.node_run_id == Some(grandchild_node))
            .unwrap()
            .parent_task_run_id
            == Some(child)
    );
    let mut cursor = None;
    let mut seen = vec![];
    loop {
        let page = store
            .workflow_trajectory_page(
                application,
                root,
                WorkflowTrajectoryQuery {
                    cursor,
                    limit: Some(1),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        seen.extend(page.items.into_iter().map(|e| e.event_id));
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(
        seen,
        all.items
            .iter()
            .map(|e| e.event_id.clone())
            .collect::<Vec<_>>()
    );
    let agents = store
        .workflow_trajectory_page(
            application,
            root,
            WorkflowTrajectoryQuery {
                category: WorkflowEventCategory::Agents,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(agents.items.len(), 4);
    assert!(agents
        .items
        .iter()
        .all(|e| [child_node, grandchild_node].contains(&e.node_run_id.unwrap())));
    let rounds = store
        .workflow_trajectory_page(
            application,
            root,
            WorkflowTrajectoryQuery {
                category: WorkflowEventCategory::Rounds,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(rounds.items.len(), 4);
    assert!(rounds
        .items
        .iter()
        .all(|e| [round_node, child_node].contains(&e.node_run_id.unwrap())));
    let requests = store
        .workflow_trajectory_page(
            application,
            root,
            WorkflowTrajectoryQuery {
                request_id: Some(request),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(requests.items.len(), 1);
    assert_eq!(requests.nodes.len(), all.nodes.len());
    assert_eq!(requests.time_start, all.time_start);
    assert!(requests.items[0]
        .native_step
        .as_ref()
        .unwrap()
        .metadata
        .get("body")
        .is_none());
    let filtered = store
        .workflow_trajectory_page(
            application,
            root,
            WorkflowTrajectoryQuery {
                node_run_id: Some(root_node),
                from: Some("2026-09-22T00:00:30Z".into()),
                to: Some("2026-09-22T00:01:00Z".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(filtered.items.len(), 1);
    assert_eq!(filtered.items[0].event_type, "node_finished");
    let body = store
        .workflow_trajectory_body(application, root, &format!("node_finished:{child_node}"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        body.sections
            .iter()
            .find(|s| s.kind == "output")
            .unwrap()
            .value,
        json!({"output":"selected only"})
    );
    sqlx::query("update node_runs set raw_json_payloads=jsonb_build_object('output_payload','invalid JSON','error_payload','invalid JSON') where id=$1").bind(child_node).execute(store.pool()).await.unwrap();
    let started_body = store
        .workflow_trajectory_body(application, root, &format!("node_started:{child_node}"))
        .await
        .unwrap()
        .unwrap();
    assert!(started_body.sections.iter().any(|s| s.kind == "input"));
    assert!(!started_body.sections.iter().any(|s| s.kind == "output"));
    let body = store
        .workflow_trajectory_body(application, root, &format!("runtime:{tool}"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        body.sections[0].value,
        json!({"result":"selected tool result"})
    );
    assert!(store
        .workflow_trajectory_body(application, root, &format!("native:{native}"))
        .await
        .unwrap()
        .is_none());
    assert!(store
        .workflow_trajectory_body(Uuid::now_v7(), root, &format!("node_finished:{root_node}"))
        .await
        .unwrap()
        .is_none());
    for query in [
        WorkflowTrajectoryQuery {
            cursor: Some("bogus".into()),
            ..Default::default()
        },
        WorkflowTrajectoryQuery {
            from: Some("bad".into()),
            ..Default::default()
        },
        WorkflowTrajectoryQuery {
            from: Some("2026-09-23T00:00:00Z".into()),
            to: Some("2026-09-22T00:00:00Z".into()),
            ..Default::default()
        },
    ] {
        assert!(store
            .workflow_trajectory_page(application, root, query)
            .await
            .unwrap_err()
            .downcast_ref::<InvalidWorkflowTrajectoryQuery>()
            .is_some());
    }
}

#[tokio::test]
async fn workflow_trajectory_scope_rejects_foreign_members_failed_imports_and_cycles() {
    let (pool, root) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool);
    let application = app(&store, root).await;
    let root_node = node(&store, root, "root").await;
    task(&store, root, None).await;
    let foreign_application = Uuid::now_v7();
    sqlx::query("insert into applications(id,workspace_id,application_type,name,description,created_by,updated_by) select $1,workspace_id,application_type,'foreign','',created_by,updated_by from applications where id=$2").bind(foreign_application).bind(application).execute(store.pool()).await.unwrap();
    let foreign = clone_run(&store, root, foreign_application).await;
    let foreign_node = node(&store, foreign, "foreign").await;
    task(&store, foreign, Some(root)).await;
    let hidden = clone_run(&store, root, application).await;
    task(&store, hidden, Some(root)).await;
    let hidden_node = node(&store, hidden, "failed import").await;
    let upload = Uuid::now_v7();
    let job = Uuid::now_v7();
    sqlx::query("insert into run_archive_upload_sessions(id,scope_id,application_id,actor_user_id,created_by,updated_by,total_size_bytes,expected_sha256,chunk_size_bytes,status) select $1,scope_id,application_id,created_by,created_by,created_by,1,'fixture',1,'completed' from flow_runs where id=$2").bind(upload).bind(root).execute(store.pool()).await.unwrap();
    sqlx::query("insert into run_archive_import_jobs(id,scope_id,application_id,actor_user_id,created_by,updated_by,upload_session_id,status) select $1,scope_id,application_id,created_by,created_by,created_by,$3,'failed' from flow_runs where id=$2").bind(job).bind(root).bind(upload).execute(store.pool()).await.unwrap();
    sqlx::query("update flow_runs set import_job_id=$2 where id=$1")
        .bind(hidden)
        .bind(job)
        .execute(store.pool())
        .await
        .unwrap();
    // Malformed projected membership and a relation cycle must not broaden or hang the read.
    sqlx::query("update application_run_log_tasks set member_run_ids=array[$1,$2,$3],parent_task_run_id=$1 where id=$1").bind(root).bind(foreign).bind(hidden).execute(store.pool()).await.unwrap();
    let page = store
        .workflow_trajectory_page(application, root, WorkflowTrajectoryQuery::default())
        .await
        .unwrap();
    assert_eq!(page.nodes.len(), 1);
    assert_eq!(page.nodes[0].node_run_id, root_node);
    assert_eq!(page.items.len(), 2);
    for id in [foreign_node, hidden_node] {
        assert!(store
            .workflow_trajectory_body(application, root, &format!("node_finished:{id}"))
            .await
            .unwrap()
            .is_none());
    }
    assert!(store
        .workflow_trajectory_page(application, hidden, WorkflowTrajectoryQuery::default())
        .await
        .unwrap()
        .items
        .is_empty());
}
