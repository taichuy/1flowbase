use super::*;
use control_plane_contracts::ports::{
    CommitContainerTransitionInput, ContainerControlFailureFact, ContainerNodeFact,
    ContainerRoundFact, ProviderTrajectoryRepository, WorkflowTrajectoryQuery,
};

#[tokio::test]
async fn container_trajectory_preserves_round_facts_identity_and_application_scope() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let run = seed_flow_run(&store, &seeded, &compiled, OffsetDateTime::now_utc()).await;
    let frame = Uuid::now_v7();
    let frame_input = CommitContainerTransitionInput {
        flow_run_id: run.id,
        resume_claim_owner: None,
        control_failure: None,
        execution_identity: domain::NodeExecutionIdentity {
            execution_id: frame,
            node_id: "loop".into(),
            parent_execution_id: None,
            round_path: vec![],
        },
        checkpoint: json!({"version":1}),
        node: Some(ContainerNodeFact::Started {
            node_type: "loop".into(),
            node_alias: "Outer".into(),
            input_payload: json!({}),
        }),
        rounds: vec![],
    };
    let frame_record = store
        .commit_container_transition(&frame_input)
        .await
        .unwrap()
        .unwrap();
    let round_id = Uuid::now_v7();
    let child_id = Uuid::now_v7();
    let mut child = CommitContainerTransitionInput {
        flow_run_id: run.id,
        resume_claim_owner: None,
        control_failure: None,
        execution_identity: domain::NodeExecutionIdentity {
            execution_id: child_id,
            node_id: "leaf".into(),
            parent_execution_id: Some(frame),
            round_path: vec![domain::ContainerRoundPath {
                container_execution_id: frame,
                container_node_id: "loop".into(),
                round_index: 0,
            }],
        },
        checkpoint: json!({"started_rounds":1}),
        node: Some(ContainerNodeFact::Started {
            node_type: "template_transform".into(),
            node_alias: "Item".into(),
            input_payload: json!({}),
        }),
        rounds: vec![ContainerRoundFact {
            execution_id: round_id,
            container_execution_id: frame,
            round_index: 0,
            deadline_unix_ms: 2000000000000,
            status: "running".into(),
            error_payload: None,
            output_payload: None,
            variables: None,
        }],
    };
    let child_record = store
        .commit_container_transition(&child)
        .await
        .unwrap()
        .unwrap();
    child.node = Some(ContainerNodeFact::Updated {
        status: NodeRunStatus::Succeeded,
        output_payload: json!({"text":"value"}),
        error_payload: None,
        metrics_payload: json!({}),
        debug_payload: json!({}),
        terminal: true,
    });
    child.rounds[0].status = "succeeded".into();
    child.rounds[0].output_payload = Some(json!("value"));
    child.rounds[0].variables = Some(json!({"total":1}));
    store.commit_container_transition(&child).await.unwrap();
    let mut frame_terminal = frame_input.clone();
    frame_terminal.node = Some(ContainerNodeFact::Updated {
        status: NodeRunStatus::Succeeded,
        output_payload: json!({"result":["value"],"variables":{"total":1}}),
        error_payload: None,
        metrics_payload: json!({}),
        debug_payload: json!({}),
        terminal: true,
    });
    frame_terminal.control_failure = Some(ContainerControlFailureFact {
        after_round_index: 0,
        error_payload: json!({"code":"condition_value_invalid"}),
    });
    store
        .commit_container_transition(&frame_terminal)
        .await
        .unwrap();
    let page = store
        .workflow_trajectory_page(
            seeded.application_id,
            run.id,
            WorkflowTrajectoryQuery::default(),
        )
        .await
        .unwrap();
    let leaf = page
        .items
        .iter()
        .find(|event| {
            event.event_type == "node_finished" && event.node_run_id == Some(child_record.id)
        })
        .unwrap();
    assert_eq!(
        leaf.execution_identity.as_ref(),
        Some(&child.execution_identity)
    );
    let finished = page
        .items
        .iter()
        .find(|event| event.event_type == "round_finished")
        .unwrap();
    assert_eq!(finished.node_run_id, Some(frame_record.id));
    assert_eq!(finished.category, "rounds");
    let identity = finished.execution_identity.as_ref().unwrap();
    assert_eq!(identity.execution_id, round_id);
    assert_eq!(identity.round_path[0].round_index, 0);
    let body = store
        .workflow_trajectory_body(seeded.application_id, run.id, &finished.event_id)
        .await
        .unwrap()
        .unwrap();
    assert!(body.sections.iter().any(|section| section.kind == "output"
        && section.value == json!({"result":"value","variables":{"total":1}})));
    let index = store
        .workflow_trajectory_page(
            seeded.application_id,
            run.id,
            WorkflowTrajectoryQuery {
                index_only: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(index
        .overview
        .unwrap()
        .items
        .iter()
        .any(|event| event.event_id == finished.event_id
            && event.execution_identity.as_ref() == Some(identity)));
    assert!(store
        .workflow_trajectory_body(Uuid::now_v7(), run.id, &finished.event_id)
        .await
        .unwrap()
        .is_none());
    let started = page
        .items
        .iter()
        .find(|event| event.event_type == "round_started")
        .unwrap();
    let cursor =
        serde_json::to_string(&(&started.created_at, run.id, 0, 0, &started.event_id)).unwrap();
    let next = store
        .workflow_trajectory_page(
            seeded.application_id,
            run.id,
            WorkflowTrajectoryQuery {
                cursor: Some(cursor),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(next
        .items
        .iter()
        .any(|event| event.event_id == finished.event_id));
    assert!(next
        .items
        .iter()
        .all(|event| event.event_id != started.event_id));
    let frame_body = store
        .workflow_trajectory_body(
            seeded.application_id,
            run.id,
            &format!("node_finished:{}", frame_record.id),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(frame_body
        .sections
        .iter()
        .any(|section| section.kind == "error"
            && section.value["error_payload"]["code"] == "condition_value_invalid"));
}
