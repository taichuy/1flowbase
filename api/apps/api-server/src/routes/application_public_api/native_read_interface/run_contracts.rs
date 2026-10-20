use super::*;

impl InterfaceContract for NativeGetRunInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[("0", mp::text_schema())]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "0",
            serde_json::Value::String((self).0.to_string()),
        )]))
    }

    const CONTRACT_ID: &'static str = "application-native-run-get-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct NativeGetRunOutput(pub(crate) NativeRunResponse);

impl InterfaceContract for NativeGetRunOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "0",
            mp::object_schema(&[
                ("id", mp::text_schema()),
                ("application_id", mp::text_schema()),
                ("publication_version_id", mp::text_schema()),
                ("status", mp::text_schema()),
                ("node_input_payload", mp::json_summary_schema()),
                ("metadata", mp::json_summary_schema()),
                (
                    "answer",
                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                ),
                (
                    "answer_segments",
                    serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                ),
                (
                    "required_action",
                    serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                ),
                (
                    "tool_calls",
                    serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                ),
                (
                    "usage",
                    serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                ),
                (
                    "error",
                    serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                ),
                (
                    "operation_terminal",
                    serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                ),
                ("created_at", mp::text_schema()),
            ]),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "0",
            mp::object_value(&[
                ("id", serde_json::Value::String((self).0.id.to_string())),
                (
                    "application_id",
                    serde_json::Value::String((self).0.application_id.to_string()),
                ),
                (
                    "publication_version_id",
                    serde_json::Value::String((self).0.publication_version_id.to_string()),
                ),
                ("status", mp::text(&(self).0.status)?),
                (
                    "node_input_payload",
                    mp::json_summary(&(self).0.node_input_payload),
                ),
                ("metadata", mp::json_summary(&(self).0.metadata)),
                (
                    "answer",
                    match (self).0.answer.as_ref() {
                        Some(item) => {
                            mp::object_value(&[("byte_count", serde_json::json!((item).len()))])
                        }
                        None => serde_json::Value::Null,
                    },
                ),
                (
                    "answer_segments",
                    match (self).0.answer_segments.as_ref() {
                        Some(item) => mp::json_summary(item),
                        None => serde_json::Value::Null,
                    },
                ),
                (
                    "required_action",
                    match (self).0.required_action.as_ref() {
                        Some(item) => mp::json_summary(item),
                        None => serde_json::Value::Null,
                    },
                ),
                (
                    "tool_calls",
                    match (self).0.tool_calls.as_ref() {
                        Some(item) => mp::json_summary(item),
                        None => serde_json::Value::Null,
                    },
                ),
                (
                    "usage",
                    match (self).0.usage.as_ref() {
                        Some(item) => mp::json_summary(item),
                        None => serde_json::Value::Null,
                    },
                ),
                (
                    "error",
                    match (self).0.error.as_ref() {
                        Some(item) => mp::json_summary(item),
                        None => serde_json::Value::Null,
                    },
                ),
                (
                    "operation_terminal",
                    match (self).0.operation_terminal.as_ref() {
                        Some(item) => mp::json_summary(item),
                        None => serde_json::Value::Null,
                    },
                ),
                ("created_at", mp::text(&(self).0.created_at)?),
            ]),
        )]))
    }

    const CONTRACT_ID: &'static str = "application-native-run-get-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct NativeCancelRunInput(pub(crate) Uuid);

impl InterfaceContract for NativeCancelRunInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[("0", mp::text_schema())]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "0",
            serde_json::Value::String((self).0.to_string()),
        )]))
    }

    const CONTRACT_ID: &'static str = "application-native-run-cancel-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct NativeCancelRunOutput(pub(crate) NativeRunResponse);

impl InterfaceContract for NativeCancelRunOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "0",
            mp::object_schema(&[
                ("id", mp::text_schema()),
                ("application_id", mp::text_schema()),
                ("publication_version_id", mp::text_schema()),
                ("status", mp::text_schema()),
                ("node_input_payload", mp::json_summary_schema()),
                ("metadata", mp::json_summary_schema()),
                (
                    "answer",
                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                ),
                (
                    "answer_segments",
                    serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                ),
                (
                    "required_action",
                    serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                ),
                (
                    "tool_calls",
                    serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                ),
                (
                    "usage",
                    serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                ),
                (
                    "error",
                    serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                ),
                (
                    "operation_terminal",
                    serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                ),
                ("created_at", mp::text_schema()),
            ]),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "0",
            mp::object_value(&[
                ("id", serde_json::Value::String((self).0.id.to_string())),
                (
                    "application_id",
                    serde_json::Value::String((self).0.application_id.to_string()),
                ),
                (
                    "publication_version_id",
                    serde_json::Value::String((self).0.publication_version_id.to_string()),
                ),
                ("status", mp::text(&(self).0.status)?),
                (
                    "node_input_payload",
                    mp::json_summary(&(self).0.node_input_payload),
                ),
                ("metadata", mp::json_summary(&(self).0.metadata)),
                (
                    "answer",
                    match (self).0.answer.as_ref() {
                        Some(item) => {
                            mp::object_value(&[("byte_count", serde_json::json!((item).len()))])
                        }
                        None => serde_json::Value::Null,
                    },
                ),
                (
                    "answer_segments",
                    match (self).0.answer_segments.as_ref() {
                        Some(item) => mp::json_summary(item),
                        None => serde_json::Value::Null,
                    },
                ),
                (
                    "required_action",
                    match (self).0.required_action.as_ref() {
                        Some(item) => mp::json_summary(item),
                        None => serde_json::Value::Null,
                    },
                ),
                (
                    "tool_calls",
                    match (self).0.tool_calls.as_ref() {
                        Some(item) => mp::json_summary(item),
                        None => serde_json::Value::Null,
                    },
                ),
                (
                    "usage",
                    match (self).0.usage.as_ref() {
                        Some(item) => mp::json_summary(item),
                        None => serde_json::Value::Null,
                    },
                ),
                (
                    "error",
                    match (self).0.error.as_ref() {
                        Some(item) => mp::json_summary(item),
                        None => serde_json::Value::Null,
                    },
                ),
                (
                    "operation_terminal",
                    match (self).0.operation_terminal.as_ref() {
                        Some(item) => mp::json_summary(item),
                        None => serde_json::Value::Null,
                    },
                ),
                ("created_at", mp::text(&(self).0.created_at)?),
            ]),
        )]))
    }

    const CONTRACT_ID: &'static str = "application-native-run-cancel-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct NativeResumeRunInput {
    pub(crate) run_id: Uuid,
    pub(crate) callback_task_id: Uuid,
    pub(crate) response_payload: Value,
    pub(crate) response_mode: Option<String>,
}

