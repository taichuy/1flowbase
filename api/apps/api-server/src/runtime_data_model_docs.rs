use control_plane::{errors::ControlPlaneError, model_definition::ModelDefinitionService};
use plugin_framework::{DataModelTemplateIdentity, DataModelTemplateOperation};
use runtime_core::{
    data_model_template_registry::CompiledDataModelTemplate,
    general_data_model_template::{core_data_model_template_registry, CoreGeneralOperationHandler},
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    openapi_docs::{DocsCatalogCategory, DocsCatalogCategoryOperations, DocsCatalogOperation},
};

pub const DATA_MODEL_DOCS_CATEGORY_ID: &str = "data-model-apis";
pub const DATA_MODEL_DOCS_CATEGORY_LABEL: &str = "Data Model APIs";
const DATA_MODEL_OPERATION_ID_PREFIX: &str = "data_model__";

pub fn operation_id(model_id: uuid::Uuid, operation_code: &str) -> String {
    format!("{DATA_MODEL_OPERATION_ID_PREFIX}{model_id}__{operation_code}")
}

pub fn parse_operation_id(
    operation_id: &str,
) -> Result<Option<(uuid::Uuid, String)>, &'static str> {
    let Some(rest) = operation_id.strip_prefix(DATA_MODEL_OPERATION_ID_PREFIX) else {
        return Ok(None);
    };
    let Some((model_id, suffix)) = rest.split_once("__") else {
        return Err("missing operation suffix");
    };
    let model_id = uuid::Uuid::parse_str(model_id).map_err(|_| "invalid model id")?;
    if suffix.is_empty() {
        return Err("missing operation code");
    }
    Ok(Some((model_id, suffix.to_owned())))
}

pub async fn ready_models(
    state: &ApiState,
    actor_user_id: Uuid,
) -> Result<Vec<domain::ModelDefinitionRecord>, ApiError> {
    let models = match ModelDefinitionService::new(state.store.clone())
        .list_models(actor_user_id)
        .await
    {
        Ok(models) => models,
        Err(error) => {
            if let Some(ControlPlaneError::PermissionDenied(_)) =
                error.downcast_ref::<ControlPlaneError>()
            {
                return Ok(vec![]);
            }
            return Err(error.into());
        }
    };
    let mut models = models
        .into_iter()
        .filter(|model| model.status == domain::DataModelStatus::Published)
        .collect::<Vec<_>>();
    models.sort_by(|left, right| left.code.cmp(&right.code));
    Ok(models)
}

pub async fn ready_model(
    state: &ApiState,
    actor_user_id: Uuid,
    model_id: Uuid,
) -> Result<Option<domain::ModelDefinitionRecord>, ApiError> {
    let model = match ModelDefinitionService::new(state.store.clone())
        .get_model(actor_user_id, model_id)
        .await
    {
        Ok(model) => model,
        Err(error) => {
            if let Some(ControlPlaneError::PermissionDenied(_) | ControlPlaneError::NotFound(_)) =
                error.downcast_ref::<ControlPlaneError>()
            {
                return Ok(None);
            }
            return Err(error.into());
        }
    };
    if model.status != domain::DataModelStatus::Published {
        return Ok(None);
    }
    Ok(Some(model))
}

fn template_for_model(
    model: &domain::ModelDefinitionRecord,
) -> Option<&'static CompiledDataModelTemplate> {
    let identity = DataModelTemplateIdentity {
        provider: model.template_provider.clone(),
        code: model.template_code.clone(),
        version: model.template_version.clone(),
    };
    core_data_model_template_registry()
        .ok()?
        .resolve(&identity)
        .ok()
}

fn operation_path(
    model: &domain::ModelDefinitionRecord,
    operation: &DataModelTemplateOperation,
) -> String {
    operation.path.replace("{model_code}", &model.code)
}

pub fn build_category(models: &[domain::ModelDefinitionRecord]) -> Option<DocsCatalogCategory> {
    let operation_count = models
        .iter()
        .filter_map(template_for_model)
        .map(|template| template.descriptor().operations.len())
        .sum();
    if operation_count == 0 {
        return None;
    }
    Some(DocsCatalogCategory {
        id: DATA_MODEL_DOCS_CATEGORY_ID.to_string(),
        label: DATA_MODEL_DOCS_CATEGORY_LABEL.to_string(),
        operation_count,
    })
}

