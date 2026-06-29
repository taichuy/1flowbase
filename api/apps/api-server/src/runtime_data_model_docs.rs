use control_plane::{errors::ControlPlaneError, model_definition::ModelDefinitionService};
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

#[derive(Clone, Copy)]
pub enum RuntimeDataModelDocsOperationKind {
    ListRecords,
    CreateRecord,
    GetRecord,
    UpdateRecord,
    DeleteRecord,
}

impl RuntimeDataModelDocsOperationKind {
    pub fn all() -> [Self; 5] {
        [
            Self::ListRecords,
            Self::CreateRecord,
            Self::GetRecord,
            Self::UpdateRecord,
            Self::DeleteRecord,
        ]
    }

    fn id_suffix(self) -> &'static str {
        match self {
            Self::ListRecords => "list_records",
            Self::CreateRecord => "create_record",
            Self::GetRecord => "get_record",
            Self::UpdateRecord => "update_record",
            Self::DeleteRecord => "delete_record",
        }
    }

    fn method(self) -> &'static str {
        match self {
            Self::ListRecords | Self::GetRecord => "GET",
            Self::CreateRecord => "POST",
            Self::UpdateRecord => "PATCH",
            Self::DeleteRecord => "DELETE",
        }
    }

    fn method_lowercase(self) -> &'static str {
        match self {
            Self::ListRecords | Self::GetRecord => "get",
            Self::CreateRecord => "post",
            Self::UpdateRecord => "patch",
            Self::DeleteRecord => "delete",
        }
    }

    fn summary(self, model: &domain::ModelDefinitionRecord) -> String {
        match self {
            Self::ListRecords => format!("List {} records", model.title),
            Self::CreateRecord => format!("Create {} record", model.title),
            Self::GetRecord => format!("Get {} record", model.title),
            Self::UpdateRecord => format!("Update {} record", model.title),
            Self::DeleteRecord => format!("Delete {} record", model.title),
        }
    }

    fn description(self, model: &domain::ModelDefinitionRecord) -> String {
        match self {
            Self::ListRecords => format!(
                "Runtime list API for Data Model `{}` with concrete path registration.",
                model.code
            ),
            Self::CreateRecord => format!(
                "Runtime create API for Data Model `{}` with concrete path registration.",
                model.code
            ),
            Self::GetRecord => format!(
                "Runtime fetch API for Data Model `{}` with concrete path registration.",
                model.code
            ),
            Self::UpdateRecord => format!(
                "Runtime update API for Data Model `{}` with concrete path registration.",
                model.code
            ),
            Self::DeleteRecord => format!(
                "Runtime delete API for Data Model `{}` with concrete path registration.",
                model.code
            ),
        }
    }
}

pub fn operation_id(model_id: uuid::Uuid, kind: RuntimeDataModelDocsOperationKind) -> String {
    format!(
        "{DATA_MODEL_OPERATION_ID_PREFIX}{model_id}__{}",
        kind.id_suffix()
    )
}

pub fn parse_operation_id(
    operation_id: &str,
) -> Result<Option<(uuid::Uuid, RuntimeDataModelDocsOperationKind)>, &'static str> {
    let Some(rest) = operation_id.strip_prefix(DATA_MODEL_OPERATION_ID_PREFIX) else {
        return Ok(None);
    };
    let Some((model_id, suffix)) = rest.split_once("__") else {
        return Err("missing operation suffix");
    };
    let model_id = uuid::Uuid::parse_str(model_id).map_err(|_| "invalid model id")?;
    let kind = match suffix {
        "list_records" => RuntimeDataModelDocsOperationKind::ListRecords,
        "create_record" => RuntimeDataModelDocsOperationKind::CreateRecord,
        "get_record" => RuntimeDataModelDocsOperationKind::GetRecord,
        "update_record" => RuntimeDataModelDocsOperationKind::UpdateRecord,
        "delete_record" => RuntimeDataModelDocsOperationKind::DeleteRecord,
        _ => return Err("unknown operation kind"),
    };
    Ok(Some((model_id, kind)))
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
        .filter(|model| model.api_exposure_status == domain::ApiExposureStatus::ApiExposedReady)
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
    if model.api_exposure_status != domain::ApiExposureStatus::ApiExposedReady {
        return Ok(None);
    }
    Ok(Some(model))
}

