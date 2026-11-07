use super::*;
use crate::{
    extension_bus::managed_projection::object_schema as object,
    routes::application_runtime::FlowRunResponse,
};
use serde_json::{json, Value};
fn text() -> Value {
    json!({"type":"string"})
}
fn nullable(schema: Value) -> Value {
    json!({"anyOf":[schema,{"type":"null"}]})
}
fn start_body_schema() -> Value {
    object([("input_payload", Value::Bool(true))])
}

pub(super) fn commands_input_schema() -> Value {
    json!({"oneOf":[
        object([("variant",json!({"const":"Start"})),("application_id",text()),("body",start_body_schema())]),
        object([("variant",json!({"const":"Resume"})),("application_id",text()),("run_id",text()),("body",object([("checkpoint_id",text()),("input_payload",Value::Bool(true))]))]),
        object([("variant",json!({"const":"Cancel"})),("application_id",text()),("run_id",text())]),
        object([("variant",json!({"const":"CompleteCallback"})),("application_id",text()),("callback_task_id",text()),("body",object([("response_payload",Value::Bool(true))]))]),
        object([("variant",json!({"const":"StartNode"})),("application_id",text()),("node_id",text()),("body",start_body_schema())]),
    ]})
}
pub(super) fn commands_input(input: &ApplicationRuntimeDebugCommandsInput) -> Value {
    match input {
        ApplicationRuntimeDebugCommandsInput::Start {
            application_id,
            body,
            ..
        } => {
            json!({"variant":"Start","application_id":application_id,"body":{"input_payload":body.input_payload}})
        }
        ApplicationRuntimeDebugCommandsInput::Resume {
            application_id,
            run_id,
            body,
            ..
        } => {
            json!({"variant":"Resume","application_id":application_id,"run_id":run_id,"body":{"checkpoint_id":body.checkpoint_id,"input_payload":body.input_payload}})
        }
        ApplicationRuntimeDebugCommandsInput::Cancel {
            application_id,
            run_id,
        } => json!({"variant":"Cancel","application_id":application_id,"run_id":run_id}),
        ApplicationRuntimeDebugCommandsInput::CompleteCallback {
            application_id,
            callback_task_id,
            body,
            ..
        } => {
            json!({"variant":"CompleteCallback","application_id":application_id,"callback_task_id":callback_task_id,"body":{"response_payload":body.response_payload}})
        }
        ApplicationRuntimeDebugCommandsInput::StartNode {
            application_id,
            node_id,
            body,
            ..
        } => {
            json!({"variant":"StartNode","application_id":application_id,"node_id":node_id,"body":{"input_payload":body.input_payload}})
        }
    }
}
fn flow_schema() -> Value {
    object([
        ("id", text()),
        ("application_id", text()),
        ("run_mode", text()),
        ("status", text()),
        ("title", text()),
        ("query", nullable(text())),
        ("model", nullable(text())),
        ("input_payload", Value::Bool(true)),
        ("output_payload", Value::Bool(true)),
        ("started_at", text()),
        ("finished_at", nullable(text())),
    ])
}
fn flow(value: &FlowRunResponse) -> Value {
    json!({"id":value.id,"application_id":value.application_id,"run_mode":value.run_mode,"status":value.status,
        "title":value.title,"query":value.query,"model":value.model,
        "input_payload":value.input_payload,"output_payload":value.output_payload,
        "started_at":value.started_at,"finished_at":value.finished_at})
}
pub(super) fn commands_output_schema() -> Value {
    json!({"oneOf":[
        object([("variant",json!({"const":"Run"})),("0",object([
            ("flow_run",flow_schema()),
            ("answer_snapshot",nullable(object([("text",text()),("complete",json!({"type":"boolean"})),("output_payload",Value::Bool(true))]))),
        ]))]),
        object([("variant",json!({"const":"Node"})),("0",object([
            ("flow_run",flow_schema()),
            ("node_run",object([("id",text()),("flow_run_id",text()),("node_id",text()),("node_type",text()),("status",text()),("input_payload",Value::Bool(true)),("output_payload",Value::Bool(true))])),
        ]))]),
    ]})
}
pub(super) fn commands_output(output: &ApplicationRuntimeDebugCommandsOutput) -> Value {
    match output {
        ApplicationRuntimeDebugCommandsOutput::Run(detail) => json!({"variant":"Run","0":{
            "flow_run":flow(&detail.flow_run),"answer_snapshot":detail.answer_snapshot.as_ref().map(|snapshot| json!({"text":snapshot.text,"complete":snapshot.complete,"output_payload":snapshot.output_payload})),
        }}),
        ApplicationRuntimeDebugCommandsOutput::Node(detail) => json!({"variant":"Node","0":{
            "flow_run":flow(&detail.flow_run),"node_run":{
                "id":detail.node_run.id,"flow_run_id":detail.node_run.flow_run_id,"node_id":detail.node_run.node_id,"node_type":detail.node_run.node_type,"status":detail.node_run.status,
                "input_payload":detail.node_run.input_payload,"output_payload":detail.node_run.output_payload,
            },
        }}),
    }
}
pub(super) fn stream_input_schema() -> Value {
    json!({"oneOf":[
        object([("variant",json!({"const":"Start"})),("application_id",text()),("body",start_body_schema())]),
        object([("variant",json!({"const":"Subscribe"})),("application_id",text()),("run_id",text()),("from_sequence",nullable(json!({"type":"integer"})))]),
    ]})
}
pub(super) fn stream_input(input: &ApplicationRuntimeDebugStreamInput) -> Value {
    match input {
        ApplicationRuntimeDebugStreamInput::Start {
            application_id,
            body,
            ..
        } => {
            json!({"variant":"Start","application_id":application_id,"body":{"input_payload":body.input_payload}})
        }
        ApplicationRuntimeDebugStreamInput::Subscribe {
            application_id,
            run_id,
            from_sequence,
        } => {
            json!({"variant":"Subscribe","application_id":application_id,"run_id":run_id,"from_sequence":from_sequence})
        }
    }
}
pub(super) fn stream_output_schema() -> Value {
    object([("application_id", text()), ("run_id", text())])
}
pub(super) fn stream_output(output: &ApplicationRuntimeDebugStreamOutput) -> Value {
    json!({"application_id":output.application_id,"run_id":output.run_id})
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "event_projection/_tests/events.rs"]
mod _tests;
