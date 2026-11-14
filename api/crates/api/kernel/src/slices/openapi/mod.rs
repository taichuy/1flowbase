use control_plane::application_public_api::{
    mapping::WorkflowExtensionResponseMode,
    published_workflow_operation::PublishedWorkflowOperation,
};
use serde_json::Map;
use serde_json::{json, Value};
pub fn workflow_extension_operation(operation: &PublishedWorkflowOperation) -> Value {
    let mut projected = json!({
        "tags": ["Workflow Extensions"],
        "operationId": operation.interface_id,
        "summary": format!("Invoke published workflow {}", operation.application_id),
        "parameters": openapi_parameters(&operation.parameter_schema),
        "security": [
            { "sessionCookie": [], "csrfHeader": [] },
            { "UserApiKey": [] }
        ],
        "responses": {
            "202": {
                "description": "Workflow run accepted",
                "content": {
                    "application/json": {
                        "schema": accepted_run_schema()
                    }
                }
            },
            "400": native_error_response(),
            "401": native_error_response(),
            "403": native_error_response(),
            "404": native_error_response(),
            "405": native_error_response(),
            "409": native_error_response()
        }
    });
    if operation.response_mode == WorkflowExtensionResponseMode::Sync {
        projected["responses"]["200"] = json!({
            "description": "Workflow end output",
            "content": {
                "application/json": {
                    "schema": operation.result_schema
                }
            }
        });
    }
    if let Some(request_body) = openapi_request_body(&operation.parameter_schema) {
        projected["requestBody"] = request_body;
    }
    projected
}

pub fn openapi_parameters(schema: &Value) -> Vec<Value> {
    ["path", "query"]
        .into_iter()
        .flat_map(|location| {
            let object = schema.pointer(&format!("/properties/{location}"));
            let required = object
                .and_then(|value| value.get("required"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            object
                .and_then(|value| value.get("properties"))
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
                .map(move |(name, field_schema)| json!({
                    "name": name,
                    "in": location,
                    "required": location == "path" || required.contains(&Value::String(name.clone())),
                    "schema": field_schema,
                }))
        })
        .collect()
}
pub fn openapi_request_body(schema: &Value) -> Option<Value> {
    let body_schema = schema.pointer("/properties/body").cloned();
    let form_schema = schema.pointer("/properties/form").cloned();
    if body_schema.is_none() && form_schema.is_none() {
        return None;
    }

    let mut content = Map::new();
    if let Some(schema) = body_schema {
        content.insert("application/json".to_string(), json!({ "schema": schema }));
    }
    if let Some(schema) = form_schema {
        content.insert(
            "application/x-www-form-urlencoded".to_string(),
            json!({ "schema": schema }),
        );
    }
    Some(json!({
        "required": true,
        "content": Value::Object(content)
    }))
}
pub fn accepted_run_schema() -> Value {
    json!({
        "type": "object",
        "required": ["run_id", "status"],
        "properties": {
            "run_id": { "type": "string", "format": "uuid" },
            "status": { "type": "string" }
        }
    })
}
pub fn native_error_response() -> Value {
    json!({
        "description": "Workflow extension API error",
        "content": {
            "application/json": {
                "schema": { "$ref": "#/components/schemas/NativeErrorBody" }
            }
        }
    })
}
