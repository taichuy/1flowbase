use control_plane::application_public_api::{
    mapping::{
        WorkflowExtensionAccessPolicy, WorkflowExtensionApiConfig, WorkflowExtensionHttpMethod,
        WorkflowExtensionResponseMode,
    },
    published_workflow_operation::{
        validate_published_workflow_contract, workflow_route_shapes_conflict,
        PublishedWorkflowOperationError,
    },
};
use serde_json::json;

fn extension(route: &str) -> WorkflowExtensionApiConfig {
    WorkflowExtensionApiConfig {
        slug: route.into(),
        method: WorkflowExtensionHttpMethod::Post,
        access_policy: WorkflowExtensionAccessPolicy::UserApiKey,
        response_mode: WorkflowExtensionResponseMode::Sync,
    }
}

fn workflow_document(path_field: &str) -> serde_json::Value {
    json!({
        "graph": {
            "nodes": [
                {
                    "id": "node-workflow-start",
                    "type": "workflow_start",
                    "config": {
                        "input_fields": [
                            {
                                "key": path_field,
                                "valueType": "string",
                                "source": "path",
                                "required": true
                            },
                            {
                                "key": "include_history",
                                "valueType": "boolean",
                                "source": "query",
                                "defaultValue": false
                            }
                        ]
                    }
                },
                {
                    "id": "node-workflow-end",
                    "type": "workflow_end",
                    "outputs": [{ "key": "accepted", "valueType": "boolean" }]
                }
            ]
        }
    })
}

#[test]
fn ac_005_route_template_requires_the_same_workflow_start_path_fields() {
    validate_published_workflow_contract(
        &extension("orders/{order_id}"),
        &workflow_document("order_id"),
    )
    .unwrap();

    assert_eq!(
        validate_published_workflow_contract(
            &extension("orders/{ticket_id}"),
            &workflow_document("order_id"),
        )
        .unwrap_err(),
        PublishedWorkflowOperationError::PathFieldsMismatch
    );
}

#[test]
fn ac_006_route_matching_conflicts_are_deterministic_and_fail_closed() {
    assert!(workflow_route_shapes_conflict(
        "orders/{order_id}",
        "orders/{ticket_id}"
    ));
    assert!(!workflow_route_shapes_conflict(
        "orders/{order_id}",
        "tickets/{ticket_id}"
    ));
}