pub fn build_category_operations(
    models: &[domain::ModelDefinitionRecord],
) -> DocsCatalogCategoryOperations {
    let mut operations = Vec::new();
    for model in models {
        let group = if model.title.is_empty() {
            model.code.clone()
        } else {
            model.title.clone()
        };
        let Some(template) = template_for_model(model) else {
            continue;
        };
        for operation in &template.descriptor().operations {
            operations.push(DocsCatalogOperation {
                id: operation_id(model.id, &operation.code),
                method: operation.method.as_str().to_owned(),
                path: operation_path(model, operation),
                summary: Some(format!("{} — {}", operation.summary, model.title)),
                description: Some(format!(
                    "{} Data Model `{}`.",
                    operation.description, model.code
                )),
                tags: vec!["data-model".to_string(), model.code.clone()],
                group: group.clone(),
                deprecated: false,
            });
        }
    }
    DocsCatalogCategoryOperations {
        id: DATA_MODEL_DOCS_CATEGORY_ID.to_string(),
        label: DATA_MODEL_DOCS_CATEGORY_LABEL.to_string(),
        operations,
    }
}

pub fn build_model_openapi(model: &domain::ModelDefinitionRecord) -> Option<Value> {
    let template = template_for_model(model)?;
    let schema_name = record_schema_name(&model.code);
    let create_schema_name = format!("{schema_name}CreateInput");
    let update_schema_name = format!("{schema_name}UpdateInput");
    let schema_ref = format!("#/components/schemas/{schema_name}");
    let create_schema_ref = format!("#/components/schemas/{create_schema_name}");
    let update_schema_ref = format!("#/components/schemas/{update_schema_name}");

    let mut paths = serde_json::Map::new();
    for operation in &template.descriptor().operations {
        let path = operation_path(model, operation);
        let method = operation.method.as_str().to_ascii_lowercase();
        let definition = build_operation_definition(
            model,
            operation,
            &schema_ref,
            &create_schema_ref,
            &update_schema_ref,
        )?;
        paths
            .entry(path)
            .or_insert_with(|| Value::Object(serde_json::Map::new()))
            .as_object_mut()?
            .insert(method, definition);
    }

    Some(json!({
        "openapi": "3.1.0",
        "info": {
            "title": format!("{} Data Model API", model.title),
            "version": "1.0.0"
        },
        "security": [{ "patBearer": [] }],
        "paths": Value::Object(paths),
        "components": {
            "securitySchemes": {
                "patBearer": {
                    "type": "http",
                    "scheme": "bearer",
                    "bearerFormat": "pat_ user API key",
                    "description": "Use Authorization: Bearer pat_... for user API key requests. PAT uses the bound user's role permissions."
                }
            },
            "schemas": {
                schema_name: record_schema(model, template),
                create_schema_name: record_write_schema(model, true),
                update_schema_name: record_write_schema(model, false)
            }
        },
        "x-data-model": {
            "id": model.id.to_string(),
            "code": model.code,
            "status": model.status.as_str(),
            "source_kind": model.source_kind.as_str(),
            "protected": model.protection.is_protected
        },
        "x-scope-permission-note": "Runtime Data Model APIs accept pat_ user API keys with bound user role permissions and require an enabled owner or scope_all scope grant for the request scope.",
        "x-external-source-safety-limits": external_source_safety_limits(model)
    }))
}

pub fn build_category_openapi(models: &[domain::ModelDefinitionRecord]) -> Value {
    let mut paths = serde_json::Map::new();
    let mut schemas = serde_json::Map::new();
    for model in models {
        let Some(spec) = build_model_openapi(model) else {
            continue;
        };
        if let Some(spec_paths) = spec.get("paths").and_then(Value::as_object) {
            for (path, path_item) in spec_paths {
                paths.insert(path.clone(), path_item.clone());
            }
        }
        if let Some(spec_schemas) = spec
            .get("components")
            .and_then(Value::as_object)
            .and_then(|components| components.get("schemas"))
            .and_then(Value::as_object)
        {
            for (schema_name, schema) in spec_schemas {
                schemas.insert(schema_name.clone(), schema.clone());
            }
        }
    }

    json!({
        "openapi": "3.1.0",
        "info": {
            "title": DATA_MODEL_DOCS_CATEGORY_LABEL,
            "version": "1.0.0"
        },
        "security": [{ "patBearer": [] }],
        "paths": Value::Object(paths),
        "components": {
            "securitySchemes": {
                "patBearer": {
                    "type": "http",
                    "scheme": "bearer",
                    "bearerFormat": "pat_ user API key",
                    "description": "Use Authorization: Bearer pat_... for user API key requests. PAT uses the bound user's role permissions."
                }
            },
            "schemas": Value::Object(schemas)
        },
        "x-category": DATA_MODEL_DOCS_CATEGORY_ID
    })
}

