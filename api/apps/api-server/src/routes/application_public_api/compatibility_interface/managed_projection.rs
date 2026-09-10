use super::*;

impl InterfaceContract for CompatibilityBlockingInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "command",
            mp::union_schema(vec![
                mp::object_schema(&[
                    ("variant", mp::tag_schema("Start")),
                    (
                        "request",
                        mp::object_schema(&[
                            (
                                "query",
                                mp::object_schema(&[("byte_count", mp::count_schema())]),
                            ),
                            (
                                "system",
                                mp::object_schema(&[("item_count", mp::count_schema())]),
                            ),
                            (
                                "model",
                                serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                            ),
                            (
                                "history",
                                mp::object_schema(&[("item_count", mp::count_schema())]),
                            ),
                            (
                                "attachments",
                                mp::object_schema(&[("item_count", mp::count_schema())]),
                            ),
                            (
                                "expand_id",
                                serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                            ),
                            (
                                "response_mode",
                                serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                            ),
                            (
                                "stream_options",
                                mp::object_schema(&[(
                                    "include_workflow_events",
                                    mp::union_schema(vec![
                                        mp::object_schema(&[("variant", mp::tag_schema("None"))]),
                                        mp::object_schema(&[("variant", mp::tag_schema("Public"))]),
                                        mp::object_schema(&[("variant", mp::tag_schema("Debug"))]),
                                    ]),
                                )]),
                            ),
                            (
                                "request_context",
                                mp::object_schema(&[(
                                    "end_user_reference",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                )]),
                            ),
                            (
                                "title",
                                serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                            ),
                            (
                                "client_protocol_envelope",
                                serde_json::json!({"anyOf": [mp::object_schema(&[("source_protocol",mp::object_schema(&[("byte_count",mp::count_schema())])), ("query",mp::object_schema(&[("item_count",mp::count_schema())])), ("body",mp::object_schema(&[("item_count",mp::count_schema())]))]), {"type":"null"}]}),
                            ),
                        ]),
                    ),
                    (
                        "protocol",
                        mp::union_schema(vec![
                            mp::object_schema(&[("variant", mp::tag_schema("Native"))]),
                            mp::object_schema(&[("variant", mp::tag_schema("OpenAiChat"))]),
                            mp::object_schema(&[("variant", mp::tag_schema("OpenAiResponses"))]),
                            mp::object_schema(&[("variant", mp::tag_schema("AnthropicMessages"))]),
                        ]),
                    ),
                    (
                        "provider_transport",
                        serde_json::json!({"anyOf": [mp::object_schema(&[("operation",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Generate")), ("0",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Standard"))]), mp::object_schema(&[("variant",mp::tag_schema("LocalSummary"))])]))]), mp::object_schema(&[("variant",mp::tag_schema("CountTokens"))]), mp::object_schema(&[("variant",mp::tag_schema("Compact")), ("0",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("ResponsesCompact"))]), mp::object_schema(&[("variant",mp::tag_schema("ResponsesCompactionV2"))])]))])]))]), {"type":"null"}]}),
                    ),
                ]),
                mp::object_schema(&[
                    ("variant", mp::tag_schema("Resume")),
                    (
                        "initial_run",
                        mp::object_schema(&[
                            ("id", mp::text_schema()),
                            ("application_id", mp::text_schema()),
                            ("publication_version_id", mp::text_schema()),
                            (
                                "status",
                                mp::union_schema(vec![
                                    mp::object_schema(&[("variant", mp::tag_schema("Created"))]),
                                    mp::object_schema(&[("variant", mp::tag_schema("Queued"))]),
                                    mp::object_schema(&[("variant", mp::tag_schema("Running"))]),
                                    mp::object_schema(&[("variant", mp::tag_schema("Waiting"))]),
                                    mp::object_schema(&[("variant", mp::tag_schema("Succeeded"))]),
                                    mp::object_schema(&[("variant", mp::tag_schema("Incomplete"))]),
                                    mp::object_schema(&[("variant", mp::tag_schema("Failed"))]),
                                    mp::object_schema(&[("variant", mp::tag_schema("Cancelled"))]),
                                ]),
                            ),
                            ("node_input_payload", mp::json_summary_schema()),
                            ("metadata", mp::json_summary_schema()),
                            (
                                "answer",
                                serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                            ),
                            (
                                "answer_segments",
                                serde_json::json!({"anyOf": [mp::object_schema(&[("item_count",mp::count_schema())]), {"type":"null"}]}),
                            ),
                            (
                                "required_action",
                                serde_json::json!({"anyOf": [mp::object_schema(&[("action_type",mp::text_schema()), ("payload",mp::json_summary_schema())]), {"type":"null"}]}),
                            ),
                            (
                                "tool_calls",
                                serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                            ),
                            (
                                "error",
                                serde_json::json!({"anyOf": [mp::object_schema(&[("code",mp::text_schema()), ("message",mp::object_schema(&[("byte_count",mp::count_schema())])), ("details",mp::json_summary_schema())]), {"type":"null"}]}),
                            ),
                            (
                                "operation_terminal",
                                serde_json::json!({"anyOf": [mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("CountTokens"))]), mp::object_schema(&[("variant",mp::tag_schema("Compact"))])]), {"type":"null"}]}),
                            ),
                            ("created_at", serde_json::json!({"type":"integer"})),
                        ]),
                    ),
                    (
                        "command",
                        mp::object_schema(&[
                            (
                                "target",
                                mp::union_schema(vec![
                                    mp::object_schema(&[
                                        ("variant", mp::tag_schema("FlowRun")),
                                        ("flow_run_id", mp::text_schema()),
                                        ("callback_task_id", mp::text_schema()),
                                    ]),
                                    mp::object_schema(&[
                                        ("variant", mp::tag_schema("CallbackTask")),
                                        ("callback_task_id", mp::text_schema()),
                                    ]),
                                ]),
                            ),
                            (
                                "source",
                                mp::union_schema(vec![
                                    mp::object_schema(&[(
                                        "variant",
                                        mp::tag_schema("NativeAgent"),
                                    )]),
                                    mp::object_schema(&[("variant", mp::tag_schema("OpenAiChat"))]),
                                    mp::object_schema(&[(
                                        "variant",
                                        mp::tag_schema("OpenAiResponses"),
                                    )]),
                                    mp::object_schema(&[(
                                        "variant",
                                        mp::tag_schema("AnthropicMessages"),
                                    )]),
                                ]),
                            ),
                            ("response_payload", mp::json_summary_schema()),
                            (
                                "response_mode",
                                serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                            ),
                        ]),
                    ),
                ]),
            ]),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[("command",match &(self).command {CompatibilityInvocationCommand::Start {request: _field_request, protocol: _field_protocol, provider_transport: _field_provider_transport, .. } => mp::object_value(&[("variant",serde_json::Value::String("Start".to_owned())), ("request",mp::object_value(&[("query",mp::object_value(&[("byte_count",serde_json::json!((&(_field_request).query).len()))])), ("system",mp::object_value(&[("item_count",serde_json::json!((&(_field_request).system).len()))])), ("model",match (&(_field_request).model).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("history",mp::object_value(&[("item_count",serde_json::json!((&(_field_request).history).len()))])), ("attachments",mp::object_value(&[("item_count",serde_json::json!((&(_field_request).attachments).len()))])), ("expand_id",match (&(_field_request).expand_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("response_mode",match (&(_field_request).response_mode).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("stream_options",mp::object_value(&[("include_workflow_events",match &(&(_field_request).stream_options).include_workflow_events {control_plane::application_public_api::native::NativeWorkflowEventVisibility::None => mp::object_value(&[("variant",serde_json::Value::String("None".to_owned()))]), control_plane::application_public_api::native::NativeWorkflowEventVisibility::Public => mp::object_value(&[("variant",serde_json::Value::String("Public".to_owned()))]), control_plane::application_public_api::native::NativeWorkflowEventVisibility::Debug => mp::object_value(&[("variant",serde_json::Value::String("Debug".to_owned()))])})])), ("request_context",mp::object_value(&[("end_user_reference",match (&(&(_field_request).request_context).end_user_reference).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("title",match (&(_field_request).title).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("client_protocol_envelope",match (&(_field_request).client_protocol_envelope).as_ref() { Some(item) => mp::object_value(&[("source_protocol",mp::object_value(&[("byte_count",serde_json::json!((&(item).source_protocol).len()))])), ("query",mp::object_value(&[("item_count",serde_json::json!((&(item).query).len()))])), ("body",mp::object_value(&[("item_count",serde_json::json!((&(item).body).len()))]))]), None => serde_json::Value::Null })])), ("protocol",match _field_protocol {control_plane::application_public_api::protocol_translation::TranslationProtocol::Native => mp::object_value(&[("variant",serde_json::Value::String("Native".to_owned()))]), control_plane::application_public_api::protocol_translation::TranslationProtocol::OpenAiChat => mp::object_value(&[("variant",serde_json::Value::String("OpenAiChat".to_owned()))]), control_plane::application_public_api::protocol_translation::TranslationProtocol::OpenAiResponses => mp::object_value(&[("variant",serde_json::Value::String("OpenAiResponses".to_owned()))]), control_plane::application_public_api::protocol_translation::TranslationProtocol::AnthropicMessages => mp::object_value(&[("variant",serde_json::Value::String("AnthropicMessages".to_owned()))])}), ("provider_transport",match (_field_provider_transport).as_ref() { Some(item) => mp::object_value(&[("operation",match &(item).operation {domain::ai_native_operation::AiNativeOperation::Generate(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Generate".to_owned())), ("0",match _field_0 {domain::ai_native_operation::AiNativeGenerateProfile::Standard => mp::object_value(&[("variant",serde_json::Value::String("Standard".to_owned()))]), domain::ai_native_operation::AiNativeGenerateProfile::LocalSummary => mp::object_value(&[("variant",serde_json::Value::String("LocalSummary".to_owned()))])})]), domain::ai_native_operation::AiNativeOperation::CountTokens => mp::object_value(&[("variant",serde_json::Value::String("CountTokens".to_owned()))]), domain::ai_native_operation::AiNativeOperation::Compact(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Compact".to_owned())), ("0",match _field_0 {domain::ai_native_operation::AiNativeCompactProfile::ResponsesCompact => mp::object_value(&[("variant",serde_json::Value::String("ResponsesCompact".to_owned()))]), domain::ai_native_operation::AiNativeCompactProfile::ResponsesCompactionV2 => mp::object_value(&[("variant",serde_json::Value::String("ResponsesCompactionV2".to_owned()))])})])})]), None => serde_json::Value::Null })]), CompatibilityInvocationCommand::Resume {initial_run: _field_initial_run, command: _field_command, .. } => mp::object_value(&[("variant",serde_json::Value::String("Resume".to_owned())), ("initial_run",mp::object_value(&[("id",serde_json::Value::String((&(_field_initial_run).id).to_string())), ("application_id",serde_json::Value::String((&(_field_initial_run).application_id).to_string())), ("publication_version_id",serde_json::Value::String((&(_field_initial_run).publication_version_id).to_string())), ("status",match &(_field_initial_run).status {control_plane::application_public_api::native::NativeRunStatus::Created => mp::object_value(&[("variant",serde_json::Value::String("Created".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Queued => mp::object_value(&[("variant",serde_json::Value::String("Queued".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Running => mp::object_value(&[("variant",serde_json::Value::String("Running".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Waiting => mp::object_value(&[("variant",serde_json::Value::String("Waiting".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Succeeded => mp::object_value(&[("variant",serde_json::Value::String("Succeeded".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Incomplete => mp::object_value(&[("variant",serde_json::Value::String("Incomplete".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Failed => mp::object_value(&[("variant",serde_json::Value::String("Failed".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Cancelled => mp::object_value(&[("variant",serde_json::Value::String("Cancelled".to_owned()))])}), ("node_input_payload",mp::json_summary(&(_field_initial_run).node_input_payload)), ("metadata",mp::json_summary(&(_field_initial_run).metadata)), ("answer",match (&(_field_initial_run).answer).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("answer_segments",match (&(_field_initial_run).answer_segments).as_ref() { Some(item) => mp::object_value(&[("item_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("required_action",match (&(_field_initial_run).required_action).as_ref() { Some(item) => mp::object_value(&[("action_type",mp::text(&(item).action_type)?), ("payload",mp::json_summary(&(item).payload))]), None => serde_json::Value::Null }), ("tool_calls",match (&(_field_initial_run).tool_calls).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("error",match (&(_field_initial_run).error).as_ref() { Some(item) => mp::object_value(&[("code",mp::text(&(item).code)?), ("message",mp::object_value(&[("byte_count",serde_json::json!((&(item).message).len()))])), ("details",mp::json_summary(&(item).details))]), None => serde_json::Value::Null }), ("operation_terminal",match (&(_field_initial_run).operation_terminal).as_ref() { Some(item) => match item {extension_contracts::semantic_terminal::NativeOperationTerminal::CountTokens(_) => mp::object_value(&[("variant",serde_json::Value::String("CountTokens".to_owned()))]), extension_contracts::semantic_terminal::NativeOperationTerminal::Compact(_) => mp::object_value(&[("variant",serde_json::Value::String("Compact".to_owned()))])}, None => serde_json::Value::Null }), ("created_at",serde_json::json!((&(_field_initial_run).created_at).unix_timestamp()))])), ("command",mp::object_value(&[("target",match &(_field_command).target {control_plane::application_public_api::callback_resume::PublishedCallbackResumeTarget::FlowRun {flow_run_id: _field_flow_run_id, callback_task_id: _field_callback_task_id, .. } => mp::object_value(&[("variant",serde_json::Value::String("FlowRun".to_owned())), ("flow_run_id",serde_json::Value::String((_field_flow_run_id).to_string())), ("callback_task_id",serde_json::Value::String((_field_callback_task_id).to_string()))]), control_plane::application_public_api::callback_resume::PublishedCallbackResumeTarget::CallbackTask {callback_task_id: _field_callback_task_id, .. } => mp::object_value(&[("variant",serde_json::Value::String("CallbackTask".to_owned())), ("callback_task_id",serde_json::Value::String((_field_callback_task_id).to_string()))])}), ("source",match &(_field_command).source {control_plane::application_public_api::callback_resume::PublishedCallbackResumeSource::NativeAgent => mp::object_value(&[("variant",serde_json::Value::String("NativeAgent".to_owned()))]), control_plane::application_public_api::callback_resume::PublishedCallbackResumeSource::OpenAiChat => mp::object_value(&[("variant",serde_json::Value::String("OpenAiChat".to_owned()))]), control_plane::application_public_api::callback_resume::PublishedCallbackResumeSource::OpenAiResponses => mp::object_value(&[("variant",serde_json::Value::String("OpenAiResponses".to_owned()))]), control_plane::application_public_api::callback_resume::PublishedCallbackResumeSource::AnthropicMessages => mp::object_value(&[("variant",serde_json::Value::String("AnthropicMessages".to_owned()))])}), ("response_payload",mp::json_summary(&(_field_command).response_payload)), ("response_mode",match (&(_field_command).response_mode).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null })]))])})]))
    }

    const CONTRACT_ID: &'static str = "application-compatibility-blocking-input";
    const CONTRACT_VERSION: &'static str = "1";
}

impl InterfaceContract for CompatibilityBlockingOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "0",
            mp::object_schema(&[
                ("id", mp::text_schema()),
                ("application_id", mp::text_schema()),
                ("publication_version_id", mp::text_schema()),
                (
                    "status",
                    mp::union_schema(vec![
                        mp::object_schema(&[("variant", mp::tag_schema("Created"))]),
                        mp::object_schema(&[("variant", mp::tag_schema("Queued"))]),
                        mp::object_schema(&[("variant", mp::tag_schema("Running"))]),
                        mp::object_schema(&[("variant", mp::tag_schema("Waiting"))]),
                        mp::object_schema(&[("variant", mp::tag_schema("Succeeded"))]),
                        mp::object_schema(&[("variant", mp::tag_schema("Incomplete"))]),
                        mp::object_schema(&[("variant", mp::tag_schema("Failed"))]),
                        mp::object_schema(&[("variant", mp::tag_schema("Cancelled"))]),
                    ]),
                ),
                ("node_input_payload", mp::json_summary_schema()),
                ("metadata", mp::json_summary_schema()),
                (
                    "answer",
                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                ),
                (
                    "answer_segments",
                    serde_json::json!({"anyOf": [serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("kind",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Reasoning"))]), mp::object_schema(&[("variant",mp::tag_schema("Message"))])])), ("text",mp::object_schema(&[("byte_count",mp::count_schema())]))])}), {"type":"null"}]}),
                ),
                (
                    "required_action",
                    serde_json::json!({"anyOf": [mp::object_schema(&[("action_type",mp::text_schema()), ("payload",mp::json_summary_schema())]), {"type":"null"}]}),
                ),
                (
                    "tool_calls",
                    serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                ),
                (
                    "error",
                    serde_json::json!({"anyOf": [mp::object_schema(&[("code",mp::text_schema()), ("message",mp::object_schema(&[("byte_count",mp::count_schema())])), ("details",mp::json_summary_schema())]), {"type":"null"}]}),
                ),
                (
                    "operation_terminal",
                    serde_json::json!({"anyOf": [mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("CountTokens"))]), mp::object_schema(&[("variant",mp::tag_schema("Compact"))])]), {"type":"null"}]}),
                ),
                ("created_at", serde_json::json!({"type":"integer"})),
            ]),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[("0",mp::object_value(&[("id",serde_json::Value::String((&(&(self).0).id).to_string())), ("application_id",serde_json::Value::String((&(&(self).0).application_id).to_string())), ("publication_version_id",serde_json::Value::String((&(&(self).0).publication_version_id).to_string())), ("status",match &(&(self).0).status {control_plane::application_public_api::native::NativeRunStatus::Created => mp::object_value(&[("variant",serde_json::Value::String("Created".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Queued => mp::object_value(&[("variant",serde_json::Value::String("Queued".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Running => mp::object_value(&[("variant",serde_json::Value::String("Running".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Waiting => mp::object_value(&[("variant",serde_json::Value::String("Waiting".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Succeeded => mp::object_value(&[("variant",serde_json::Value::String("Succeeded".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Incomplete => mp::object_value(&[("variant",serde_json::Value::String("Incomplete".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Failed => mp::object_value(&[("variant",serde_json::Value::String("Failed".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Cancelled => mp::object_value(&[("variant",serde_json::Value::String("Cancelled".to_owned()))])}), ("node_input_payload",mp::json_summary(&(&(self).0).node_input_payload)), ("metadata",mp::json_summary(&(&(self).0).metadata)), ("answer",match (&(&(self).0).answer).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("answer_segments",match (&(&(self).0).answer_segments).as_ref() { Some(item) => { if (item).len() > 32 { return None; } serde_json::Value::Array((item).iter().map(|item| Some(mp::object_value(&[("kind",match &(item).kind {orchestration_runtime::answer_projection::AnswerProjectionSegmentKind::Reasoning => mp::object_value(&[("variant",serde_json::Value::String("Reasoning".to_owned()))]), orchestration_runtime::answer_projection::AnswerProjectionSegmentKind::Message => mp::object_value(&[("variant",serde_json::Value::String("Message".to_owned()))])}), ("text",mp::object_value(&[("byte_count",serde_json::json!((&(item).text).len()))]))]))).collect::<Option<Vec<_>>>()?) }, None => serde_json::Value::Null }), ("required_action",match (&(&(self).0).required_action).as_ref() { Some(item) => mp::object_value(&[("action_type",mp::text(&(item).action_type)?), ("payload",mp::json_summary(&(item).payload))]), None => serde_json::Value::Null }), ("tool_calls",match (&(&(self).0).tool_calls).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("error",match (&(&(self).0).error).as_ref() { Some(item) => mp::object_value(&[("code",mp::text(&(item).code)?), ("message",mp::object_value(&[("byte_count",serde_json::json!((&(item).message).len()))])), ("details",mp::json_summary(&(item).details))]), None => serde_json::Value::Null }), ("operation_terminal",match (&(&(self).0).operation_terminal).as_ref() { Some(item) => match item {extension_contracts::semantic_terminal::NativeOperationTerminal::CountTokens(_) => mp::object_value(&[("variant",serde_json::Value::String("CountTokens".to_owned()))]), extension_contracts::semantic_terminal::NativeOperationTerminal::Compact(_) => mp::object_value(&[("variant",serde_json::Value::String("Compact".to_owned()))])}, None => serde_json::Value::Null }), ("created_at",serde_json::json!((&(&(self).0).created_at).unix_timestamp()))]))]))
    }

    const CONTRACT_ID: &'static str = "application-compatibility-blocking-output";
    const CONTRACT_VERSION: &'static str = "1";
}

impl InterfaceContract for CompatibilityBlockingTargetError {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "0",
            mp::object_schema(&[
                (
                    "status",
                    serde_json::json!({"type":"integer","minimum":100,"maximum":599}),
                ),
                ("code", mp::text_schema()),
                (
                    "message",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "0",
            mp::object_value(&[
                ("status", serde_json::json!((&(&(self).0).status).as_u16())),
                ("code", mp::text(&(&(self).0).code)?),
                (
                    "message",
                    mp::object_value(&[(
                        "byte_count",
                        serde_json::json!((&(&(self).0).message).len()),
                    )]),
                ),
            ]),
        )]))
    }

    const CONTRACT_ID: &'static str = "application-compatibility-blocking-error";
    const CONTRACT_VERSION: &'static str = "1";
}

impl InterfaceContract for CompatibilityStreamEvent {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[
            (
                "run",
                mp::object_schema(&[
                    ("id", mp::text_schema()),
                    ("application_id", mp::text_schema()),
                    ("publication_version_id", mp::text_schema()),
                    (
                        "status",
                        mp::union_schema(vec![
                            mp::object_schema(&[("variant", mp::tag_schema("Created"))]),
                            mp::object_schema(&[("variant", mp::tag_schema("Queued"))]),
                            mp::object_schema(&[("variant", mp::tag_schema("Running"))]),
                            mp::object_schema(&[("variant", mp::tag_schema("Waiting"))]),
                            mp::object_schema(&[("variant", mp::tag_schema("Succeeded"))]),
                            mp::object_schema(&[("variant", mp::tag_schema("Incomplete"))]),
                            mp::object_schema(&[("variant", mp::tag_schema("Failed"))]),
                            mp::object_schema(&[("variant", mp::tag_schema("Cancelled"))]),
                        ]),
                    ),
                    ("node_input_payload", mp::json_summary_schema()),
                    ("metadata", mp::json_summary_schema()),
                    (
                        "answer",
                        serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                    ),
                    (
                        "answer_segments",
                        serde_json::json!({"anyOf": [serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("kind",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Reasoning"))]), mp::object_schema(&[("variant",mp::tag_schema("Message"))])])), ("text",mp::object_schema(&[("byte_count",mp::count_schema())]))])}), {"type":"null"}]}),
                    ),
                    (
                        "required_action",
                        serde_json::json!({"anyOf": [mp::object_schema(&[("action_type",mp::text_schema()), ("payload",mp::json_summary_schema())]), {"type":"null"}]}),
                    ),
                    (
                        "tool_calls",
                        serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                    ),
                    (
                        "error",
                        serde_json::json!({"anyOf": [mp::object_schema(&[("code",mp::text_schema()), ("message",mp::object_schema(&[("byte_count",mp::count_schema())])), ("details",mp::json_summary_schema())]), {"type":"null"}]}),
                    ),
                    (
                        "operation_terminal",
                        serde_json::json!({"anyOf": [mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("CountTokens"))]), mp::object_schema(&[("variant",mp::tag_schema("Compact"))])]), {"type":"null"}]}),
                    ),
                    ("created_at", serde_json::json!({"type":"integer"})),
                ]),
            ),
            (
                "envelope",
                mp::object_schema(&[
                    ("run_id", mp::text_schema()),
                    (
                        "node_run_id",
                        serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                    ),
                    ("sequence", serde_json::json!({"type":"integer"})),
                    ("event_id", mp::text_schema()),
                    ("event_type", mp::text_schema()),
                    ("occurred_at", serde_json::json!({"type":"integer"})),
                    (
                        "delta_index",
                        serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                    ),
                    (
                        "content_type",
                        serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                    ),
                    (
                        "text",
                        serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                    ),
                    (
                        "source",
                        mp::union_schema(vec![
                            mp::object_schema(&[("variant", mp::tag_schema("Runtime"))]),
                            mp::object_schema(&[("variant", mp::tag_schema("Provider"))]),
                            mp::object_schema(&[("variant", mp::tag_schema("Persister"))]),
                            mp::object_schema(&[("variant", mp::tag_schema("System"))]),
                        ]),
                    ),
                    (
                        "durability",
                        mp::union_schema(vec![
                            mp::object_schema(&[("variant", mp::tag_schema("Ephemeral"))]),
                            mp::object_schema(&[("variant", mp::tag_schema("DurableRequired"))]),
                            mp::object_schema(&[("variant", mp::tag_schema("AuditRequired"))]),
                        ]),
                    ),
                    ("persist_required", serde_json::json!({"type":"boolean"})),
                    ("trace_visible", serde_json::json!({"type":"boolean"})),
                    ("payload", mp::json_summary_schema()),
                ]),
            ),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[("run",mp::object_value(&[("id",serde_json::Value::String((&(&(self).run).id).to_string())), ("application_id",serde_json::Value::String((&(&(self).run).application_id).to_string())), ("publication_version_id",serde_json::Value::String((&(&(self).run).publication_version_id).to_string())), ("status",match &(&(self).run).status {control_plane::application_public_api::native::NativeRunStatus::Created => mp::object_value(&[("variant",serde_json::Value::String("Created".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Queued => mp::object_value(&[("variant",serde_json::Value::String("Queued".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Running => mp::object_value(&[("variant",serde_json::Value::String("Running".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Waiting => mp::object_value(&[("variant",serde_json::Value::String("Waiting".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Succeeded => mp::object_value(&[("variant",serde_json::Value::String("Succeeded".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Incomplete => mp::object_value(&[("variant",serde_json::Value::String("Incomplete".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Failed => mp::object_value(&[("variant",serde_json::Value::String("Failed".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Cancelled => mp::object_value(&[("variant",serde_json::Value::String("Cancelled".to_owned()))])}), ("node_input_payload",mp::json_summary(&(&(self).run).node_input_payload)), ("metadata",mp::json_summary(&(&(self).run).metadata)), ("answer",match (&(&(self).run).answer).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("answer_segments",match (&(&(self).run).answer_segments).as_ref() { Some(item) => { if (item).len() > 32 { return None; } serde_json::Value::Array((item).iter().map(|item| Some(mp::object_value(&[("kind",match &(item).kind {orchestration_runtime::answer_projection::AnswerProjectionSegmentKind::Reasoning => mp::object_value(&[("variant",serde_json::Value::String("Reasoning".to_owned()))]), orchestration_runtime::answer_projection::AnswerProjectionSegmentKind::Message => mp::object_value(&[("variant",serde_json::Value::String("Message".to_owned()))])}), ("text",mp::object_value(&[("byte_count",serde_json::json!((&(item).text).len()))]))]))).collect::<Option<Vec<_>>>()?) }, None => serde_json::Value::Null }), ("required_action",match (&(&(self).run).required_action).as_ref() { Some(item) => mp::object_value(&[("action_type",mp::text(&(item).action_type)?), ("payload",mp::json_summary(&(item).payload))]), None => serde_json::Value::Null }), ("tool_calls",match (&(&(self).run).tool_calls).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("error",match (&(&(self).run).error).as_ref() { Some(item) => mp::object_value(&[("code",mp::text(&(item).code)?), ("message",mp::object_value(&[("byte_count",serde_json::json!((&(item).message).len()))])), ("details",mp::json_summary(&(item).details))]), None => serde_json::Value::Null }), ("operation_terminal",match (&(&(self).run).operation_terminal).as_ref() { Some(item) => match item {extension_contracts::semantic_terminal::NativeOperationTerminal::CountTokens(_) => mp::object_value(&[("variant",serde_json::Value::String("CountTokens".to_owned()))]), extension_contracts::semantic_terminal::NativeOperationTerminal::Compact(_) => mp::object_value(&[("variant",serde_json::Value::String("Compact".to_owned()))])}, None => serde_json::Value::Null }), ("created_at",serde_json::json!((&(&(self).run).created_at).unix_timestamp()))])), ("envelope",mp::object_value(&[("run_id",serde_json::Value::String((&(&(self).envelope).run_id).to_string())), ("node_run_id",match (&(&(self).envelope).node_run_id).as_ref() { Some(item) => serde_json::Value::String((item).to_string()), None => serde_json::Value::Null }), ("sequence",serde_json::json!(*(&(&(self).envelope).sequence))), ("event_id",mp::text(&(&(self).envelope).event_id)?), ("event_type",mp::text(&(&(self).envelope).event_type)?), ("occurred_at",serde_json::json!((&(&(self).envelope).occurred_at).unix_timestamp())), ("delta_index",match (&(&(self).envelope).delta_index).as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null }), ("content_type",match (&(&(self).envelope).content_type).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("text",match (&(&(self).envelope).text).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("source",match &(&(self).envelope).source {control_plane::ports::RuntimeEventSource::Runtime => mp::object_value(&[("variant",serde_json::Value::String("Runtime".to_owned()))]), control_plane::ports::RuntimeEventSource::Provider => mp::object_value(&[("variant",serde_json::Value::String("Provider".to_owned()))]), control_plane::ports::RuntimeEventSource::Persister => mp::object_value(&[("variant",serde_json::Value::String("Persister".to_owned()))]), control_plane::ports::RuntimeEventSource::System => mp::object_value(&[("variant",serde_json::Value::String("System".to_owned()))])}), ("durability",match &(&(self).envelope).durability {control_plane::ports::RuntimeEventDurability::Ephemeral => mp::object_value(&[("variant",serde_json::Value::String("Ephemeral".to_owned()))]), control_plane::ports::RuntimeEventDurability::DurableRequired => mp::object_value(&[("variant",serde_json::Value::String("DurableRequired".to_owned()))]), control_plane::ports::RuntimeEventDurability::AuditRequired => mp::object_value(&[("variant",serde_json::Value::String("AuditRequired".to_owned()))])}), ("persist_required",serde_json::Value::Bool(*(&(&(self).envelope).persist_required))), ("trace_visible",serde_json::Value::Bool(*(&(&(self).envelope).trace_visible))), ("payload",mp::json_summary(&(&(self).envelope).payload))]))]))
    }

    const CONTRACT_ID: &'static str = "application-compatibility-stream-event";
    const CONTRACT_VERSION: &'static str = "1";
}