pub fn list_path(model: &domain::ModelDefinitionRecord) -> String {
    format!("/api/runtime/models/{}/list", model.code)
}

pub fn get_path(model: &domain::ModelDefinitionRecord) -> String {
    format!("/api/runtime/models/{}/get/{{id}}", model.code)
}

pub fn create_path(model: &domain::ModelDefinitionRecord) -> String {
    format!("/api/runtime/models/{}/create", model.code)
}

pub fn update_path(model: &domain::ModelDefinitionRecord) -> String {
    format!("/api/runtime/models/{}/update/{{id}}", model.code)
}

pub fn delete_path(model: &domain::ModelDefinitionRecord) -> String {
    format!("/api/runtime/models/{}/delete/{{id}}", model.code)
}

pub fn operation_path(
    model: &domain::ModelDefinitionRecord,
    kind: RuntimeDataModelDocsOperationKind,
) -> String {
    match kind {
        RuntimeDataModelDocsOperationKind::ListRecords => list_path(model),
        RuntimeDataModelDocsOperationKind::CreateRecord => create_path(model),
        RuntimeDataModelDocsOperationKind::GetRecord => get_path(model),
        RuntimeDataModelDocsOperationKind::UpdateRecord => update_path(model),
        RuntimeDataModelDocsOperationKind::DeleteRecord => delete_path(model),
    }
}

pub fn build_category(models: &[domain::ModelDefinitionRecord]) -> Option<DocsCatalogCategory> {
    if models.is_empty() {
        return None;
    }
    Some(DocsCatalogCategory {
        id: DATA_MODEL_DOCS_CATEGORY_ID.to_string(),
        label: DATA_MODEL_DOCS_CATEGORY_LABEL.to_string(),
        operation_count: models.len() * RuntimeDataModelDocsOperationKind::all().len(),
    })
}

