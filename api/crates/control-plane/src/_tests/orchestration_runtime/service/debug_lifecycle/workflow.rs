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
                            "value": "{{ trigger.type }}-ticket-{{ node-workflow-start.customer_id }}"
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
        json!({ "ticket_id": "extension-ticket-C-42" })
    );
    assert_eq!(
        node_run(&completed, "node-workflow-end").output_payload,
        json!({ "ticket_id": "extension-ticket-C-42" })
    );
    assert_eq!(
        node_run(&completed, "node-workflow-start").input_payload["sys"],
        json!({
            "application_id": seeded.application_id.to_string(),
            "workflow_id": seeded.flow_id.to_string(),
            "workflow_run_id": completed.flow_run.id.to_string()
        })
    );
    assert_eq!(
        node_run(&completed, "node-workflow-start").input_payload["trigger"],
        json!({ "type": "extension" })
    );
}

#[tokio::test]
async fn workflow_node_preview_compiles_and_materializes_start_globals() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_workflow_application_with_flow("Preview Workflow")
        .await;
    let preview = service
        .start_node_debug_preview(StartNodeDebugPreviewCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            node_id: "node-workflow-start".to_string(),
            input_payload: json!({
                "node-workflow-start": {"customer_id": "C-42"},
                "sys": {"workflow_run_id": "untrusted"}, "trigger": {"type": "untrusted"},
                "env": {"untrusted": "discard"}
            }),
            document_snapshot: Some(workflow_document(seeded.flow_id)),
            debug_session_id: None,
        })
        .await
        .expect("Workflow single node preview should use the workflow compiler");
    assert_eq!(preview.flow_run.status, domain::FlowRunStatus::Succeeded);
    assert_eq!(preview.node_run.input_payload["customer_id"], "C-42");
    assert_eq!(
        preview.node_run.input_payload["sys"],
        json!({
            "application_id": seeded.application_id.to_string(),
            "workflow_id": seeded.flow_id.to_string(),
            "workflow_run_id": preview.flow_run.id.to_string(),
        })
    );
    assert_eq!(
        preview.node_run.input_payload["trigger"],
        json!({"type": "extension"})
    );
    assert_eq!(preview.node_run.input_payload["env"], json!({}));
}

#[tokio::test]
async fn workflow_template_and_end_preview_use_supplied_upstream_outputs() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_workflow_application_with_flow("Preview Workflow")
        .await;
    for (node_id, input, expected) in [
        (
            "node-transform",
            json!({"node-workflow-start": {"customer_id": "C-42"}}),
            json!({"ticket_id": "extension-ticket-C-42"}),
        ),
        (
            "node-workflow-end",
            json!({"node-transform": {"ticket_id": "supplied-ticket"}}),
            json!({"ticket_id": "supplied-ticket"}),
        ),
    ] {
        let preview = service
            .start_node_debug_preview(StartNodeDebugPreviewCommand {
                actor_user_id: seeded.actor_user_id,
                application_id: seeded.application_id,
                node_id: node_id.to_string(),
                input_payload: input,
                document_snapshot: Some(workflow_document(seeded.flow_id)),
                debug_session_id: None,
            })
            .await
            .unwrap();
        assert_eq!(preview.flow_run.status, domain::FlowRunStatus::Succeeded);
        assert_eq!(preview.node_run.output_payload, expected);
        // The supplied End input must work even though its upstream Start input is absent.
        // This proves preview does not replay the template or Start.
        assert!(!preview.node_run.input_payload.is_null());
    }
}

fn workflow_data_model_document(
    flow_id: Uuid,
    action: &str,
    config: Value,
    bindings: Value,
) -> Value {
    let mut document = workflow_document(flow_id);
    document["graph"]["nodes"][0]["config"]["input_fields"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "key": "record_id", "label": "Record ID", "inputType": "text",
            "valueType": "string", "required": false
        }));
    document["graph"]["nodes"][1] = data_model_node("node-transform", action, config, bindings);
    document["graph"]["nodes"][2]["bindings"]["ticket_id"] = json!({
        "kind": "selector", "value": ["node-workflow-start", "customer_id"]
    });
    document
}

