use super::*;
use crate::extension_bus::managed_projection::object_schema as object;
use serde_json::{json, Value};

fn text() -> Value {
    json!({"type":"string"})
}
fn nullable(schema: Value) -> Value {
    json!({"anyOf":[schema,{"type":"null"}]})
}

pub(super) fn input_schema() -> Value {
    object([
        (
            "request",
            object([
                ("query", text()),
                ("model", nullable(text())),
                (
                    "inputs",
                    json!({"type":"object","additionalProperties":true}),
                ),
                ("history", json!({"type":"array","items":true})),
                (
                    "system",
                    json!({"type":"array","items":object([("text",text())])}),
                ),
                ("title", nullable(text())),
                ("expand_id", nullable(text())),
                ("response_mode", nullable(text())),
            ]),
        ),
        (
            "protocol",
            json!({"type":"string","enum":["native","open_ai_chat","open_ai_responses","anthropic_messages"]}),
        ),
    ])
}
pub(super) fn input(input: &ApplicationNativeRunInput) -> Value {
    let request = &input.request;
    json!({"request":{
            "query":request.query,"model":request.model,"inputs":request.inputs,"history":request.history,
            "system":request.system.iter().map(|block| json!({"text":block.text_content()})).collect::<Vec<_>>(),
            "title":request.title,"expand_id":request.expand_id,"response_mode":request.response_mode,
        },"protocol": input.protocol
    })
}
pub(super) fn output_schema() -> Value {
    object([(
        "0",
        object([
            ("id", text()),
            ("application_id", text()),
            ("publication_version_id", text()),
            (
                "status",
                json!({"type":"string","enum":["created","queued","running","waiting","succeeded","incomplete","failed","cancelled"]}),
            ),
            ("answer", nullable(text())),
            (
                "answer_segments",
                nullable(json!({"type":"array","items":object([
                    ("kind",json!({"type":"string","enum":["reasoning","message"]})),("text",text())
                ])})),
            ),
            (
                "error",
                nullable(object([("code", text()), ("message", text())])),
            ),
            ("created_at", json!({"type":"integer"})),
        ]),
    )])
}
pub(super) fn output(output: &ApplicationNativeRunOutput) -> Value {
    let output = &output.0;
    json!({"0":{
        "id":output.id,"application_id":output.application_id,"publication_version_id":output.publication_version_id,
        "status":output.status,"answer":output.answer,
        "answer_segments":output.answer_segments.as_ref().map(|segments| segments.iter().map(|segment| json!({
            "kind":match segment.kind {
                orchestration_runtime::answer_projection::AnswerProjectionSegmentKind::Reasoning => "reasoning",
                orchestration_runtime::answer_projection::AnswerProjectionSegmentKind::Message => "message",
            }, "text":segment.text,
        })).collect::<Vec<_>>()),
        "error":output.error.as_ref().map(|error| json!({"code":error.code,"message":error.message})),
        "created_at":output.created_at.unix_timestamp(),
    }})
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "event_projection/_tests/events.rs"]
mod _tests;
