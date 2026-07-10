use control_plane::application_public_api::{
    workflow_extension::WorkflowExtensionRequestParameters,
    workflow_start_http_inputs::{
        build_workflow_start_node_input_payload, parse_workflow_start_http_inputs,
        WorkflowStartHttpInputError, WorkflowStartHttpInputSource,
    },
};
use serde_json::{json, Map};
use std::collections::BTreeMap;

fn workflow_document(input_fields: serde_json::Value) -> serde_json::Value {
    json!({
        "schemaVersion": "1flowbase.flow/v2",
        "graph": {
            "nodes": [
                {
                    "id": "node-workflow-start",
                    "type": "workflow_start",
                    "config": { "input_fields": input_fields }
                }
            ],
            "edges": []
        }
    })
}

#[test]
fn parses_http_input_contract_from_published_workflow_start() {
    let document = workflow_document(json!([
        {
            "key": "customer_id",
            "valueType": "string",
            "required": true,
            "source": "path"
        },
        {
            "key": "attempts",
            "valueType": "number",
            "required": false,
            "defaultValue": 3,
            "source": "query"
        },
        {
            "key": "enabled",
            "valueType": "boolean",
            "source": "body"
        },
        {
            "key": "note",
            "valueType": "string",
            "source": "form"
        }
    ]));

    let contract = parse_workflow_start_http_inputs(&document).unwrap();

    assert_eq!(contract.start_node_id(), "node-workflow-start");
    assert_eq!(contract.fields().len(), 4);
    assert_eq!(contract.fields()[0].key(), "customer_id");
    assert_eq!(
        contract.fields()[0].source(),
        WorkflowStartHttpInputSource::Path
    );
    assert!(contract.fields()[0].required());
    assert_eq!(contract.fields()[1].default_value(), Some(&json!(3)));
}

#[test]
fn rejects_invalid_source_and_duplicate_key() {
    let invalid_source = workflow_document(json!([{
        "key": "customer_id",
        "valueType": "string",
        "source": "header"
    }]));
    assert_eq!(
        parse_workflow_start_http_inputs(&invalid_source).unwrap_err(),
        WorkflowStartHttpInputError::InvalidSource {
            key: "customer_id".into(),
            invalid_source: "header".into(),
        }
    );

    let duplicate_key = workflow_document(json!([
        { "key": "customer_id", "valueType": "string", "source": "query" },
        { "key": "customer_id", "valueType": "string", "source": "body" }
    ]));
    assert_eq!(
        parse_workflow_start_http_inputs(&duplicate_key).unwrap_err(),
        WorkflowStartHttpInputError::DuplicateKey("customer_id".into())
    );

    let target_selector = workflow_document(json!([{
        "key": "customer_id",
        "valueType": "string",
        "source": "query",
        "target": "node-workflow-start.customer_id"
    }]));
    assert_eq!(
        parse_workflow_start_http_inputs(&target_selector).unwrap_err(),
        WorkflowStartHttpInputError::TargetSelectorNotAllowed("customer_id".into())
    );
}

#[test]
fn builds_node_payload_from_path_query_body_and_form_with_coercion() {
    let document = workflow_document(json!([
        { "key": "customer_id", "valueType": "string", "required": true, "source": "path" },
        { "key": "attempts", "valueType": "number", "required": true, "source": "query" },
        { "key": "enabled", "valueType": "boolean", "required": true, "source": "body" },
        { "key": "note", "valueType": "string", "required": true, "source": "form" }
    ]));
    let contract = parse_workflow_start_http_inputs(&document).unwrap();
    let parameters = WorkflowExtensionRequestParameters {
        path: BTreeMap::from([("customer_id".into(), json!(42))]),
        query: Map::from_iter([("attempts".into(), json!("2.5"))]),
        body: json!({ "enabled": "true" }),
        form: Map::from_iter([("note".into(), json!("urgent"))]),
    };

    let payload = build_workflow_start_node_input_payload(&contract, &parameters).unwrap();

    assert_eq!(
        payload,
        json!({
            "node-workflow-start": {
                "customer_id": "42",
                "attempts": 2.5,
                "enabled": true,
                "note": "urgent"
            }
        })
    );
}

#[test]
fn applies_optional_default_and_omits_optional_without_default() {
    let document = workflow_document(json!([
        { "key": "attempts", "valueType": "number", "defaultValue": 3, "source": "query" },
        { "key": "note", "valueType": "string", "source": "form" },
        { "key": "legacy_default", "valueType": "number", "default": 9, "source": "query" }
    ]));
    let contract = parse_workflow_start_http_inputs(&document).unwrap();

    let payload = build_workflow_start_node_input_payload(
        &contract,
        &WorkflowExtensionRequestParameters::default(),
    )
    .unwrap();

    assert_eq!(payload, json!({ "node-workflow-start": { "attempts": 3 } }));
    assert!(payload["node-workflow-start"]
        .get("legacy_default")
        .is_none());
}

#[test]
fn rejects_missing_required_and_invalid_basic_type() {
    let document = workflow_document(json!([
        { "key": "customer_id", "valueType": "string", "required": true, "source": "query" },
        { "key": "enabled", "valueType": "boolean", "required": true, "source": "body" }
    ]));
    let contract = parse_workflow_start_http_inputs(&document).unwrap();

    assert_eq!(
        build_workflow_start_node_input_payload(
            &contract,
            &WorkflowExtensionRequestParameters::default(),
        )
        .unwrap_err(),
        WorkflowStartHttpInputError::RequiredValueMissing("customer_id".into())
    );

    let parameters = WorkflowExtensionRequestParameters {
        query: Map::from_iter([("customer_id".into(), json!("C-42"))]),
        body: json!({ "enabled": "not-a-boolean" }),
        ..WorkflowExtensionRequestParameters::default()
    };
    assert_eq!(
        build_workflow_start_node_input_payload(&contract, &parameters).unwrap_err(),
        WorkflowStartHttpInputError::InvalidValue {
            key: "enabled".into(),
            value_type: "boolean".into(),
        }
    );
}
