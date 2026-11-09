use super::*;
use crate::extension_bus::managed_projection::object_schema as object;

pub(super) const OPERATIONS: &[&str] = &[
    "communications.binding.get",
    "communications.binding.update",
    "communications.messages.retry",
    "communications.messages.list",
    "communications.management.messages.list",
    "communications.management.messages.get",
    "communications.management.conversations.messages.list",
];
fn text() -> Value {
    json!({"type":"string"})
}
fn nullable(schema: Value) -> Value {
    json!({"anyOf":[schema,{"type":"null"}]})
}
fn binding_schema() -> Value {
    // Explicitly selected business configuration; no provider config or authorization state.
    object([
        ("application_id", text()),
        ("model", nullable(text())),
        ("reasoning_effort", nullable(text())),
        ("file_table_id", nullable(text())),
        ("mcp_instance_ids", json!({"type":"array","items":text()})),
        ("system_prompt", text()),
    ])
}
fn binding_input(input: &domain::CommunicationBindingInput) -> Value {
    json!({"application_id":input.application_id,"model":input.model,"reasoning_effort":input.reasoning_effort,
        "file_table_id":input.file_table_id,"mcp_instance_ids":input.mcp_instance_ids,"system_prompt":input.system_prompt})
}
fn query_schema() -> Value {
    object([
        ("cursor", nullable(text())),
        ("limit", nullable(json!({"type":"integer","minimum":0}))),
    ])
}
fn query(query: &PageQuery) -> Value {
    json!({"cursor":query.cursor,"limit":query.limit})
}
pub(super) fn input_schema() -> Value {
    json!({"oneOf":[
        object([("operation",json!({"const":"binding"})),("instance_id",text())]),
        object([("operation",json!({"const":"save_binding"})),("instance_id",text()),("body",binding_schema())]),
        object([("operation",json!({"const":"retry"})),("instance_id",text()),("message_id",text())]),
        object([("operation",json!({"const":"messages"})),("instance_id",text()),("conversation_id",text()),("query",query_schema())]),
        object([("operation",json!({"const":"management_history"})),("conversation_id",text()),("query",query_schema())]),
        object([("operation",json!({"const":"management_message"})),("message_id",text())]),
        object([("operation",json!({"const":"management_messages"})),("query",object([
            ("instance_id",nullable(text())),("conversation_id",nullable(text())),
            ("processing_status",nullable(json!({"type":"string","enum":["pending","processing","succeeded","failed"]}))),
            ("send_status",nullable(json!({"type":"string","enum":["none","pending","sending","succeeded","failed","unknown"]}))),
            ("direction",nullable(json!({"type":"string","enum":["inbound","outbound"]}))),
            ("type",nullable(json!({"type":"string","enum":["text","file"]}))),
            ("page",nullable(json!({"type":"integer"}))), ("page_size",nullable(json!({"type":"integer"}))),
        ]))]),
    ]})
}
pub(super) fn input(input: &Input) -> Option<Value> {
    Some(match input {
        Input::Binding(instance_id) => json!({"operation":"binding","instance_id":instance_id}),
        Input::SaveBinding(instance_id, body) => {
            json!({"operation":"save_binding","instance_id":instance_id,"body":binding_input(body)})
        }
        Input::Retry(instance_id, message_id) => {
            json!({"operation":"retry","instance_id":instance_id,"message_id":message_id})
        }
        Input::Messages(instance_id, conversation_id, page) => {
            json!({"operation":"messages","instance_id":instance_id,"conversation_id":conversation_id,"query":query(page)})
        }
        Input::ManagementHistory(conversation_id, page) => {
            json!({"operation":"management_history","conversation_id":conversation_id,"query":query(page)})
        }
        Input::ManagementMessage(message_id) => {
            json!({"operation":"management_message","message_id":message_id})
        }
        Input::ManagementMessages(query) => json!({"operation":"management_messages","query":{
            "instance_id":query.instance_id,"conversation_id":query.conversation_id,
            "processing_status":query.processing_status,"send_status":query.send_status,"direction":query.direction,
            "type":query.message_type,"page":query.page,"page_size":query.page_size,
        }}),
        _ => return None,
    })
}
fn binding_record_schema() -> Value {
    let mut schema = binding_schema();
    let properties = schema["properties"].as_object_mut().expect("object schema");
    for key in [
        "workspace_id",
        "instance_id",
        "actor_user_id",
        "plugin_mcp_instance_id",
    ] {
        properties.insert(key.into(), text());
    }
    properties.insert("revision".into(), json!({"type":"integer"}));
    schema
}
fn message_schema() -> Value {
    let mut properties = serde_json::Map::new();
    for key in [
        "id",
        "conversation_id",
        "input_id",
        "instance_id",
        "type",
        "direction",
    ] {
        properties.insert(key.into(), text());
    }
    for key in [
        "instance_display_name",
        "external_identity",
        "sender_display_name",
        "content",
        "content_preview",
        "run_id",
        "application_id",
        "processing_status",
        "send_status",
        "failure_code",
    ] {
        properties.insert(key.into(), nullable(text()));
    }
    properties.insert("occurred_at_ms".into(), json!({"type":"integer"}));
    properties.insert("can_retry".into(), json!({"type":"boolean"}));
    properties.insert(
        "file".into(),
        nullable(object([
            ("file_name", text()),
            ("media_type", text()),
            ("size_bytes", nullable(json!({"type":"integer"}))),
            ("status", text()),
            ("file_table_id", nullable(text())),
            ("file_record_id", nullable(text())),
            ("failure_code", nullable(text())),
        ])),
    );
    json!({"type":"object","properties":properties,"additionalProperties":false})
}
pub(super) fn output_schema() -> Value {
    let mut schema = message_schema();
    let properties = schema["properties"].as_object_mut().expect("object schema");
    for (key, value) in binding_record_schema()["properties"]
        .as_object()
        .expect("object schema")
    {
        properties.insert(key.clone(), value.clone());
    }
    properties.insert("binding".into(), nullable(binding_record_schema()));
    properties.insert(
        "items".into(),
        json!({"type":"array","items":message_schema()}),
    );
    properties.insert("next_cursor".into(), nullable(text()));
    for key in ["total", "page", "page_size"] {
        properties.insert(key.into(), json!({"type":"integer"}));
    }
    schema
}
fn selected(value: &Value, keys: &[&str]) -> Value {
    Value::Object(
        keys.iter()
            .filter_map(|key| {
                value
                    .get(key)
                    .map(|value| ((*key).to_owned(), value.clone()))
            })
            .collect(),
    )
}
fn message(value: &Value) -> Value {
    let mut result = selected(
        value,
        &[
            "id",
            "conversation_id",
            "input_id",
            "instance_id",
            "instance_display_name",
            "external_identity",
            "sender_display_name",
            "type",
            "direction",
            "occurred_at_ms",
            "content",
            "content_preview",
            "run_id",
            "application_id",
            "processing_status",
            "send_status",
            "failure_code",
            "can_retry",
        ],
    );
    if let Some(file) = value.get("file") {
        result["file"] = if file.is_null() {
            Value::Null
        } else {
            selected(
                file,
                &[
                    "file_name",
                    "media_type",
                    "size_bytes",
                    "status",
                    "file_table_id",
                    "file_record_id",
                    "failure_code",
                ],
            )
        };
    }
    result
}
fn binding_record(value: &Value) -> Value {
    selected(
        value,
        &[
            "workspace_id",
            "instance_id",
            "actor_user_id",
            "application_id",
            "model",
            "reasoning_effort",
            "file_table_id",
            "mcp_instance_ids",
            "system_prompt",
            "plugin_mcp_instance_id",
            "revision",
        ],
    )
}
pub(super) fn output(output: &Output) -> Option<Value> {
    let value = &output.0;
    if let Some(binding) = value.get("binding") {
        Some(
            json!({"binding":if binding.is_null() { Value::Null } else { binding_record(binding) }}),
        )
    } else if value.get("revision").is_some() {
        Some(binding_record(value))
    } else if let Some(items) = value.get("items").and_then(Value::as_array) {
        let mut result = selected(value, &["next_cursor", "total", "page", "page_size"]);
        result["items"] = Value::Array(items.iter().map(message).collect());
        Some(result)
    } else if value.get("id").is_some() {
        Some(message(value))
    } else {
        None
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "event_projection/_tests/events.rs"]
mod _tests;
