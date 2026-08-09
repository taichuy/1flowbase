use control_plane::{errors::ControlPlaneError, model_definition::ModelDefinitionService};
use plugin_framework::{DataModelTemplateIdentity, DataModelTemplateOperation};
use runtime_core::{
    data_model_template_registry::{CompiledDataModelTemplate, DataModelTemplateCatalog},
    general_data_model_template::CoreGeneralOperationHandler,
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
    catalog: &DataModelTemplateCatalog,
) -> Option<CompiledDataModelTemplate> {
    let identity = DataModelTemplateIdentity {
        provider: model.template_provider.clone(),
        code: model.template_code.clone(),
        version: model.template_version.clone(),
    };
    catalog.resolve(&identity).ok()
}

fn operation_path(
    model: &domain::ModelDefinitionRecord,
    operation: &DataModelTemplateOperation,
) -> String {
    operation.path.replace("{model_code}", &model.code)
}

pub fn build_category(
    models: &[domain::ModelDefinitionRecord],
    catalog: &DataModelTemplateCatalog,
) -> Option<DocsCatalogCategory> {
    let operation_count = models
        .iter()
        .filter_map(|model| template_for_model(model, catalog))
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
    catalog: &DataModelTemplateCatalog,
) -> DocsCatalogCategoryOperations {
    let mut operations = Vec::new();
    for model in models {
        let group = if model.title.is_empty() {
            model.code.clone()
        } else {
            model.title.clone()
        };
        let Some(template) = template_for_model(model, catalog) else {
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

pub fn build_model_openapi(
    model: &domain::ModelDefinitionRecord,
    catalog: &DataModelTemplateCatalog,
) -> Option<Value> {
    let template = template_for_model(model, catalog)?;
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
                schema_name: record_schema(model, &template),
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

pub fn build_category_openapi(
    models: &[domain::ModelDefinitionRecord],
    catalog: &DataModelTemplateCatalog,
) -> Value {
    let mut paths = serde_json::Map::new();
    let mut schemas = serde_json::Map::new();
    for model in models {
        let Some(spec) = build_model_openapi(model, catalog) else {
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

pub fn append_template_runtime_openapi_paths(
    document: &mut Value,
    catalog: &DataModelTemplateCatalog,
) {
    let Some(paths) = document.get_mut("paths").and_then(Value::as_object_mut) else {
        return;
    };

    for template in catalog.templates() {
        let identity = template.identity().canonical_name();
        for operation in &template.descriptor().operations {
            let method = operation.method.as_str().to_ascii_lowercase();
            let path_item = paths
                .entry(operation.path.clone())
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
            let Some(path_item) = path_item.as_object_mut() else {
                continue;
            };
            if let Some(existing) = path_item.get_mut(&method) {
                append_operation_template_identity(existing, &identity);
                continue;
            }

            let operation_id = format!(
                "data_model_template__{}__{}__{}__{}",
                template.identity().provider,
                template.identity().code,
                template.identity().version,
                operation.code
            );
            let mut definition = descriptor_operation_definition(
                operation,
                operation_id,
                operation.summary.clone(),
                true,
            );
            definition["tags"] = json!(["Data Model Runtime"]);
            definition["security"] = json!([{ "UserApiKey": [] }]);
            definition["x-data-model-templates"] = json!([identity]);
            path_item.insert(method, definition);
        }
    }
}

fn append_operation_template_identity(operation: &mut Value, identity: &str) {
    let Some(identities) = operation
        .get_mut("x-data-model-templates")
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    if !identities
        .iter()
        .any(|value| value.as_str() == Some(identity))
    {
        identities.push(Value::String(identity.to_owned()));
    }
}

pub fn build_operation_openapi(
    model: &domain::ModelDefinitionRecord,
    operation_code: &str,
    catalog: &DataModelTemplateCatalog,
) -> Option<Value> {
    let template = template_for_model(model, catalog)?;
    let operation_descriptor = template.operation(operation_code)?;
    let full_spec = build_model_openapi(model, catalog)?;
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
    let handler = CoreGeneralOperationHandler::from_ref(&operation.handler_ref);
    let operation_id = operation_id(model.id, &operation.code);
    let summary = format!("{} — {}", operation.summary, model.title);
    if handler.is_none() {
        return Some(descriptor_operation_definition(
            operation,
            operation_id,
            summary,
            false,
        ));
    }
    let mut definition = operation_definition_base(operation, operation_id, summary);

    match handler {
        None => unreachable!("non-general handlers return above"),
        Some(CoreGeneralOperationHandler::ListRecords) => {
            definition.insert("parameters".to_owned(), runtime_list_parameters());
            definition.insert("responses".to_owned(), runtime_responses(schema_ref, true));
        }
        Some(CoreGeneralOperationHandler::GetRecord) => {
            definition.insert(
                "parameters".to_owned(),
                json!([id_parameter(), expand_parameter()]),
            );
            definition.insert("responses".to_owned(), runtime_responses(schema_ref, false));
        }
        Some(CoreGeneralOperationHandler::CreateRecord) => {
            definition.insert(
                "requestBody".to_owned(),
                json_request_body(create_schema_ref),
            );
            definition.insert("responses".to_owned(), runtime_responses(schema_ref, false));
        }
        Some(CoreGeneralOperationHandler::UpdateRecord) => {
            definition.insert("parameters".to_owned(), json!([id_parameter()]));
            definition.insert(
                "requestBody".to_owned(),
                json_request_body(update_schema_ref),
            );
            definition.insert("responses".to_owned(), runtime_responses(schema_ref, false));
        }
        Some(CoreGeneralOperationHandler::DeleteRecord) => {
            definition.insert("parameters".to_owned(), json!([id_parameter()]));
            definition.insert("responses".to_owned(), runtime_delete_responses());
        }
    }
    Some(Value::Object(definition))
}

fn operation_definition_base(
    operation: &DataModelTemplateOperation,
    operation_id: String,
    summary: String,
) -> serde_json::Map<String, Value> {
    serde_json::Map::from_iter([
        ("operationId".to_owned(), Value::String(operation_id)),
        ("summary".to_owned(), Value::String(summary)),
        (
            "description".to_owned(),
            Value::String(operation.description.clone()),
        ),
        ("security".to_owned(), json!([{ "patBearer": [] }])),
    ])
}

fn descriptor_operation_definition(
    operation: &DataModelTemplateOperation,
    operation_id: String,
    summary: String,
    include_model_code_parameter: bool,
) -> Value {
    let mut definition = operation_definition_base(operation, operation_id, summary);
    let mut parameters = operation
        .path
        .split('/')
        .filter_map(|segment| {
            segment
                .strip_prefix('{')
                .and_then(|value| value.strip_suffix('}'))
        })
        .filter(|parameter| include_model_code_parameter || *parameter != "model_code")
        .map(|parameter| {
            json!({
                "name": parameter,
                "in": "path",
                "required": true,
                "schema": { "type": "string" }
            })
        })
        .collect::<Vec<_>>();
    if matches!(
        operation.method,
        plugin_framework::DataModelOperationMethod::Get
    ) {
        let required = operation
            .input_schema
            .get("required")
            .and_then(Value::as_array);
        if let Some(properties) = operation
            .input_schema
            .get("properties")
            .and_then(Value::as_object)
        {
            parameters.extend(properties.iter().map(|(name, schema)| {
                json!({
                    "name": name,
                    "in": "query",
                    "required": required.is_some_and(|required| {
                        required.iter().any(|field| field.as_str() == Some(name))
                    }),
                    "schema": schema
                })
            }));
        }
    }
    if !parameters.is_empty() {
        definition.insert("parameters".to_owned(), Value::Array(parameters));
    }
    if !matches!(
        operation.method,
        plugin_framework::DataModelOperationMethod::Get
    ) {
        definition.insert(
            "requestBody".to_owned(),
            json!({
                "required": true,
                "content": {
                    "application/json": { "schema": operation.input_schema.clone() }
                }
            }),
        );
    }
    definition.insert(
        "responses".to_owned(),
        json!({
            "200": {
                "description": "Success",
                "content": {
                    "application/json": { "schema": operation.output_schema.clone() }
                }
            },
            "400": { "description": "Invalid operation input" },
            "401": { "description": "Missing or invalid API key" },
            "403": { "description": "Operation permission or scope grant denied" },
            "404": { "description": "Data Model operation not found" },
            "409": { "description": "Data Model state or ordered-tree mutation conflict" },
            "503": { "description": "Ordered-tree adapter or query capability unavailable" },
            "502": { "description": "RuntimeExtension returned an invalid response" }
        }),
    );
    Value::Object(definition)
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
    fn data_model_template_descriptor_drives_catalog_openapi_and_system_fields() {
        let model = model_fixture();
        let templates = DataModelTemplateCatalog::core();
        let template =
            template_for_model(&model, &templates).expect("production template must resolve");
        let catalog = build_category_operations(std::slice::from_ref(&model), &templates);

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
            let operation_spec = build_operation_openapi(&model, &descriptor.code, &templates)
                .expect("every resolved handler must project OpenAPI");
            assert!(operation_spec["paths"][&catalog_operation.path]
                [descriptor.method.as_str().to_ascii_lowercase()]
            .is_object());
        }

        let spec = build_model_openapi(&model, &templates)
            .expect("production template must project OpenAPI");
        let record_properties = &spec["components"]["schemas"]["OrdersRecord"]["properties"];
        for field in &template.descriptor().system_fields {
            assert_eq!(record_properties[&field.code], field.value_schema);
        }
    }

    // AC-008/AC-011: matcher, capability catalog, and OpenAPI are projections of one descriptor.
    #[test]
    fn data_model_template_ordered_tree_routes_and_get_query_schemas_stay_aligned() {
        let mut model = model_fixture();
        model.template_code = "ordered_tree".to_owned();
        let templates = DataModelTemplateCatalog::core();
        let template = template_for_model(&model, &templates).expect("ordered tree must resolve");
        let catalog = build_category_operations(std::slice::from_ref(&model), &templates);

        assert_eq!(template.descriptor().operations.len(), 12);
        assert_eq!(catalog.operations.len(), 12);
        for descriptor in &template.descriptor().operations {
            let path = operation_path(&model, descriptor);
            let catalog_operation = catalog
                .operations
                .iter()
                .find(|operation| operation.id == operation_id(model.id, &descriptor.code))
                .expect("descriptor operation must be discoverable");
            assert_eq!(catalog_operation.method, descriptor.method.as_str());
            assert_eq!(catalog_operation.path, path);
            let matched = template
                .match_operation(
                    descriptor.method,
                    &path.replace("{id}", &Uuid::nil().to_string()),
                )
                .expect("documented operation must match the production descriptor router");
            assert_eq!(matched.operation.code, descriptor.code);

            let spec = build_operation_openapi(&model, &descriptor.code, &templates)
                .expect("descriptor operation must project OpenAPI");
            let operation = &spec["paths"][&path][descriptor.method.as_str().to_ascii_lowercase()];
            assert_eq!(
                operation["summary"],
                format!("{} — Orders", descriptor.summary)
            );
            assert_eq!(operation["description"], descriptor.description);
        }

        let descendants = build_operation_openapi(&model, "tree_descendants", &templates)
            .expect("descendants docs must exist");
        let parameters = descendants["paths"]["/api/runtime/models/orders/tree/descendants/{id}"]
            ["get"]["parameters"]
            .as_array()
            .expect("GET schema properties must project as parameters");
        for query_name in ["max_depth", "limit", "include_path"] {
            assert!(parameters.iter().any(|parameter| {
                parameter["name"] == query_name && parameter["in"] == "query"
            }));
        }
        let search = build_operation_openapi(&model, "tree_search", &templates).unwrap();
        assert!(
            search["paths"]["/api/runtime/models/orders/tree/search"]["get"]["parameters"]
                .as_array()
                .unwrap()
                .iter()
                .any(|parameter| parameter["name"] == "prefix" && parameter["required"] == true)
        );
    }

    #[test]
    fn data_model_template_global_openapi_uses_descriptor_route_inventory_only() {
        let templates = DataModelTemplateCatalog::core();
        let mut document = json!({ "paths": {} });

        append_template_runtime_openapi_paths(&mut document, &templates);

        let paths = document["paths"].as_object().unwrap();
        for template in templates.templates() {
            for operation in &template.descriptor().operations {
                assert!(
                    paths[&operation.path][operation.method.as_str().to_ascii_lowercase()]
                        .is_object(),
                    "descriptor operation must appear in global OpenAPI: {} {}",
                    operation.method.as_str(),
                    operation.path
                );
            }
        }
        assert_eq!(
            paths.len(),
            12,
            "general CRUD paths are shared by ordered tree"
        );
        assert!(paths.keys().all(|path| {
            !["convert", "migrate", "copy"]
                .iter()
                .any(|forbidden| path.contains(forbidden))
        }));
        assert_eq!(
            paths["/api/runtime/models/{model_code}/list"]["get"]["x-data-model-templates"]
                .as_array()
                .unwrap()
                .len(),
            2,
            "shared CRUD routes must retain both descriptor owners"
        );
    }

    #[test]
    fn unknown_template_identity_is_excluded_from_dynamic_docs() {
        let mut model = model_fixture();
        model.template_provider = "missing".to_owned();
        let templates = DataModelTemplateCatalog::core();

        assert!(build_category(&[model.clone()], &templates).is_none());
        assert!(build_category_operations(&[model.clone()], &templates)
            .operations
            .is_empty());
        assert!(build_model_openapi(&model, &templates).is_none());
    }

    #[test]
    fn ac_013_external_custom_operation_uses_canonical_openapi_and_permission_metadata() {
        let mut model = model_fixture();
        model.source_kind = domain::DataModelSourceKind::ExternalSource;
        model.data_source_instance_id = Some(Uuid::now_v7());
        model.external_resource_key = Some("contacts".into());
        model.template_provider = "acme_source".into();
        model.template_code = "contact_archive".into();
        model.template_version = "v1".into();
        let templates = DataModelTemplateCatalog::core();
        templates
            .replace_provider(
                "acme_source@1.0.0",
                "acme_source",
                vec![serde_json::from_value(json!({
                    "descriptor_version": 1,
                    "identity": { "provider": "acme_source", "code": "contact_archive", "version": "v1" },
                    "source_selector": { "kind": "external_provider", "provider": "acme_source" },
                    "required_capabilities": [{ "code": "update_record" }],
                    "system_fields": [{
                        "code": "id", "value_schema": { "type": "string" }, "required": true,
                        "write_policy": "read_only_projection", "summary": "ID", "description": "External ID."
                    }],
                    "operations": [{
                        "code": "archive_contact", "method": "POST",
                        "path": "/api/runtime/models/{model_code}/archive/{id}",
                        "input_schema": { "type": "object", "properties": { "reason": { "type": "string" } } },
                        "output_schema": { "type": "object", "required": ["archived"], "properties": { "archived": { "type": "boolean" } } },
                        "permission_action": "update",
                        "handler_ref": { "provider": "acme_source", "code": "archive_contact", "version": "v1" },
                        "summary": "Archive", "description": "Archive one contact."
                    }],
                    "summary": "Contact archive", "description": "External archive template."
                }))
                .unwrap()],
            )
            .unwrap();

        let spec = build_operation_openapi(&model, "archive_contact", &templates).unwrap();
        let operation = &spec["paths"]["/api/runtime/models/orders/archive/{id}"]["post"];
        assert_eq!(
            operation["requestBody"]["content"]["application/json"]["schema"]["properties"]
                ["reason"]["type"],
            "string"
        );
        assert_eq!(
            operation["responses"]["200"]["content"]["application/json"]["schema"]["properties"]
                ["archived"]["type"],
            "boolean"
        );
        assert_eq!(
            operation["responses"]["403"]["description"],
            "Operation permission or scope grant denied"
        );
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
