use super::*;

impl InterfaceContract for ApplicationRuntimeDebugCommandsInput {
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
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Resume")),
                ("application_id", mp::text_schema()),
                ("run_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        ("checkpoint_id", mp::text_schema()),
                        ("input_payload", mp::json_summary_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Cancel")),
                ("application_id", mp::text_schema()),
                ("run_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CompleteCallback")),
                ("application_id", mp::text_schema()),
                ("callback_task_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[("response_payload", mp::json_summary_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("StartNode")),
                ("application_id", mp::text_schema()),
                ("node_id", mp::text_schema()),
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
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Start {
                application_id: _field_application_id,
                body: _field_body,
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
            ]),
            Self::Resume {
                application_id: _field_application_id,
                run_id: _field_run_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Resume".to_owned())),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "run_id",
                    serde_json::Value::String((_field_run_id).to_string()),
                ),
                (
                    "body",
                    mp::object_value(&[
                        ("checkpoint_id", mp::text(&(_field_body).checkpoint_id)?),
                        (
                            "input_payload",
                            mp::json_summary(&(_field_body).input_payload),
                        ),
                    ]),
                ),
            ]),
            Self::Cancel {
                application_id: _field_application_id,
                run_id: _field_run_id,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Cancel".to_owned())),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "run_id",
                    serde_json::Value::String((_field_run_id).to_string()),
                ),
            ]),
            Self::CompleteCallback {
                application_id: _field_application_id,
                callback_task_id: _field_callback_task_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CompleteCallback".to_owned()),
                ),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "callback_task_id",
                    serde_json::Value::String((_field_callback_task_id).to_string()),
                ),
                (
                    "body",
                    mp::object_value(&[(
                        "response_payload",
                        mp::json_summary(&(_field_body).response_payload),
                    )]),
                ),
            ]),
            Self::StartNode {
                application_id: _field_application_id,
                node_id: _field_node_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("StartNode".to_owned())),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                ("node_id", mp::text(_field_node_id)?),
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
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-application-runtime-debug-commands-input";
    const CONTRACT_VERSION: &'static str = "1";
}
