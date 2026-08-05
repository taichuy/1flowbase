use std::sync::Arc;

use anyhow::anyhow;
use axum::{extract::State, Json};
use control_plane::{
    application_public_api::{
        mapping::WorkflowExtensionResponseMode,
        published_workflow_operation::{
            build_published_workflow_operations, PublishedWorkflowOperation,
        },
    },
    ports::ApplicationPublicationRepository,
};
use serde_json::{json, Map, Value};
use utoipa::OpenApi;

#[derive(utoipa::ToSchema)]
#[schema(value_type = String, format = Binary)]
#[allow(dead_code)]
pub(crate) struct OpenApiBinaryBody(pub Vec<u8>);

use crate::{app_state::ApiState, error_response::ApiError};

mod application;
mod console;
mod extensions;

pub struct ApiDoc;

impl OpenApi for ApiDoc {
    fn openapi() -> utoipa::openapi::OpenApi {
        let mut document = utoipa::openapi::OpenApi::new(
            utoipa::openapi::Info::new("1flowbase API", "0.1.0"),
            utoipa::openapi::path::Paths::new(),
        );
        for fragment in [
            application::ApplicationOpenApi::openapi(),
            console::ConsoleOpenApi::openapi(),
            extensions::ExtensionsOpenApi::openapi(),
        ] {
            document.merge(fragment);
        }
        document
    }
}

pub(crate) async fn dynamic_openapi_document(state: &ApiState) -> Result<Value, ApiError> {
    let mut document = serde_json::to_value(ApiDoc::openapi())?;
    let publications = state.store.list_enabled_extension_publications().await?;
    let operations = build_published_workflow_operations(publications)
        .map_err(|_| control_plane::errors::ControlPlaneError::Conflict("workflow_route"))?;
    document["components"]["securitySchemes"]["UserApiKey"] = json!({
        "type": "http",
        "scheme": "bearer",
        "bearerFormat": "User API Key"
    });
    let document_map = document
        .as_object_mut()
        .ok_or_else(|| anyhow!("dynamic OpenAPI document must be an object"))?;
    crate::openapi_docs::ensure_session_security_schemes(document_map, &state.cookie_name)?;
    append_workflow_extension_paths(&mut document, &operations);
    Ok(document)
}

pub(crate) async fn workflow_extension_openapi_document(
    state: &ApiState,
) -> Result<Value, ApiError> {
    let mut document = dynamic_openapi_document(state).await?;
    let paths = document
        .get_mut("paths")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| anyhow!("dynamic OpenAPI document must contain paths"))?;
    paths.retain(|path, _| path.starts_with("/api/ex/"));
    Ok(document)
}

pub async fn dynamic_openapi(State(state): State<Arc<ApiState>>) -> Result<Json<Value>, ApiError> {
    Ok(Json(dynamic_openapi_document(&state).await?))
}

fn append_workflow_extension_paths(
    document: &mut Value,
    operations: &[PublishedWorkflowOperation],
) {
    let Some(paths) = document.get_mut("paths").and_then(Value::as_object_mut) else {
        return;
    };

    for published_operation in operations {
        let operation = workflow_extension_operation(published_operation);
        let path = published_operation.public_path();
        let method = published_operation.method.as_str().to_ascii_lowercase();
        let entry = paths
            .entry(path)
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(path_item) = entry.as_object_mut() {
            path_item.insert(method, operation);
        }
    }
}