impl InterfaceContract for NativeResumeRunInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[
            ("run_id", mp::text_schema()),
            ("callback_task_id", mp::text_schema()),
            ("response_payload", mp::json_summary_schema()),
            (
                "response_mode",
                serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
            ),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[
            (
                "run_id",
                serde_json::Value::String((self).run_id.to_string()),
            ),
            (
                "callback_task_id",
                serde_json::Value::String((self).callback_task_id.to_string()),
            ),
            (
                "response_payload",
                mp::json_summary(&(self).response_payload),
            ),
            (
                "response_mode",
                match (self).response_mode.as_ref() {
                    Some(item) => mp::text(item)?,
                    None => serde_json::Value::Null,
                },
            ),
        ]))
    }

    const CONTRACT_ID: &'static str = "application-native-run-resume-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct NativeResumeRunOutput(pub(crate) NativeRunResponse);

impl InterfaceContract for NativeResumeRunOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "0",
            mp::object_schema(&[
                ("id", mp::text_schema()),
                ("application_id", mp::text_schema()),
                ("publication_version_id", mp::text_schema()),
                ("status", mp::text_schema()),
                ("node_input_payload", mp::json_summary_schema()),
                ("metadata", mp::json_summary_schema()),
                (
                    "answer",
                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                ),
                (
                    "answer_segments",
                    serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                ),
                (
                    "required_action",
                    serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                ),
                (
                    "tool_calls",
                    serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                ),
                (
                    "usage",
                    serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                ),
                (
                    "error",
                    serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                ),
                (
                    "operation_terminal",
                    serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                ),
                ("created_at", mp::text_schema()),
            ]),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "0",
            mp::object_value(&[
                ("id", serde_json::Value::String((self).0.id.to_string())),
                (
                    "application_id",
                    serde_json::Value::String((self).0.application_id.to_string()),
                ),
                (
                    "publication_version_id",
                    serde_json::Value::String((self).0.publication_version_id.to_string()),
                ),
                ("status", mp::text(&(self).0.status)?),
                (
                    "node_input_payload",
                    mp::json_summary(&(self).0.node_input_payload),
                ),
                ("metadata", mp::json_summary(&(self).0.metadata)),
                (
                    "answer",
                    match (self).0.answer.as_ref() {
                        Some(item) => {
                            mp::object_value(&[("byte_count", serde_json::json!((item).len()))])
                        }
                        None => serde_json::Value::Null,
                    },
                ),
                (
                    "answer_segments",
                    match (self).0.answer_segments.as_ref() {
                        Some(item) => mp::json_summary(item),
                        None => serde_json::Value::Null,
                    },
                ),
                (
                    "required_action",
                    match (self).0.required_action.as_ref() {
                        Some(item) => mp::json_summary(item),
                        None => serde_json::Value::Null,
                    },
                ),
                (
                    "tool_calls",
                    match (self).0.tool_calls.as_ref() {
                        Some(item) => mp::json_summary(item),
                        None => serde_json::Value::Null,
                    },
                ),
                (
                    "usage",
                    match (self).0.usage.as_ref() {
                        Some(item) => mp::json_summary(item),
                        None => serde_json::Value::Null,
                    },
                ),
                (
                    "error",
                    match (self).0.error.as_ref() {
                        Some(item) => mp::json_summary(item),
                        None => serde_json::Value::Null,
                    },
                ),
                (
                    "operation_terminal",
                    match (self).0.operation_terminal.as_ref() {
                        Some(item) => mp::json_summary(item),
                        None => serde_json::Value::Null,
                    },
                ),
                ("created_at", mp::text(&(self).0.created_at)?),
            ]),
        )]))
    }

    const CONTRACT_ID: &'static str = "application-native-run-resume-output";
    const CONTRACT_VERSION: &'static str = "1";
}
