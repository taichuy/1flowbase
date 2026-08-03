use control_plane::application_public_api::{
    mapping::{
        ApplicationApiMappingConfig, ApplicationApiMappingInput, ApplicationApiMappingOutput,
        WorkflowExtensionApiConfig, WorkflowExtensionHttpMethod, WorkflowExtensionResponseMode,
    },
    publications::ApplicationPublicationVersionRecord,
    published_workflow_operation::{
        validate_published_workflow_contract, workflow_route_shapes_conflict,
        PublishedWorkflowOperation, PublishedWorkflowOperationError,
    },
};
use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

fn extension(route: &str) -> WorkflowExtensionApiConfig {
    WorkflowExtensionApiConfig {
        slug: route.into(),
        method: WorkflowExtensionHttpMethod::Post,
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

#[test]
fn workflow_result_schema_preserves_array_and_object_output_types() {
    let document = json!({
        "graph": {
            "nodes": [
                {
                    "id": "node-workflow-start",
                    "type": "workflow_start",
                    "config": { "input_fields": [] }
                },
                {
                    "id": "node-workflow-end",
                    "type": "workflow_end",
                    "outputs": [
                        { "key": "statistics", "valueType": "array" },
                        { "key": "metadata", "valueType": "object" },
                        { "key": "payload", "valueType": "json" }
                    ]
                }
            ]
        }
    });
    let publication = ApplicationPublicationVersionRecord {
        id: Uuid::now_v7(),
        application_id: Uuid::now_v7(),
        workspace_id: Uuid::now_v7(),
        flow_id: Uuid::now_v7(),
        flow_version_id: Uuid::now_v7(),
        mapping_snapshot: ApplicationApiMappingConfig {
            input: ApplicationApiMappingInput {
                query_target: "node-workflow-start".into(),
                model_target: None,
                inputs_target: None,
                history_target: None,
                attachments_target: None,
            },
            output: ApplicationApiMappingOutput::default(),
            extension: Some(extension("task-statistics")),
        },
        extension_slug: Some("task-statistics".into()),
        compiled_plan_id: Uuid::now_v7(),
        version_sequence: 1,
        active: true,
        api_enabled: true,
        flow_schema_version: "1flowbase.flow/v2".into(),
        document_hash: "hash".into(),
        document_snapshot: document,
        runtime_profile_snapshot: json!({}),
        output_selector: json!({}),
        dependency_snapshot: Vec::new(),
        created_by: Uuid::now_v7(),
        created_at: OffsetDateTime::UNIX_EPOCH,
    };

    let operation = PublishedWorkflowOperation::from_publication(publication).unwrap();

    assert_eq!(
        operation.result_schema["properties"]["statistics"],
        json!({ "type": "array" })
    );
    assert_eq!(
        operation.result_schema["properties"]["metadata"],
        json!({ "type": "object" })
    );
    assert_eq!(operation.result_schema["properties"]["payload"], json!({}));
}
