pub use api_host_kernel::openapi::*;
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

// utoipa derives apply explicit schemas first, then handler schema references;
// both use last-write-wins within a category. OpenApi::merge is first-write-wins,
// so merge category contributions explicitly before the existing outer merge.
fn compose_category(
    mut document: utoipa::openapi::OpenApi,
    mut paths: Vec<(usize, utoipa::openapi::OpenApi)>,
    mut schemas: Vec<(usize, utoipa::openapi::OpenApi)>,
) -> utoipa::openapi::OpenApi {
    paths.sort_by_key(|(ordinal, _)| *ordinal);
    schemas.sort_by_key(|(ordinal, _)| *ordinal);
    let mut handler_schemas = Vec::new();
    for (_, fragment) in paths {
        document.paths.merge(fragment.paths);
        if let Some(components) = fragment.components {
            handler_schemas.push(components.schemas);
        }
    }
    let components = document.components.get_or_insert_with(Default::default);
    for (_, fragment) in schemas {
        if let Some(explicit) = fragment.components {
            components.schemas.extend(explicit.schemas);
        }
    }
    for schemas in handler_schemas {
        components.schemas.extend(schemas);
    }
    document
}

pub(crate) async fn dynamic_openapi_document(state: &ApiState) -> Result<Value, ApiError> {
    let mut document = dynamic_openapi_document_with(
        state.store.clone(),
        &state.cookie_name,
        state.runtime_engine.template_catalog(),
    )
    .await?;
    crate::managed_services::append_openapi(
        &mut document,
        state.console_surface_registry.managed_services(),
    );
    Ok(document)
}

pub(crate) async fn dynamic_openapi_document_with(
    store: storage_durable_postgres::MainDurableStore,
    cookie_name: &str,
    template_catalog: &runtime_core::data_model_template_registry::DataModelTemplateCatalog,
) -> Result<Value, ApiError> {
    let mut document = serde_json::to_value(ApiDoc::openapi())?;
    let publications = store.list_enabled_extension_publications().await?;
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
    crate::openapi_docs::ensure_session_security_schemes(document_map, cookie_name)?;
    append_workflow_extension_paths(&mut document, &operations);
    crate::runtime_data_model_docs::append_template_runtime_openapi_paths(
        &mut document,
        template_catalog,
    );
    Ok(document)
}

pub(crate) async fn workflow_extension_openapi_document_with(
    store: storage_durable_postgres::MainDurableStore,
    cookie_name: &str,
    template_catalog: &runtime_core::data_model_template_registry::DataModelTemplateCatalog,
) -> Result<Value, ApiError> {
    let mut document = dynamic_openapi_document_with(store, cookie_name, template_catalog).await?;
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

#[cfg_attr(test, api_test_selection::group(core))]
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
    fn category_composition_preserves_path_and_schema_collision_precedence() {
        fn fragment(label: &str) -> utoipa::openapi::OpenApi {
            serde_json::from_value(json!({
                "openapi": "3.1.0",
                "info": {"title": "collision fixture", "version": "1"},
                "paths": {"/collision": {"get": {
                    "operationId": label, "responses": {"200": {"description": "ok"}}
                }}},
                "components": {"schemas": {
                    "Shared": {"type": "string", "description": label}
                }}
            }))
            .unwrap()
        }
        let mut explicit_early = fragment("explicit-early");
        explicit_early.components.as_mut().unwrap().schemas.insert(
            "ExplicitOnly".into(),
            serde_json::from_value(json!({"type": "string", "description": "early"})).unwrap(),
        );
        let mut explicit_late = fragment("explicit-late");
        explicit_late.components.as_mut().unwrap().schemas.insert(
            "ExplicitOnly".into(),
            serde_json::from_value(json!({"type": "string", "description": "late"})).unwrap(),
        );
        let document = compose_category(
            serde_json::from_value(json!({
                "openapi": "3.1.0", "info": {"title": "root", "version": "1"}, "paths": {}
            }))
            .unwrap(),
            vec![
                (7, fragment("handler-late")),
                (2, fragment("handler-early")),
            ],
            vec![(9, explicit_late), (1, explicit_early)],
        );
        let document = serde_json::to_value(document).unwrap();
        assert_eq!(
            document["paths"]["/collision"]["get"]["operationId"],
            "handler-early"
        );
        assert_eq!(
            document["components"]["schemas"]["ExplicitOnly"]["description"],
            "late"
        );
        assert_eq!(
            document["components"]["schemas"]["Shared"]["description"],
            "handler-late"
        );
        assert_eq!(document["info"]["title"], "root");
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
    fn ac_006_external_auth_fragment_and_composition_keep_canonical_metadata() {
        for document in [console::ConsoleOpenApi::openapi(), ApiDoc::openapi()] {
            for binding in crate::routes::external_auth::interface::bindings_metadata() {
                let path = document
                    .paths
                    .paths
                    .get(binding.path)
                    .expect("canonical external authentication path must be documented");
                let operation = match binding.method {
                    "GET" => path.get.as_ref(),
                    "POST" => path.post.as_ref(),
                    _ => None,
                }
                .expect("canonical external authentication method must be documented");
                assert_eq!(
                    operation.summary.as_deref(),
                    Some(binding.summary),
                    "{} {} summary",
                    binding.method,
                    binding.path
                );
                assert_eq!(
                    operation.description.as_deref(),
                    Some(binding.description),
                    "{} {} description",
                    binding.method,
                    binding.path
                );
            }
        }
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