pub fn build_category_operations(
    models: &[domain::ModelDefinitionRecord],
) -> DocsCatalogCategoryOperations {
    let mut operations =
        Vec::with_capacity(models.len() * RuntimeDataModelDocsOperationKind::all().len());
    for model in models {
        let group = if model.title.is_empty() {
            model.code.clone()
        } else {
            model.title.clone()
        };
        for kind in RuntimeDataModelDocsOperationKind::all() {
            operations.push(DocsCatalogOperation {
                id: operation_id(model.id, kind),
                method: kind.method().to_string(),
                path: operation_path(model, kind),
                summary: Some(kind.summary(model)),
                description: Some(kind.description(model)),
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

pub fn build_model_openapi(model: &domain::ModelDefinitionRecord) -> Value {
    let list_path = list_path(model);
    let get_path = get_path(model);
    let create_path = create_path(model);
    let update_path = update_path(model);
    let delete_path = delete_path(model);
    let schema_name = record_schema_name(&model.code);
    let create_schema_name = format!("{schema_name}CreateInput");
    let update_schema_name = format!("{schema_name}UpdateInput");
    let schema_ref = format!("#/components/schemas/{schema_name}");
    let create_schema_ref = format!("#/components/schemas/{create_schema_name}");
    let update_schema_ref = format!("#/components/schemas/{update_schema_name}");

    json!({
        "openapi": "3.1.0",
        "info": {
            "title": format!("{} Data Model API", model.title),
            "version": "1.0.0"
        },
        "security": [{ "patBearer": [] }],
        "paths": {
            list_path: {
                "get": {
                    "operationId": format!("list_{}_records", model.code),
                    "summary": format!("List {} records", model.title),
                    "description": "List records with filter, sort, pagination, and relation expansion. PAT uses bound user role permissions plus an enabled scope grant.",
                    "security": [{ "patBearer": [] }],
                    "parameters": runtime_list_parameters(),
                    "responses": runtime_responses(&schema_ref, true)
                }
            },
            create_path: {
                "post": {
                    "operationId": format!("create_{}_record", model.code),
                    "summary": format!("Create {} record", model.title),
                    "description": "Create a record. PAT uses bound user role permissions, scope permission, and audit logging.",
                    "security": [{ "patBearer": [] }],
                    "requestBody": json_request_body(&create_schema_ref),
                    "responses": runtime_responses(&schema_ref, false)
                }
            },
            get_path: {
                "get": {
                    "operationId": format!("get_{}_record", model.code),
                    "summary": format!("Get {} record", model.title),
                    "security": [{ "patBearer": [] }],
                    "parameters": [id_parameter(), expand_parameter()],
                    "responses": runtime_responses(&schema_ref, false)
                }
            },
            update_path: {
                "patch": {
                    "operationId": format!("update_{}_record", model.code),
                    "summary": format!("Update {} record", model.title),
                    "description": "Update a record. PAT uses bound user role permissions, scope permission, and audit logging.",
                    "security": [{ "patBearer": [] }],
                    "parameters": [id_parameter()],
                    "requestBody": json_request_body(&update_schema_ref),
                    "responses": runtime_responses(&schema_ref, false)
                }
            },
            delete_path: {
                "delete": {
                    "operationId": format!("delete_{}_record", model.code),
                    "summary": format!("Delete {} record", model.title),
                    "description": "Delete a record. PAT uses bound user role permissions, scope permission, and audit logging.",
                    "security": [{ "patBearer": [] }],
                    "parameters": [id_parameter()],
                    "responses": runtime_delete_responses()
                }
            }
        },
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
                schema_name: record_schema(model),
                create_schema_name: record_write_schema(model, true),
                update_schema_name: record_write_schema(model, false)
            }
        },
        "x-data-model": {
            "id": model.id.to_string(),
            "code": model.code,
            "status": model.status.as_str(),
            "api_exposure_status": model.api_exposure_status.as_str(),
            "source_kind": model.source_kind.as_str(),
            "protected": model.protection.is_protected
        },
        "x-scope-permission-note": "Runtime Data Model APIs accept pat_ user API keys with bound user role permissions and require an enabled owner or scope_all scope grant for the request scope.",
        "x-external-source-safety-limits": external_source_safety_limits(model)
    })
}

pub fn build_category_openapi(models: &[domain::ModelDefinitionRecord]) -> Value {
    let mut paths = serde_json::Map::new();
    let mut schemas = serde_json::Map::new();
    for model in models {
        let spec = build_model_openapi(model);
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
    kind: RuntimeDataModelDocsOperationKind,
) -> Value {
    let full_spec = build_model_openapi(model);
    let path = operation_path(model, kind);
    let method = kind.method_lowercase();
    let operation = full_spec
        .get("paths")
        .and_then(Value::as_object)
        .and_then(|paths| paths.get(&path))
        .and_then(Value::as_object)
        .and_then(|path_item| path_item.get(method))
        .cloned()
        .unwrap_or(Value::Null);
    let mut path_item = serde_json::Map::new();
    path_item.insert(method.to_string(), operation);
    let mut paths = serde_json::Map::new();
    paths.insert(path, Value::Object(path_item));

    json!({
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
    })
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
                "data": {
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

fn record_schema(model: &domain::ModelDefinitionRecord) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for field in &model.fields {
        properties.insert(field.code.clone(), field_schema(field));
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

fn record_write_schema(model: &domain::ModelDefinitionRecord, include_required: bool) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for field in &model.fields {
        properties.insert(field.code.clone(), field_schema(field));
        if include_required && field.is_required {
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