pub fn build_operation_openapi(
    model: &domain::ModelDefinitionRecord,
    operation_code: &str,
) -> Option<Value> {
    let template = template_for_model(model)?;
    let operation_descriptor = template.operation(operation_code)?;
    let full_spec = build_model_openapi(model)?;
    let path = operation_path(model, operation_descriptor);
    let method = operation_descriptor.method.as_str().to_ascii_lowercase();
    let operation = full_spec
        .get("paths")
        .and_then(Value::as_object)
        .and_then(|paths| paths.get(&path))
        .and_then(Value::as_object)
        .and_then(|path_item| path_item.get(&method))
        .cloned()?;
    let mut path_item = serde_json::Map::new();
    path_item.insert(method, operation);
    let mut paths = serde_json::Map::new();
    paths.insert(path, Value::Object(path_item));

    Some(json!({
        "openapi": "3.1.0",
        "info": full_spec.get("info").cloned().unwrap_or_else(|| json!({})),
        "security": full_spec.get("security").cloned().unwrap_or_else(|| json!([])),
        "paths": Value::Object(paths),
        "components": full_spec
            .get("components")
            .cloned()
            .unwrap_or_else(|| json!({})),
        "x-data-model": full_spec
            .get("x-data-model")
            .cloned()
            .unwrap_or_else(|| json!({})),
        "x-scope-permission-note": full_spec
            .get("x-scope-permission-note")
            .cloned()
            .unwrap_or_else(|| Value::String(String::new())),
        "x-external-source-safety-limits": full_spec
            .get("x-external-source-safety-limits")
            .cloned()
            .unwrap_or_else(|| Value::String(String::new()))
    }))
}

fn build_operation_definition(
    model: &domain::ModelDefinitionRecord,
    operation: &DataModelTemplateOperation,
    schema_ref: &str,
    create_schema_ref: &str,
    update_schema_ref: &str,
) -> Option<Value> {
    let handler = CoreGeneralOperationHandler::from_ref(&operation.handler_ref)?;
    let mut definition = serde_json::Map::from_iter([
        (
            "operationId".to_owned(),
            Value::String(operation_id(model.id, &operation.code)),
        ),
        (
            "summary".to_owned(),
            Value::String(format!("{} — {}", operation.summary, model.title)),
        ),
        (
            "description".to_owned(),
            Value::String(operation.description.clone()),
        ),
        ("security".to_owned(), json!([{ "patBearer": [] }])),
    ]);

    match handler {
        CoreGeneralOperationHandler::ListRecords => {
            definition.insert("parameters".to_owned(), runtime_list_parameters());
            definition.insert("responses".to_owned(), runtime_responses(schema_ref, true));
        }
        CoreGeneralOperationHandler::GetRecord => {
            definition.insert(
                "parameters".to_owned(),
                json!([id_parameter(), expand_parameter()]),
            );
            definition.insert("responses".to_owned(), runtime_responses(schema_ref, false));
        }
        CoreGeneralOperationHandler::CreateRecord => {
            definition.insert(
                "requestBody".to_owned(),
                json_request_body(create_schema_ref),
            );
            definition.insert("responses".to_owned(), runtime_responses(schema_ref, false));
        }
        CoreGeneralOperationHandler::UpdateRecord => {
            definition.insert("parameters".to_owned(), json!([id_parameter()]));
            definition.insert(
                "requestBody".to_owned(),
                json_request_body(update_schema_ref),
            );
            definition.insert("responses".to_owned(), runtime_responses(schema_ref, false));
        }
        CoreGeneralOperationHandler::DeleteRecord => {
            definition.insert("parameters".to_owned(), json!([id_parameter()]));
            definition.insert("responses".to_owned(), runtime_delete_responses());
        }
    }
    Some(Value::Object(definition))
}

