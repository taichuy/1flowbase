use super::*;

impl InterfaceContract for ApplicationRuntimeDebugStreamInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Start")),
                ("application_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        ("input_payload", mp::json_summary_schema()),
                        (
                            "document",
                            serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
                (
                    "stream_query",
                    mp::object_schema(&[
                        (
                            "from_sequence",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "last_event_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Subscribe")),
                ("application_id", mp::text_schema()),
                ("run_id", mp::text_schema()),
                (
                    "from_sequence",
                    serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Start {
                application_id: _field_application_id,
                body: _field_body,
                stream_query: _field_stream_query,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Start".to_owned())),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "input_payload",
                            mp::json_summary(&(_field_body).input_payload),
                        ),
                        (
                            "document",
                            match (&(_field_body).document).as_ref() {
                                Some(item) => mp::json_summary(item),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
                (
                    "stream_query",
                    mp::object_value(&[
                        (
                            "from_sequence",
                            match (&(_field_stream_query).from_sequence).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "last_event_id",
                            match (&(_field_stream_query).last_event_id).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::Subscribe {
                application_id: _field_application_id,
                run_id: _field_run_id,
                from_sequence: _field_from_sequence,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Subscribe".to_owned())),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "run_id",
                    serde_json::Value::String((_field_run_id).to_string()),
                ),
                (
                    "from_sequence",
                    match (_field_from_sequence).as_ref() {
                        Some(item) => serde_json::json!(*(item)),
                        None => serde_json::Value::Null,
                    },
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-application-runtime-debug-stream-input";
    const CONTRACT_VERSION: &'static str = "1";
}
