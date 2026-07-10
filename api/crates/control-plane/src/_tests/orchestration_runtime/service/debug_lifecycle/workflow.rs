use super::*;

fn workflow_document(flow_id: Uuid) -> Value {
    json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": {
            "flowId": flow_id.to_string(),
            "name": "Ticket Workflow",
            "description": "",
            "tags": []
        },
        "graph": {
            "nodes": [
                {
                    "id": "node-workflow-start",
                    "type": "workflow_start",
                    "alias": "Workflow Start",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 0, "y": 0 },
                    "configVersion": 1,
                    "config": {
                        "input_fields": [
                            {
                                "key": "customer_id",
                                "label": "Customer ID",
                                "inputType": "text",
                                "valueType": "string",
                                "required": true
                            }
                        ],
                        "sync_timeout_ms": 30000
                    },
                    "bindings": {},
                    "outputs": []
                },
                {
                    "id": "node-transform",
                    "type": "template_transform",
                    "alias": "Template Transform",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 240, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "template": {
                            "kind": "templated_text",
                            "value": "ticket-{{ node-workflow-start.customer_id }}"
                        }
                    },
                    "outputs": [
                        { "key": "ticket_id", "title": "Ticket ID", "valueType": "string" }
                    ]
                },
                {
                    "id": "node-workflow-end",
                    "type": "workflow_end",
                    "alias": "Workflow End",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 480, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "ticket_id": {
                            "kind": "selector",
                            "value": ["node-transform", "ticket_id"]
                        }
                    },
                    "outputs": [
                        { "key": "ticket_id", "title": "Ticket ID", "valueType": "string" }
                    ]
                }
            ],
            "edges": [
                {
                    "id": "edge-start-transform",
                    "source": "node-workflow-start",
                    "target": "node-transform",
                    "sourceHandle": null,
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                },
                {
                    "id": "edge-transform-end",
                    "source": "node-transform",
                    "target": "node-workflow-end",
                    "sourceHandle": null,
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                }
            ]
        },
        "editor": {
            "viewport": { "x": 0, "y": 0, "zoom": 1 },
            "annotations": [],
            "activeContainerPath": []
        }
    })
}

#[tokio::test]
async fn workflow_debug_run_compiles_workflow_document_by_application_type() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_workflow_application_with_flow("Ticket Workflow")
        .await;

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({
                "node-workflow-start": { "customer_id": "C-42" }
            }),
            document_snapshot: Some(workflow_document(seeded.flow_id)),
            debug_session_id: None,
        })
        .await
        .expect("AC-101 workflow draft should compile through the workflow compiler");

    assert_eq!(started.flow_run.status, domain::FlowRunStatus::Running);
}

#[tokio::test]
async fn workflow_debug_run_persists_workflow_end_projection_as_flow_output() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_workflow_application_with_flow("Ticket Workflow")
        .await;
    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({
                "node-workflow-start": { "customer_id": "C-42" }
            }),
            document_snapshot: Some(workflow_document(seeded.flow_id)),
            debug_session_id: None,
        })
        .await
        .expect("AC-101 workflow draft should compile through the workflow compiler");

    let completed = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .expect("AC-104 workflow debug run should complete");

    assert_eq!(completed.flow_run.status, domain::FlowRunStatus::Succeeded);
    assert_eq!(
        completed.flow_run.output_payload,
        json!({ "ticket_id": "ticket-C-42" })
    );
    assert_eq!(
        node_run(&completed, "node-workflow-end").output_payload,
        json!({ "ticket_id": "ticket-C-42" })
    );
}