fn runtime_list_parameters() -> Value {
    json!([
        {
            "name": "filter",
            "in": "query",
            "required": false,
            "schema": { "type": "string" },
            "example": "{\"status\":{\"$eq\":\"paid\"}}",
            "description": "JSON filter expression. Supports field operators such as $eq, $ne, $gt, $gte, $lt, $lte, $includes, $notIncludes and $in."
        },
        {
            "name": "sort",
            "in": "query",
            "required": false,
            "schema": { "type": "string" },
            "example": "created_at:desc",
            "description": "Single sort expression using field:asc or field:desc."
        },
        {
            "name": "page",
            "in": "query",
            "required": false,
            "schema": { "type": "integer", "minimum": 1, "default": 1 },
            "description": "Page number."
        },
        {
            "name": "page_size",
            "in": "query",
            "required": false,
            "schema": { "type": "integer", "minimum": 1, "default": 20 },
            "description": "Page size."
        },
        expand_parameter()
    ])
}

fn id_parameter() -> Value {
    json!({
        "name": "id",
        "in": "path",
        "required": true,
        "schema": { "type": "string", "format": "uuid" }
    })
}

fn expand_parameter() -> Value {
    json!({
        "name": "expand",
        "in": "query",
        "required": false,
        "schema": { "type": "string" },
        "example": "customer,items",
        "description": "Comma-separated relation field codes to expand."
    })
}

fn json_request_body(schema_ref: &str) -> Value {
    json!({
        "required": true,
        "content": {
            "application/json": {
                "schema": { "$ref": schema_ref }
            }
        }
    })
}

fn runtime_responses(schema_ref: &str, list: bool) -> Value {
    let success_schema = if list {
        json!({
            "type": "object",
            "properties": {
                "items": {
                    "type": "array",
                    "items": { "$ref": schema_ref }
                },
                "total": { "type": "integer" }
            }
        })
    } else {
        json!({ "$ref": schema_ref })
    };

    json!({
        "200": {
            "description": "Success",
            "content": { "application/json": { "schema": success_schema } }
        },
        "201": { "description": "Created" },
        "400": { "description": "Bad request or invalid filter/sort/expand expression" },
        "401": { "description": "Missing or invalid API key" },
        "403": { "description": "API key, action permission, or scope grant denied" },
        "404": { "description": "Data Model or record not found" },
        "409": { "description": "Data Model is not published, disabled, broken, or unsafe" }
    })
}

fn runtime_delete_responses() -> Value {
    json!({
        "200": { "description": "Deleted" },
        "401": { "description": "Missing or invalid API key" },
        "403": { "description": "API key, action permission, or scope grant denied" },
        "404": { "description": "Data Model or record not found" },
        "409": { "description": "Data Model is not published, disabled, broken, or unsafe" }
    })
}

fn record_schema(
    model: &domain::ModelDefinitionRecord,
    template: &CompiledDataModelTemplate,
) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for field in &template.descriptor().system_fields {
        properties.insert(field.code.clone(), field.value_schema.clone());
        if field.required {
            required.push(Value::String(field.code.clone()));
        }
    }
    for field in &model.fields {
        if properties.contains_key(&field.code) {
            continue;
        }
        properties.insert(field.code.clone(), record_field_schema(field));
        if field.is_required {
            required.push(Value::String(field.code.clone()));
        }
    }

    json!({
        "type": "object",
        "properties": properties,
        "required": required
    })
}

fn record_field_schema(field: &domain::ModelFieldRecord) -> Value {
    let schema = field_schema(field);
    if field.is_required {
        schema
    } else {
        json!({ "anyOf": [schema, { "type": "null" }] })
    }
}

fn record_write_schema(model: &domain::ModelDefinitionRecord, include_required: bool) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for field in &model.fields {
        if !field.is_writable {
            continue;
        }
        properties.insert(field.code.clone(), field_schema(field));
        if include_required && field.api_required {
            required.push(Value::String(field.code.clone()));
        }
    }

    json!({
        "type": "object",
        "properties": properties,
        "required": required
    })
}

