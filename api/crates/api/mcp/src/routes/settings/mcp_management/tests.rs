use super::*;
use serde_json::json;

fn operation(id: &str, method: &str, path: &str) -> DocsCatalogOperation {
    DocsCatalogOperation {
        id: id.into(),
        method: method.into(),
        path: path.into(),
        summary: None,
        description: None,
        tags: Vec::new(),
        group: "settings".into(),
        deprecated: false,
    }
}

#[test]
fn mcp_interface_descriptors_classify_url_json_body_and_form_parameters() {
    let spec = json!({
        "paths": {
            "/api/console/widgets/{widget_id}": {
                "parameters": [
                    {
                        "name": "widget_id",
                        "in": "path",
                        "required": true,
                        "schema": { "type": "string" }
                    }
                ],
                "post": {
                    "operationId": "create_widget",
                    "parameters": [
                        {
                            "name": "locale",
                            "in": "query",
                            "required": false,
                            "schema": { "type": "string" }
                        }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "required": ["title"],
                                    "properties": {
                                        "title": {
                                            "type": "string",
                                            "description": "Widget title"
                                        },
                                        "enabled": { "type": "boolean" }
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "ok",
                            "content": {
                                "application/json": {
                                    "schema": { "type": "object" }
                                }
                            }
                        }
                    }
                }
            },
            "/api/console/uploads": {
                "post": {
                    "operationId": "upload_widget",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "multipart/form-data": {
                                "schema": {
                                    "type": "object",
                                    "required": ["file"],
                                    "properties": {
                                        "file": { "type": "string", "format": "binary" },
                                        "label": { "type": "string" }
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "ok",
                            "content": {
                                "application/json": {
                                    "schema": { "type": "object" }
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    let json_entry = mcp_interface_entry_from_operation(
        &operation("create_widget", "POST", "/api/console/widgets/{widget_id}"),
        &spec,
    )
    .expect("JSON operation should become an MCP interface entry");
    assert!(json_entry
        .parameter_descriptors
        .iter()
        .any(|descriptor| descriptor.name == "widget_id"
            && descriptor.parameter_type == McpParameterType::Url
            && descriptor.required));
    assert!(json_entry
        .parameter_descriptors
        .iter()
        .any(|descriptor| descriptor.name == "locale"
            && descriptor.parameter_type == McpParameterType::Url
            && !descriptor.required));
    assert!(json_entry
        .parameter_descriptors
        .iter()
        .any(|descriptor| descriptor.name == "title"
            && descriptor.parameter_type == McpParameterType::JsonBody
            && descriptor.field_type == "string"
            && descriptor.required
            && descriptor.description.as_deref() == Some("Widget title")));
    assert!(json_entry
        .parameter_descriptors
        .iter()
        .any(|descriptor| descriptor.name == "enabled"
            && descriptor.parameter_type == McpParameterType::JsonBody
            && descriptor.field_type == "boolean"
            && !descriptor.required));

    let form_entry = mcp_interface_entry_from_operation(
        &operation("upload_widget", "POST", "/api/console/uploads"),
        &spec,
    )
    .expect("form operation should become an MCP interface entry");
    assert!(form_entry
        .parameter_descriptors
        .iter()
        .any(|descriptor| descriptor.name == "file"
            && descriptor.parameter_type == McpParameterType::Form
            && descriptor.required));
    assert!(form_entry
        .parameter_descriptors
        .iter()
        .any(|descriptor| descriptor.name == "label"
            && descriptor.parameter_type == McpParameterType::Form
            && !descriptor.required));
}

#[test]
fn mcp_interface_descriptors_expand_nested_json_body_schema_properties() {
    let spec = json!({
        "paths": {
            "/api/console/applications/{application_id}/api-publications": {
                "parameters": [
                    {
                        "name": "application_id",
                        "in": "path",
                        "required": true,
                        "schema": { "type": "string" }
                    }
                ],
                "post": {
                    "operationId": "publish_application_api",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "required": ["api_enabled", "mapping"],
                                    "properties": {
                                        "api_enabled": { "type": "boolean" },
                                        "mapping": {
                                            "type": "object",
                                            "required": ["input", "output"],
                                            "properties": {
                                                "input": {
                                                    "type": "object",
                                                    "required": ["query_target"],
                                                    "properties": {
                                                        "query_target": {
                                                            "type": "string",
                                                            "description": "Query target"
                                                        },
                                                        "history_target": { "type": "string" }
                                                    }
                                                },
                                                "output": {
                                                    "type": "object",
                                                    "properties": {
                                                        "answer_selector": { "type": "string" },
                                                        "usage_selector": { "type": "string" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "ok",
                            "content": {
                                "application/json": {
                                    "schema": { "type": "object" }
                                }
                            }
                        }
                    }
                }
            },
            "/api/console/optional-publications": {
                "post": {
                    "operationId": "optional_publish_application_api",
                    "requestBody": {
                        "required": false,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "required": ["mapping"],
                                    "properties": {
                                        "mapping": {
                                            "type": "object",
                                            "required": ["input"],
                                            "properties": {
                                                "input": {
                                                    "type": "object",
                                                    "required": ["query_target"],
                                                    "properties": {
                                                        "query_target": { "type": "string" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "ok",
                            "content": {
                                "application/json": {
                                    "schema": { "type": "object" }
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    let entry = mcp_interface_entry_from_operation(
        &operation(
            "publish_application_api",
            "POST",
            "/api/console/applications/{application_id}/api-publications",
        ),
        &spec,
    )
    .expect("publish operation should become an MCP interface entry");

    let descriptor = |name: &str| {
        entry
            .parameter_descriptors
            .iter()
            .find(|descriptor| descriptor.name == name)
            .unwrap_or_else(|| panic!("missing descriptor {name}"))
    };

    assert_eq!(
        entry
            .parameter_descriptors
            .iter()
            .map(|descriptor| descriptor.name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "application_id",
            "api_enabled",
            "mapping.input.query_target",
            "mapping.input.history_target",
            "mapping.output.answer_selector",
            "mapping.output.usage_selector",
        ]
    );
    assert_eq!(
        descriptor("mapping.input.query_target").parameter_type,
        McpParameterType::JsonBody
    );
    assert_eq!(
        descriptor("mapping.input.query_target").field_type,
        "string"
    );
    assert_eq!(
        descriptor("mapping.input.query_target")
            .description
            .as_deref(),
        Some("Query target")
    );
    assert!(descriptor("api_enabled").required);
    assert!(descriptor("mapping.input.query_target").required);
    assert!(!descriptor("mapping.input.history_target").required);
    assert!(!descriptor("mapping.output.answer_selector").required);

    let optional_entry = mcp_interface_entry_from_operation(
        &operation(
            "optional_publish_application_api",
            "POST",
            "/api/console/optional-publications",
        ),
        &spec,
    )
    .expect("optional publish operation should become an MCP interface entry");
    let optional_descriptor = optional_entry
        .parameter_descriptors
        .iter()
        .find(|descriptor| descriptor.name == "mapping.input.query_target")
        .expect("optional body should still expose nested descriptor");
    assert!(!optional_descriptor.required);
}

#[test]
fn mcp_interface_descriptors_keep_non_object_json_body_fallback() {
    let spec = json!({
        "paths": {
            "/api/console/raw-body": {
                "post": {
                    "operationId": "submit_raw_body",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "string",
                                    "description": "Raw body"
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "ok",
                            "content": {
                                "application/json": {
                                    "schema": { "type": "object" }
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    let entry = mcp_interface_entry_from_operation(
        &operation("submit_raw_body", "POST", "/api/console/raw-body"),
        &spec,
    )
    .expect("raw body operation should become an MCP interface entry");

    assert_eq!(entry.parameter_descriptors.len(), 1);
    let descriptor = &entry.parameter_descriptors[0];
    assert_eq!(descriptor.name, "body");
    assert_eq!(descriptor.field_type, "string");
    assert_eq!(descriptor.parameter_type, McpParameterType::JsonBody);
    assert_eq!(descriptor.description.as_deref(), Some("Raw body"));
    assert!(descriptor.required);
}

#[test]
fn ac_007_published_workflow_interface_is_bindable_with_stable_identity() {
    let path = "/api/ex/orders/{order_id}";
    let spec = json!({
        "paths": {
            (path): {
                "post": {
                    "operationId": "published_workflow_operation:11111111-1111-1111-1111-111111111111",
                    "security": [{ "UserApiKey": [] }],
                    "parameters": [{
                        "name": "order_id",
                        "in": "path",
                        "required": true,
                        "schema": { "type": "string" }
                    }],
                    "responses": { "200": { "description": "Workflow Result", "content": {
                        "application/json": { "schema": { "type": "object", "properties": {
                            "accepted": { "type": "boolean" }
                        } } }
                    } } }
                }
            }
        }
    });
    let entry = mcp_interface_entry_from_operation(
        &operation(
            "published_workflow_operation:11111111-1111-1111-1111-111111111111",
            "POST",
            path,
        ),
        &spec,
    )
    .expect("published workflow operation should become an MCP interface");

    assert!(entry.bindable);
    assert_eq!(
        entry.interface_id,
        "published_workflow_operation:11111111-1111-1111-1111-111111111111"
    );
    assert_eq!(entry.parameter_descriptors[0].name, "order_id");
    assert_eq!(entry.security, json!([{ "UserApiKey": [] }]));
    assert_eq!(
        entry.result_schema["properties"]["accepted"]["type"],
        json!("boolean")
    );
}