pub(crate) fn workflow_extension_operation(operation: &PublishedWorkflowOperation) -> Value {
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

fn openapi_parameters(schema: &Value) -> Vec<Value> {
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

fn openapi_request_body(schema: &Value) -> Option<Value> {
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

fn accepted_run_schema() -> Value {
    json!({
        "type": "object",
        "required": ["run_id", "status"],
        "properties": {
            "run_id": { "type": "string", "format": "uuid" },
            "status": { "type": "string" }
        }
    })
}

fn native_error_response() -> Value {
    json!({
        "description": "Workflow extension API error",
        "content": {
            "application/json": {
                "schema": { "$ref": "#/components/schemas/NativeErrorBody" }
            }
        }
    })
}

#[cfg(test)]
mod workflow_operation_tests {
    use super::*;
    use control_plane::application_public_api::{
        mapping::{
            ApplicationApiMappingConfig, ApplicationApiMappingInput, ApplicationApiMappingOutput,
            WorkflowExtensionApiConfig, WorkflowExtensionHttpMethod,
        },
        publications::ApplicationPublicationVersionRecord,
        published_workflow_operation::PublishedWorkflowOperation,
    };
    use std::collections::BTreeMap;
    use time::OffsetDateTime;
    use uuid::Uuid;

    #[derive(Debug, PartialEq)]
    struct OpenApiInventory {
        operations: BTreeMap<(String, String), Value>,
        schemas: BTreeMap<String, Value>,
        security_schemes: BTreeMap<String, Value>,
    }

    fn inventory(document: utoipa::openapi::OpenApi) -> OpenApiInventory {
        let document = serde_json::to_value(document).expect("OpenAPI document must serialize");
        let mut operations = BTreeMap::new();
        for (path, path_item) in document["paths"]
            .as_object()
            .expect("OpenAPI paths must be an object")
        {
            for (method, operation) in path_item
                .as_object()
                .expect("OpenAPI path item must be an object")
            {
                if operation.get("operationId").is_some() {
                    assert!(
                        operations
                            .insert((path.clone(), method.clone()), operation.clone())
                            .is_none(),
                        "duplicate OpenAPI operation {method} {path}"
                    );
                }
            }
        }

        let component_inventory = |name: &str| {
            document["components"][name]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect()
        };
        OpenApiInventory {
            operations,
            schemas: component_inventory("schemas"),
            security_schemes: component_inventory("securitySchemes"),
        }
    }

    fn merge_inventory(target: &mut OpenApiInventory, fragment: OpenApiInventory) {
        for (identity, operation) in fragment.operations {
            assert!(
                target
                    .operations
                    .insert(identity.clone(), operation)
                    .is_none(),
                "fragment operation collision: {} {}",
                identity.1,
                identity.0
            );
        }
        for (name, schema) in fragment.schemas {
            if let Some(existing) = target.schemas.get(&name) {
                assert_eq!(
                    existing, &schema,
                    "fragment schema collision with different definitions: {name}"
                );
            } else {
                target.schemas.insert(name, schema);
            }
        }
        for (name, scheme) in fragment.security_schemes {
            if let Some(existing) = target.security_schemes.get(&name) {
                assert_eq!(
                    existing, &scheme,
                    "fragment security scheme collision with different definitions: {name}"
                );
            } else {
                target.security_schemes.insert(name, scheme);
            }
        }
    }

    fn operation() -> PublishedWorkflowOperation {
        let application_id = Uuid::from_u128(0x11111111111111111111111111111111);
        PublishedWorkflowOperation::from_publication(ApplicationPublicationVersionRecord {
            id: Uuid::from_u128(0x22222222222222222222222222222222),
            application_id,
            workspace_id: Uuid::from_u128(0x33333333333333333333333333333333),
            flow_id: Uuid::from_u128(0x44444444444444444444444444444444),
            flow_version_id: Uuid::from_u128(0x55555555555555555555555555555555),
            mapping_snapshot: ApplicationApiMappingConfig {
                input: ApplicationApiMappingInput {
                    query_target: "node-workflow-start.query".into(),
                    model_target: None,
                    inputs_target: None,
                    history_target: None,
                    attachments_target: None,
                },
                output: ApplicationApiMappingOutput::default(),
                extension: Some(WorkflowExtensionApiConfig {
                    slug: "orders/{order_id}".into(),
                    method: WorkflowExtensionHttpMethod::Post,
                    response_mode: WorkflowExtensionResponseMode::Sync,
                }),
            },
            extension_slug: Some("orders/{order_id}".into()),
            compiled_plan_id: Uuid::from_u128(0x66666666666666666666666666666666),
            version_sequence: 1,
            active: true,
            api_enabled: true,
            flow_schema_version: "1flowbase.flow/v2".into(),
            document_hash: "hash".into(),
            document_snapshot: json!({
                "graph": { "nodes": [
                    { "id": "node-workflow-start", "type": "workflow_start", "config": { "input_fields": [
                        { "key": "order_id", "valueType": "string", "source": "path", "required": true }
                    ] } },
                    { "id": "node-workflow-end", "type": "workflow_end", "outputs": [
                        { "key": "accepted", "valueType": "boolean" }
                    ] }
                ] }
            }),
            runtime_profile_snapshot: json!({}),
            output_selector: json!({}),
            dependency_snapshot: Vec::new(),
            created_by: Uuid::from_u128(0x77777777777777777777777777777777),
            created_at: OffsetDateTime::UNIX_EPOCH,
        })
        .unwrap()
    }

    #[test]
    fn ac_006_composed_openapi_preserves_fragment_inventory_without_collisions() {
        let mut expected = OpenApiInventory {
            operations: BTreeMap::new(),
            schemas: BTreeMap::new(),
            security_schemes: BTreeMap::new(),
        };
        for fragment in [
            application::ApplicationOpenApi::openapi(),
            console::ConsoleOpenApi::openapi(),
            extensions::ExtensionsOpenApi::openapi(),
        ] {
            merge_inventory(&mut expected, inventory(fragment));
        }

        let document = ApiDoc::openapi();
        assert_eq!(document.info.title, "1flowbase API");
        assert_eq!(document.info.version, "0.1.0");
        assert_eq!(inventory(document), expected);
    }

    #[test]
    fn ac_006_openapi_projects_start_end_and_current_user_or_api_key_security() {
        let projected = workflow_extension_operation(&operation());
        assert_eq!(
            projected["security"],
            json!([
                { "sessionCookie": [], "csrfHeader": [] },
                { "UserApiKey": [] }
            ])
        );
        assert_eq!(projected["parameters"][0]["name"], json!("order_id"));
        assert_eq!(
            projected["responses"]["200"]["content"]["application/json"]["schema"]["properties"]
                ["accepted"]["type"],
            json!("boolean")
        );
    }
}