fn field_schema(field: &domain::ModelFieldRecord) -> Value {
    match field.field_kind {
        domain::ModelFieldKind::Number => json!({ "type": "number" }),
        domain::ModelFieldKind::Boolean => json!({ "type": "boolean" }),
        domain::ModelFieldKind::Datetime => {
            json!({ "type": "string", "format": "date-time" })
        }
        domain::ModelFieldKind::Json => json!({ "type": "object" }),
        domain::ModelFieldKind::ManyToOne
        | domain::ModelFieldKind::OneToMany
        | domain::ModelFieldKind::ManyToMany => json!({
            "type": "string",
            "format": "uuid",
            "description": "Relation record id or relation expansion target."
        }),
        domain::ModelFieldKind::String
        | domain::ModelFieldKind::Enum
        | domain::ModelFieldKind::Text => json!({ "type": "string" }),
    }
}

fn external_source_safety_limits(model: &domain::ModelDefinitionRecord) -> String {
    if model.source_kind == domain::DataModelSourceKind::ExternalSource {
        let supports_scope_filter = model
            .external_capability_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.get("supports_scope_filter"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        return format!(
            "External source APIs require provider-enforced scope filter support before exposure; supports_scope_filter={supports_scope_filter}."
        );
    }

    "Main-source APIs use platform scope filter enforcement; external source exposure still requires provider scope filter support.".to_string()
}

fn record_schema_name(code: &str) -> String {
    let mut name = String::new();
    for segment in code.split(['_', '-']).filter(|segment| !segment.is_empty()) {
        let mut chars = segment.chars();
        if let Some(first) = chars.next() {
            name.extend(first.to_uppercase());
            name.push_str(chars.as_str());
        }
    }
    if name.is_empty() {
        name.push_str("DataModel");
    }
    name.push_str("Record");
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_drives_catalog_openapi_and_system_fields() {
        let model = model_fixture();
        let template = template_for_model(&model).expect("production template must resolve");
        let catalog = build_category_operations(std::slice::from_ref(&model));

        assert_eq!(
            catalog.operations.len(),
            template.descriptor().operations.len()
        );
        for descriptor in &template.descriptor().operations {
            let catalog_operation = catalog
                .operations
                .iter()
                .find(|operation| operation.id == operation_id(model.id, &descriptor.code))
                .expect("every descriptor operation must be cataloged");
            assert_eq!(catalog_operation.method, descriptor.method.as_str());
            assert_eq!(catalog_operation.path, operation_path(&model, descriptor));
            let operation_spec = build_operation_openapi(&model, &descriptor.code)
                .expect("every resolved handler must project OpenAPI");
            assert!(operation_spec["paths"][&catalog_operation.path]
                [descriptor.method.as_str().to_ascii_lowercase()]
            .is_object());
        }

        let spec = build_model_openapi(&model).expect("production template must project OpenAPI");
        let record_properties = &spec["components"]["schemas"]["OrdersRecord"]["properties"];
        for field in &template.descriptor().system_fields {
            assert_eq!(record_properties[&field.code], field.value_schema);
        }
    }

    #[test]
    fn unknown_template_identity_is_excluded_from_dynamic_docs() {
        let mut model = model_fixture();
        model.template_provider = "missing".to_owned();

        assert!(build_category(&[model.clone()]).is_none());
        assert!(build_category_operations(&[model.clone()])
            .operations
            .is_empty());
        assert!(build_model_openapi(&model).is_none());
    }

    fn model_fixture() -> domain::ModelDefinitionRecord {
        domain::ModelDefinitionRecord {
            id: Uuid::nil(),
            scope_kind: domain::DataModelScopeKind::Workspace,
            scope_id: Uuid::nil(),
            data_source_instance_id: None,
            source_kind: domain::DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            template_provider: domain::CORE_DATA_MODEL_TEMPLATE_PROVIDER.to_owned(),
            template_code: domain::GENERAL_DATA_MODEL_TEMPLATE_CODE.to_owned(),
            template_version: domain::GENERAL_DATA_MODEL_TEMPLATE_VERSION.to_owned(),
            code: "orders".to_owned(),
            title: "Orders".to_owned(),
            description: None,
            physical_table_name: "rtm_workspace_orders".to_owned(),
            acl_namespace: "state_model.orders".to_owned(),
            audit_namespace: "audit.state_model.orders".to_owned(),
            fields: vec![],
            availability_status: domain::MetadataAvailabilityStatus::Available,
            status: domain::DataModelStatus::Published,
            protection: domain::DataModelProtection::default(),
        }
    }
}
