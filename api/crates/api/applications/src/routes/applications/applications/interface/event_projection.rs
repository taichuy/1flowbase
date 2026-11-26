use super::{ApplicationSummaryResponse, ApplicationsInput, ApplicationsOutput};
use crate::extension_bus::managed_projection::object_schema as object;
use serde_json::{json, Value};

pub(super) const OPERATIONS: &[&str] = &[
    "applications.list",
    "applications.create",
    "applications.get",
    "applications.update",
    "applications.delete",
];
fn text() -> Value {
    json!({"type":"string"})
}
fn nullable(schema: Value) -> Value {
    json!({"anyOf":[schema,{"type":"null"}]})
}
fn application_type() -> Value {
    json!({"type":"string","enum":["agent_logs","agent_flow","workflow"]})
}
fn body_schema(create: bool) -> Value {
    let mut schema = object([
        ("name", text()),
        ("description", text()),
        ("icon", nullable(text())),
        ("icon_type", nullable(text())),
        ("icon_background", nullable(text())),
    ]);
    let properties = schema["properties"].as_object_mut().expect("object schema");
    if create {
        properties.insert("application_type".into(), application_type());
        properties.insert(
            "workflow_trigger_type".into(),
            nullable(json!({"type":"string","enum":["extension","schedule","event"]})),
        );
        properties.insert(
            "workflow_trigger_config".into(),
            nullable(object([
                ("cron", nullable(text())),
                ("timezone", nullable(text())),
                ("input_payload", Value::Bool(true)),
                ("subpath", nullable(text())),
                ("http_method", nullable(text())),
                ("response_mode", nullable(text())),
            ])),
        );
    } else {
        properties.insert("tag_ids".into(), json!({"type":"array","items":text()}));
    }
    let required = properties.keys().cloned().map(Value::String).collect();
    schema["required"] = Value::Array(required);
    schema
}
pub(super) fn input_schema() -> Value {
    json!({"oneOf":[
        object([("variant",json!({"const":"List"}))]),
        object([("variant",json!({"const":"Create"})),("0",body_schema(true))]),
        object([("variant",json!({"const":"Get"})),("application_id",text())]),
        object([("variant",json!({"const":"Patch"})),("application_id",text()),("body",body_schema(false))]),
        object([("variant",json!({"const":"Delete"})),("application_id",text())]),
    ]})
}
pub(super) fn input(input: &ApplicationsInput) -> Option<Value> {
    Some(match input {
        ApplicationsInput::List => json!({"variant":"List"}),
        ApplicationsInput::Create(body) => json!({"variant":"Create","0":{
            "application_type":body.application_type,"workflow_trigger_type":body.workflow_trigger_type,
            "workflow_trigger_config":body.workflow_trigger_config.as_ref().map(|config| json!({
                "cron":config.cron,"timezone":config.timezone,"input_payload":config.input_payload,
                "subpath":config.subpath,"http_method":config.http_method,"response_mode":config.response_mode,
            })),
            "name":body.name,"description":body.description,"icon":body.icon,"icon_type":body.icon_type,"icon_background":body.icon_background,
        }}),
        ApplicationsInput::Get { application_id } => {
            json!({"variant":"Get","application_id":application_id})
        }
        ApplicationsInput::Patch {
            application_id,
            body,
        } => json!({"variant":"Patch","application_id":application_id,"body":{
            "name":body.name,"description":body.description,"tag_ids":body.tag_ids,
            "icon":body.icon,"icon_type":body.icon_type,"icon_background":body.icon_background,
        }}),
        ApplicationsInput::Delete { application_id } => {
            json!({"variant":"Delete","application_id":application_id})
        }
        _ => return None,
    })
}
fn application_schema(detail: bool) -> Value {
    let mut schema = object([
        ("id", text()),
        ("application_type", application_type()),
        ("name", text()),
        ("description", text()),
        ("icon", nullable(text())),
        ("icon_type", nullable(text())),
        ("icon_background", nullable(text())),
        ("created_by", text()),
        ("updated_at", text()),
        (
            "tags",
            json!({"type":"array","items":object([("id",text()),("name",text())])}),
        ),
    ]);
    if detail {
        schema["properties"]["workflow_trigger_type"] =
            nullable(json!({"type":"string","enum":["extension","schedule","event"]}));
        schema["required"]
            .as_array_mut()
            .expect("object required fields")
            .push(json!("workflow_trigger_type"));
    }
    schema
}
pub(super) fn output_schema() -> Value {
    json!({"oneOf":[
        object([("variant",json!({"const":"Applications"})),("0",json!({"type":"array","items":application_schema(false)}))]),
        object([("variant",json!({"const":"Application"})),("0",application_schema(true))]),
        object([("variant",json!({"const":"NoContent"}))]),
    ]})
}
fn summary(value: &ApplicationSummaryResponse) -> Value {
    json!({"id":value.id,"application_type":value.application_type,"name":value.name,"description":value.description,
        "icon":value.icon,"icon_type":value.icon_type,"icon_background":value.icon_background,
        "created_by":value.created_by,"updated_at":value.updated_at,"tags":value.tags})
}
pub(super) fn output(output: &ApplicationsOutput) -> Option<Value> {
    Some(match output {
        ApplicationsOutput::Applications(values) => {
            json!({"variant":"Applications","0":values.iter().map(summary).collect::<Vec<_>>()})
        }
        ApplicationsOutput::Application(value) => json!({"variant":"Application","0":{
            "workflow_trigger_type":value.workflow_trigger_type,
            "id":value.id,"application_type":value.application_type,"name":value.name,"description":value.description,
            "icon":value.icon,"icon_type":value.icon_type,"icon_background":value.icon_background,
            "created_by":value.created_by,"updated_at":value.updated_at,"tags":value.tags,
        }}),
        ApplicationsOutput::NoContent => json!({"variant":"NoContent"}),
        _ => return None,
    })
}