#[tokio::test]
async fn workflow_data_model_preview_creates_reads_updates_and_deletes_real_records() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_workflow_application_with_flow("Data Preview Workflow")
        .await;
    let mut record_id = Value::Null;
    let mut deleted = false;
    for (action, config) in [
        (
            "create",
            json!({"payload": {"title": "Preview Order", "status": "draft"}}),
        ),
        ("get", json!({})),
        ("list", json!({})),
        ("update", json!({"payload": {"status": "paid"}})),
        ("delete", json!({})),
        ("list", json!({})),
    ] {
        let bindings = if matches!(action, "get" | "update" | "delete") {
            json!({"record_id": {"kind": "selector", "value": ["node-workflow-start", "record_id"]}})
        } else {
            json!({})
        };
        let preview = service
            .start_node_debug_preview(StartNodeDebugPreviewCommand {
                actor_user_id: seeded.actor_user_id,
                application_id: seeded.application_id,
                node_id: "node-transform".to_string(),
                input_payload: json!({"node-workflow-start": {"record_id": record_id}}),
                document_snapshot: Some(workflow_data_model_document(
                    seeded.flow_id,
                    action,
                    config,
                    bindings,
                )),
                debug_session_id: None,
            })
            .await
            .expect("data model preview must have an active persisted node and authorized context");
        assert_eq!(
            preview.flow_run.status,
            domain::FlowRunStatus::Succeeded,
            "action {action}: {:?}",
            preview.node_run.error_payload
        );
        let output = &preview.node_run.output_payload;
        match action {
            "create" => {
                record_id = output["record"]["id"].clone();
                assert!(record_id.is_string());
            }
            "get" => assert_eq!(output["record"]["title"], "Preview Order"),
            "list" if !deleted => {
                assert_eq!(output["total"], 1);
                assert_eq!(output["records"][0]["id"], record_id);
            }
            "list" => assert_eq!(output["total"], 0),
            "update" => assert_eq!(output["record"]["status"], "paid"),
            "delete" => {
                assert_eq!(output["deleted_id"], record_id);
                deleted = true;
            }
            _ => unreachable!(),
        }
    }
}

#[tokio::test]
async fn workflow_data_model_preview_keeps_confirmation_and_disabled_writes_non_mutating() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_workflow_application_with_flow("Policy Preview Workflow")
        .await;
    for policy in ["confirm_each_run", "disabled"] {
        let preview = service.start_node_debug_preview(StartNodeDebugPreviewCommand {
            actor_user_id: seeded.actor_user_id, application_id: seeded.application_id,
            node_id: "node-transform".to_string(), input_payload: json!({}),
            document_snapshot: Some(workflow_data_model_document(seeded.flow_id, "create", json!({
                "payload": {"title": "Do not create", "status": "draft"}, "side_effect_policy": policy,
            }), json!({}))), debug_session_id: None,
        }).await.unwrap();
        assert_eq!(preview.flow_run.status, domain::FlowRunStatus::Failed);
        assert_eq!(preview.node_run.status, domain::NodeRunStatus::Failed);
        if policy == "confirm_each_run" {
            assert_eq!(
                preview.node_run.error_payload.as_ref().unwrap()["error_code"],
                "node_preview_callback_not_supported"
            );
        } else {
            assert!(preview.node_run.error_payload.as_ref().unwrap()["message"]
                .as_str()
                .unwrap()
                .contains("DATA_MODEL_SIDE_EFFECT_DISABLED"));
        }
    }
    let list = service
        .start_node_debug_preview(StartNodeDebugPreviewCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            node_id: "node-transform".to_string(),
            input_payload: json!({}),
            document_snapshot: Some(workflow_data_model_document(
                seeded.flow_id,
                "list",
                json!({}),
                json!({}),
            )),
            debug_session_id: None,
        })
        .await
        .unwrap();
    assert_eq!(list.node_run.output_payload["total"], 0);
}

#[tokio::test]
async fn workflow_data_model_preview_preserves_scope_denial() {
    let service = OrchestrationRuntimeService::for_tests_without_data_model_scope_grant();
    let seeded = service
        .seed_workflow_application_with_flow("Denied Preview Workflow")
        .await;
    let preview = service
        .start_node_debug_preview(StartNodeDebugPreviewCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            node_id: "node-transform".to_string(),
            input_payload: json!({}),
            document_snapshot: Some(workflow_data_model_document(
                seeded.flow_id,
                "list",
                json!({}),
                json!({}),
            )),
            debug_session_id: None,
        })
        .await
        .unwrap();
    assert_eq!(preview.flow_run.status, domain::FlowRunStatus::Failed);
    assert_eq!(preview.node_run.status, domain::NodeRunStatus::Failed);
    assert!(preview.node_run.error_payload.as_ref().unwrap()["message"]
        .as_str()
        .unwrap()
        .contains("permission denied"));
}
