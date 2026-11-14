use super::*;

pub(super) fn runtime_list_parameters() -> Value {
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

pub(super) fn id_parameter() -> Value {
    json!({
        "name": "id",
        "in": "path",
        "required": true,
        "schema": { "type": "string", "format": "uuid" }
    })
}

pub(super) fn expand_parameter() -> Value {
    json!({
        "name": "expand",
        "in": "query",
        "required": false,
        "schema": { "type": "string" },
        "example": "customer,items",
        "description": "Comma-separated relation field codes to expand."
    })
}

pub(super) fn json_request_body(schema_ref: &str) -> Value {
    json!({
        "required": true,
        "content": {
            "application/json": {
                "schema": { "$ref": schema_ref }
            }
        }
    })
}

pub(super) fn json_schema_request_body(schema: Value) -> Value {
    json!({
        "required": true,
        "content": {
            "application/json": { "schema": schema }
        }
    })
}

pub(super) fn ordered_tree_write_schema(
    model: &domain::ModelDefinitionRecord,
    operation: &DataModelTemplateOperation,
    include_required: bool,
) -> Value {
    let mut schema = record_write_schema(model, include_required);
    let Some(schema_object) = schema.as_object_mut() else {
        return schema;
    };
    if let Some(operation_properties) = operation
        .input_schema
        .get("properties")
        .and_then(Value::as_object)
    {
        let properties = schema_object
            .entry("properties")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .expect("record write schema properties must be an object");
        properties.extend(operation_properties.clone());
    }
    if let Some(operation_required) = operation
        .input_schema
        .get("required")
        .and_then(Value::as_array)
    {
        let required = schema_object
            .entry("required")
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .expect("record write schema required must be an array");
        for field in operation_required {
            if !required.contains(field) {
                required.push(field.clone());
            }
        }
    }
    schema_object.insert("additionalProperties".to_owned(), Value::Bool(false));
    schema
}

pub(super) fn runtime_responses(schema_ref: &str, list: bool) -> Value {
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

pub(super) fn runtime_delete_responses() -> Value {
    json!({
        "200": {
            "description": "Deleted",
            "content": {
                "application/json": {
                    "schema": {
                        "type": "object",
                        "required": ["deleted"],
                        "properties": { "deleted": { "type": "boolean" } }
                    }
                }
            }
        },
        "401": { "description": "Missing or invalid API key" },
        "403": { "description": "API key, action permission, or scope grant denied" },
        "404": { "description": "Data Model or record not found" },
        "409": { "description": "Data Model is not published, disabled, broken, or unsafe" }
    })
}

pub(super) fn record_schema(
    model: &domain::ModelDefinitionRecord,
    template: &CompiledDataModelTemplate,
) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for field in template.descriptor().system_fields.iter().filter(|field| {
        !model.protection.is_protected
            || model.fields.iter().any(|model_field| {
                model_field.code == field.code || model_field.physical_column_name == field.code
            })
    }) {
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

pub(super) fn record_field_schema(field: &domain::ModelFieldRecord) -> Value {
    if field.field_kind == domain::ModelFieldKind::ManyToMany {
        return json!({"type":"array", "items":{"anyOf":[{"type":"string","format":"uuid"},{"type":"object"}]}, "description":"Related record IDs, or records when expanded; empty relations return []."});
    }
    let schema = field_schema(field);
    if field.is_required {
        schema
    } else {
        json!({ "anyOf": [schema, { "type": "null" }] })
    }
}

pub(super) fn record_write_schema(
    model: &domain::ModelDefinitionRecord,
    include_required: bool,
) -> Value {
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

pub(super) fn field_schema(field: &domain::ModelFieldRecord) -> Value {
    match field.field_kind {
        domain::ModelFieldKind::FilePreviewUrl => json!({"type":["string","null"],"readOnly":true}),
        domain::ModelFieldKind::Number => json!({ "type": "number" }),
        domain::ModelFieldKind::Boolean => json!({ "type": "boolean" }),
        domain::ModelFieldKind::Datetime => {
            json!({ "type": "string", "format": "date-time" })
        }
        // JSON fields may contain arrays or scalars as well as objects.
        domain::ModelFieldKind::Json => Value::Bool(true),
        domain::ModelFieldKind::ManyToMany => json!({
            "type": "array", "items": {"type":"string", "format":"uuid"},
            "description": "Replace the relation set with these record IDs. [] clears it; omission on update preserves it."
        }),
        domain::ModelFieldKind::ManyToOne | domain::ModelFieldKind::OneToMany => json!({
            "type": "string",
            "format": "uuid",
            "description": "Relation record id or relation expansion target."
        }),
        domain::ModelFieldKind::String
        | domain::ModelFieldKind::Enum
        | domain::ModelFieldKind::Text => json!({ "type": "string" }),
    }
}

pub(super) fn external_source_safety_limits(model: &domain::ModelDefinitionRecord) -> String {
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

pub(super) fn record_schema_name(code: &str) -> String {
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
