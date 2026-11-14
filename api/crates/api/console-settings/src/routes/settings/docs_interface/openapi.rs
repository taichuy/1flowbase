//! Host documentation for SharedApiDocument's immutable JSON value. ScopePlan::materialize owns
//! the finite outer fields; selected path operations and component nodes remain canonical Value
//! documents, including arbitrary JSON Schema values. No HTTP bytes are decoded for this schema.
use serde_json::{json, Value};

pub struct ConsoleOpenApiDocumentSchema;
impl utoipa::ToSchema for ConsoleOpenApiDocumentSchema {}
impl utoipa::PartialSchema for ConsoleOpenApiDocumentSchema {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        serde_json::from_value(document_schema()).expect("static scoped OpenAPI document schema")
    }
}
fn text() -> Value {
    json!({"type":"string"})
}
fn array(items: Value) -> Value {
    json!({"type":"array","items":items})
}
fn map(values: Value) -> Value {
    json!({"type":"object","additionalProperties":values})
}
fn dynamic(description: &str) -> Value {
    json!({"type":["null","boolean","number","string","array","object"],"description":description})
}
fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","properties":properties,"required":required})
}
fn external_docs() -> Value {
    object(json!({"description":text(),"url":text()}), &["url"])
}
fn server_schema() -> Value {
    object(
        json!({"url":text(),"description":text(),"variables":map(object(json!({"enum":array(text()),"default":text(),"description":text()}),&["default"]))}),
        &["url"],
    )
}
fn path_item_schema() -> Value {
    let mut properties = json!({"$ref":text(),"summary":text(),"description":text(),"servers":array(server_schema()),
        "parameters":array(dynamic("Canonical OpenAPI parameter or reference node copied by the selected scope."))});
    for method in [
        "get", "put", "post", "delete", "options", "head", "patch", "trace",
    ] {
        properties[method]=dynamic("Canonical OpenAPI operation node selected from the shared document, preserving its request/response schemas and extensions.");
    }
    object(properties, &[])
}
fn document_schema() -> Value {
    let mut component_properties = serde_json::Map::new();
    for kind in [
        "schemas",
        "responses",
        "parameters",
        "examples",
        "requestBodies",
        "headers",
        "securitySchemes",
        "links",
        "callbacks",
        "pathItems",
    ] {
        component_properties.insert(kind.into(),map(dynamic("Canonical component Value copied with the selected document's complete local reference closure.")));
    }
    let mut document = object(
        json!({
            "openapi":text(),
            "info":object(json!({"title":text(),"version":text(),"summary":text(),"description":text(),"termsOfService":text(),
                "contact":object(json!({"name":text(),"url":text(),"email":text()}),&[]),
                "license":object(json!({"name":text(),"url":text(),"identifier":text()}),&["name"])}),&["title","version"]),
            "paths":map(path_item_schema()),"components":object(Value::Object(component_properties),&[]),
            "servers":array(server_schema()),"security":array(map(array(text()))),
            "tags":array(object(json!({"name":text(),"description":text(),"externalDocs":external_docs()}),&["name"]))
        }),
        &["openapi", "info", "paths", "components"],
    );
    document["additionalProperties"] = json!(false);
    document
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/openapi.rs"]
mod tests;
