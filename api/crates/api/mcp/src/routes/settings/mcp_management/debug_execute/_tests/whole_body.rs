use super::*;
use axum::{routing::post, Json};
use domain::mcp_management::{McpInterfaceCatalogSource, McpRiskLevel};
use serde_json::json;

// Use the real OpenAPI descriptor extractor: array/record/union roots are exposed
// as `body`, while a genuine object property named `body` keeps its own schema.
fn interface(path: &str, body_schema: Value) -> domain::McpInterfaceCatalogEntry {
    let spec = json!({
        "openapi":"3.1.0", "info":{"title":"Root bodies", "version":"1"},
        "paths":{(path):{"post":{
            "operationId":"whole_body", "requestBody":{"required":true,"content":{
                "application/json":{"schema":body_schema}
            }},
            "responses":{"200":{"description":"Echo","content":{
                "application/json":{"schema":true}
            }}}
        }}}
    });
    let operation = crate::openapi_docs::DocsCatalogOperation {
        id: "whole_body".into(),
        method: "POST".into(),
        path: path.into(),
        summary: None,
        description: None,
        tags: vec![],
        group: "fixture".into(),
        deprecated: false,
    };
    let catalog =
        crate::openapi_interface::catalog_entry_from_operation(&operation, &spec).unwrap();
    domain::McpInterfaceCatalogEntry {
        interface_id: catalog.operation_id,
        source: McpInterfaceCatalogSource::StaticApi,
        method: catalog.method,
        path: catalog.path,
        name: catalog.name,
        short_description: catalog.description,
        parameter_descriptors: catalog
            .parameter_descriptors
            .into_iter()
            .map(|descriptor| {
                assert_eq!(
                    descriptor.location,
                    crate::openapi_interface::OpenApiParameterLocation::JsonBody
                );
                McpParameterDescriptor {
                    name: descriptor.name,
                    field_type: descriptor.field_type,
                    parameter_type: McpParameterType::JsonBody,
                    description: descriptor.description,
                    required: descriptor.required,
                    schema: descriptor.schema,
                }
            })
            .collect(),
        parameter_schema: catalog.request_schema,
        result_schema: catalog.response_schema,
        permission_code: None,
        security: json!({}),
        risk_level: McpRiskLevel::Low,
        bindable: true,
        disabled_reason: None,
    }
}

fn mapping() -> Value {
    json!({"mappings":[{
        "interface_param":"body", "source":{"kind":"mcp_argument","path":"body"},
        "required":true
    }]})
}

async fn assert_http_body(path: &str, schema: Value, supplied: Value, expected: Value) {
    for use_port in [false, true] {
        let expected_http = expected.clone();
        let router = Router::new().route(
            path,
            post(move |Json(body): Json<Value>| {
                let expected = expected_http.clone();
                async move {
                    assert_eq!(
                        body, expected,
                        "HTTP root body must match the supplied value"
                    );
                    Json(json!({"data":{"received":body}}))
                }
            }),
        );
        let entry = interface(path, schema.clone());
        let call = McpDebugExecuteBody {
            interface_id: entry.interface_id.clone(),
            debug_response_mode: McpDebugResponseMode::DebugDetails,
            mcp_arguments: json!({"body":supplied}),
            input_mapping: mapping(),
            output_mapping: json!({}),
        };
        let bound = McpServerBoundInputs {
            workspace_id: Uuid::nil(),
        };
        let result = if use_port {
            let dispatcher =
                crate::openapi_interface::console_router_callable_dispatch_port(router);
            execute_with_dispatch_port(
                dispatcher.as_ref(),
                Default::default(),
                entry,
                call,
                bound,
                None,
            )
            .await
            .map_err(|_| ())
        } else {
            execute_with_console_router(router, HeaderMap::new(), entry, call, bound)
                .await
                .map_err(|_| ())
        }
        .expect("whole body must pass real request validation and HTTP dispatch");
        assert_eq!(result["interface_arguments"]["body"], expected);
        assert_eq!(result["tool_result"]["received"], expected);
    }
}

#[tokio::test]
async fn whole_body_batch_array_reaches_both_http_dispatch_adapters() {
    let schema = json!({"type":"array", "items":{
        "type":"object", "properties":{"method":{"type":"string"}, "path":{"type":"string"}},
        "required":["method","path"], "additionalProperties":false
    }});
    let batch = json!([{"method":"GET","path":"/api/me"}]);
    // Controlled old behavior: wrapping the array under a body property is invalid.
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(validator.is_valid(&batch));
    assert!(!validator.is_valid(&json!({"body":batch})));
    assert_http_body("/api/batch", schema, batch.clone(), batch).await;
}

#[tokio::test]
async fn whole_body_send_union_and_record_and_all_of_keep_their_http_root() {
    let event = json!({"type":"object","properties":{
        "type":{"const":"event"}, "payload":{"type":"object", "properties":{"name":{"type":"string"}},"required":["name"]}
    },"required":["type","payload"],"additionalProperties":false});
    let identify = json!({"type":"object","properties":{
        "type":{"const":"identify"}, "payload":{"type":"object", "properties":{"id":{"type":"string"}},"required":["id"]}
    },"required":["type","payload"],"additionalProperties":false});
    for composition in ["oneOf", "anyOf"] {
        let schema = json!({(composition):[event,identify]});
        let payload = json!({"type":"event", "payload":{"name":"purchase"}});
        assert!(!jsonschema::validator_for(&schema)
            .unwrap()
            .is_valid(&json!({"body":payload})));
        assert_http_body("/api/send", schema, payload.clone(), payload).await;
    }
    let record = json!({"domain":"example.test", "language":"en"});
    assert_http_body(
        "/api/record",
        json!({"type":"object", "additionalProperties":{"type":"string"}}),
        record.clone(),
        record,
    )
    .await;
    let report = json!({"websiteId":"fixture", "timezone":"UTC"});
    assert_http_body("/api/reports", json!({"allOf":[
        {"type":"object", "properties":{"websiteId":{"type":"string"}}, "required":["websiteId"]},
        {"type":"object", "properties":{"timezone":{"type":"string"}}, "required":["timezone"]}
    ]}), report.clone(), report).await;
}

#[tokio::test]
async fn whole_body_marker_does_not_replace_an_actual_body_property() {
    let schema = json!({"type":"object", "properties":{"body":{"type":"string"}}, "required":["body"], "additionalProperties":false});
    assert_http_body(
        "/api/actual-body-field",
        schema,
        json!("content"),
        json!({"body":"content"}),
    )
    .await;
    let schema = json!({"allOf":[{"type":"object", "properties":{"body":{"type":"string"}}, "required":["body"]}]});
    assert_http_body(
        "/api/composed-body-field",
        schema,
        json!("content"),
        json!({"body":"content"}),
    )
    .await;
}

#[test]
fn whole_body_rejects_ambiguous_child_mappings_in_either_order() {
    let entry = interface(
        "/api/composed",
        json!({"allOf":[{
            "type":"object", "properties":{"name":{"type":"string"}}, "required":["name"]
        }]}),
    );
    let root = mapping()["mappings"][0].clone();
    let child = json!({"interface_param":"name", "source":{"kind":"mcp_argument","path":"name"}});
    for mappings in [json!([root, child]), json!([child, root])] {
        let result = build_interface_arguments(
            &entry,
            &json!({"mappings":mappings}),
            &json!({"body":{"name":"root"},"name":"child"}),
            McpServerBoundInputs {
                workspace_id: Uuid::nil(),
            },
        );
        assert!(
            result.is_err(),
            "whole body and child mappings must not override one another"
        );
    }
}

#[test]
fn whole_body_mapper_preserves_explicit_null_separately_from_missing() {
    let entry = interface("/api/nullable", json!({"type":["object","null"]}));
    let supplied = build_interface_arguments(
        &entry,
        &mapping(),
        &json!({"body":null}),
        McpServerBoundInputs {
            workspace_id: Uuid::nil(),
        },
    )
    .unwrap();
    assert_eq!(supplied.whole_body, Some(Value::Null));
    assert_eq!(
        supplied.to_value(&entry.parameter_schema).get("body"),
        Some(&Value::Null)
    );
    let optional = json!({"mappings":[{"interface_param":"body", "mcp_param":"body"}]});
    let missing = build_interface_arguments(
        &entry,
        &optional,
        &json!({}),
        McpServerBoundInputs {
            workspace_id: Uuid::nil(),
        },
    )
    .unwrap();
    assert!(missing.whole_body.is_none());
    assert!(missing
        .to_value(&entry.parameter_schema)
        .get("body")
        .is_none());
}
