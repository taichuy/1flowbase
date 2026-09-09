use super::*;

impl InterfaceContract for ApplicationRuntimeTracePayloadsInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("NodeContent")),
                ("application_id", mp::text_schema()),
                ("run_id", mp::text_schema()),
                ("trace_node_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("NodeDetail")),
                ("application_id", mp::text_schema()),
                ("run_id", mp::text_schema()),
                ("trace_node_id", mp::text_schema()),
                ("detail_ref_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ToolCallbackContent")),
                ("application_id", mp::text_schema()),
                ("run_id", mp::text_schema()),
                ("trace_node_id", mp::text_schema()),
                ("tool_call_id", mp::text_schema()),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::NodeContent {
                application_id: _field_application_id,
                run_id: _field_run_id,
                trace_node_id: _field_trace_node_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("NodeContent".to_owned()),
                ),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "run_id",
                    serde_json::Value::String((_field_run_id).to_string()),
                ),
                ("trace_node_id", mp::text(_field_trace_node_id)?),
            ]),
            Self::NodeDetail {
                application_id: _field_application_id,
                run_id: _field_run_id,
                trace_node_id: _field_trace_node_id,
                detail_ref_id: _field_detail_ref_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("NodeDetail".to_owned()),
                ),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "run_id",
                    serde_json::Value::String((_field_run_id).to_string()),
                ),
                ("trace_node_id", mp::text(_field_trace_node_id)?),
                ("detail_ref_id", mp::text(_field_detail_ref_id)?),
            ]),
            Self::ToolCallbackContent {
                application_id: _field_application_id,
                run_id: _field_run_id,
                trace_node_id: _field_trace_node_id,
                tool_call_id: _field_tool_call_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ToolCallbackContent".to_owned()),
                ),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "run_id",
                    serde_json::Value::String((_field_run_id).to_string()),
                ),
                ("trace_node_id", mp::text(_field_trace_node_id)?),
                ("tool_call_id", mp::text(_field_tool_call_id)?),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-application-runtime-trace-payloads-input";
    const CONTRACT_VERSION: &'static str = "1";
}

impl InterfaceContract for ApplicationRuntimeTracePayloadsOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("NodeContent")),
                (
                    "0",
                    mp::object_schema(&[
                        ("trace_node_id", mp::text_schema()),
                        (
                            "node_kind",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "projection_status",
                            mp::object_schema(&[
                                (
                                    "projection_status",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                ("projection_version", serde_json::json!({"type":"integer"})),
                                (
                                    "source_watermark",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                ("attempt_count", serde_json::json!({"type":"integer"})),
                                (
                                    "last_attempt_at",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "last_success_at",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "last_error_code",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                (
                                    "last_error_stage",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "last_error_source_kind",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "last_error_source_locator",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "last_error_ref",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                ("retriable", serde_json::json!({"type":"boolean"})),
                            ]),
                        ),
                        (
                            "content_kind",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("source_refs", mp::json_summary_schema()),
                        ("detail_refs", mp::json_summary_schema()),
                        ("payload", mp::json_summary_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("NodeDetail")),
                (
                    "0",
                    mp::object_schema(&[
                        ("trace_node_id", mp::text_schema()),
                        (
                            "node_kind",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "projection_status",
                            mp::object_schema(&[
                                (
                                    "projection_status",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                ("projection_version", serde_json::json!({"type":"integer"})),
                                (
                                    "source_watermark",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                ("attempt_count", serde_json::json!({"type":"integer"})),
                                (
                                    "last_attempt_at",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "last_success_at",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "last_error_code",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                (
                                    "last_error_stage",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "last_error_source_kind",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "last_error_source_locator",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "last_error_ref",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                ("retriable", serde_json::json!({"type":"boolean"})),
                            ]),
                        ),
                        ("detail_ref_id", mp::text_schema()),
                        (
                            "detail_kind",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("source_refs", mp::json_summary_schema()),
                        ("payload", mp::json_summary_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ToolCallbackContent")),
                (
                    "0",
                    mp::object_schema(&[
                        ("trace_node_id", mp::text_schema()),
                        ("tool_call_id", mp::text_schema()),
                        (
                            "projection_status",
                            mp::object_schema(&[
                                (
                                    "projection_status",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                ("projection_version", serde_json::json!({"type":"integer"})),
                                (
                                    "source_watermark",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                ("attempt_count", serde_json::json!({"type":"integer"})),
                                (
                                    "last_attempt_at",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "last_success_at",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "last_error_code",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                (
                                    "last_error_stage",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "last_error_source_kind",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "last_error_source_locator",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "last_error_ref",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                ("retriable", serde_json::json!({"type":"boolean"})),
                            ]),
                        ),
                        ("payload", mp::json_summary_schema()),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::NodeContent(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("NodeContent".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("trace_node_id", mp::text(&(_field_0).trace_node_id)?),
                        (
                            "node_kind",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).node_kind).len()),
                            )]),
                        ),
                        (
                            "projection_status",
                            mp::object_value(&[
                                (
                                    "projection_status",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_0).projection_status).projection_status)
                                                .len()
                                        ),
                                    )]),
                                ),
                                (
                                    "projection_version",
                                    serde_json::json!(
                                        *(&(&(_field_0).projection_status).projection_version)
                                    ),
                                ),
                                (
                                    "source_watermark",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_0).projection_status).source_watermark)
                                                .len()
                                        ),
                                    )]),
                                ),
                                (
                                    "attempt_count",
                                    serde_json::json!(
                                        *(&(&(_field_0).projection_status).attempt_count)
                                    ),
                                ),
                                (
                                    "last_attempt_at",
                                    match (&(&(_field_0).projection_status).last_attempt_at)
                                        .as_ref()
                                    {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "last_success_at",
                                    match (&(&(_field_0).projection_status).last_success_at)
                                        .as_ref()
                                    {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "last_error_code",
                                    match (&(&(_field_0).projection_status).last_error_code)
                                        .as_ref()
                                    {
                                        Some(item) => mp::text(item)?,
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "last_error_stage",
                                    match (&(&(_field_0).projection_status).last_error_stage)
                                        .as_ref()
                                    {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "last_error_source_kind",
                                    match (&(&(_field_0).projection_status).last_error_source_kind)
                                        .as_ref()
                                    {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "last_error_source_locator",
                                    match (&(&(_field_0).projection_status)
                                        .last_error_source_locator)
                                        .as_ref()
                                    {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "last_error_ref",
                                    match (&(&(_field_0).projection_status).last_error_ref).as_ref()
                                    {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "retriable",
                                    serde_json::Value::Bool(
                                        *(&(&(_field_0).projection_status).retriable),
                                    ),
                                ),
                            ]),
                        ),
                        (
                            "content_kind",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).content_kind).len()),
                            )]),
                        ),
                        ("source_refs", mp::json_summary(&(_field_0).source_refs)),
                        ("detail_refs", mp::json_summary(&(_field_0).detail_refs)),
                        ("payload", mp::json_summary(&(_field_0).payload)),
                    ]),
                ),
            ]),
            Self::NodeDetail(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("NodeDetail".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("trace_node_id", mp::text(&(_field_0).trace_node_id)?),
                        (
                            "node_kind",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).node_kind).len()),
                            )]),
                        ),
                        (
                            "projection_status",
                            mp::object_value(&[
                                (
                                    "projection_status",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_0).projection_status).projection_status)
                                                .len()
                                        ),
                                    )]),
                                ),
                                (
                                    "projection_version",
                                    serde_json::json!(
                                        *(&(&(_field_0).projection_status).projection_version)
                                    ),
                                ),
                                (
                                    "source_watermark",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_0).projection_status).source_watermark)
                                                .len()
                                        ),
                                    )]),
                                ),
                                (
                                    "attempt_count",
                                    serde_json::json!(
                                        *(&(&(_field_0).projection_status).attempt_count)
                                    ),
                                ),
                                (
                                    "last_attempt_at",
                                    match (&(&(_field_0).projection_status).last_attempt_at)
                                        .as_ref()
                                    {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "last_success_at",
                                    match (&(&(_field_0).projection_status).last_success_at)
                                        .as_ref()
                                    {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "last_error_code",
                                    match (&(&(_field_0).projection_status).last_error_code)
                                        .as_ref()
                                    {
                                        Some(item) => mp::text(item)?,
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "last_error_stage",
                                    match (&(&(_field_0).projection_status).last_error_stage)
                                        .as_ref()
                                    {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "last_error_source_kind",
                                    match (&(&(_field_0).projection_status).last_error_source_kind)
                                        .as_ref()
                                    {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "last_error_source_locator",
                                    match (&(&(_field_0).projection_status)
                                        .last_error_source_locator)
                                        .as_ref()
                                    {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "last_error_ref",
                                    match (&(&(_field_0).projection_status).last_error_ref).as_ref()
                                    {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "retriable",
                                    serde_json::Value::Bool(
                                        *(&(&(_field_0).projection_status).retriable),
                                    ),
                                ),
                            ]),
                        ),
                        ("detail_ref_id", mp::text(&(_field_0).detail_ref_id)?),
                        (
                            "detail_kind",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).detail_kind).len()),
                            )]),
                        ),
                        ("source_refs", mp::json_summary(&(_field_0).source_refs)),
                        ("payload", mp::json_summary(&(_field_0).payload)),
                    ]),
                ),
            ]),
            Self::ToolCallbackContent(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ToolCallbackContent".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("trace_node_id", mp::text(&(_field_0).trace_node_id)?),
                        ("tool_call_id", mp::text(&(_field_0).tool_call_id)?),
                        (
                            "projection_status",
                            mp::object_value(&[
                                (
                                    "projection_status",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_0).projection_status).projection_status)
                                                .len()
                                        ),
                                    )]),
                                ),
                                (
                                    "projection_version",
                                    serde_json::json!(
                                        *(&(&(_field_0).projection_status).projection_version)
                                    ),
                                ),
                                (
                                    "source_watermark",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_0).projection_status).source_watermark)
                                                .len()
                                        ),
                                    )]),
                                ),
                                (
                                    "attempt_count",
                                    serde_json::json!(
                                        *(&(&(_field_0).projection_status).attempt_count)
                                    ),
                                ),
                                (
                                    "last_attempt_at",
                                    match (&(&(_field_0).projection_status).last_attempt_at)
                                        .as_ref()
                                    {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "last_success_at",
                                    match (&(&(_field_0).projection_status).last_success_at)
                                        .as_ref()
                                    {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "last_error_code",
                                    match (&(&(_field_0).projection_status).last_error_code)
                                        .as_ref()
                                    {
                                        Some(item) => mp::text(item)?,
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "last_error_stage",
                                    match (&(&(_field_0).projection_status).last_error_stage)
                                        .as_ref()
                                    {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "last_error_source_kind",
                                    match (&(&(_field_0).projection_status).last_error_source_kind)
                                        .as_ref()
                                    {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "last_error_source_locator",
                                    match (&(&(_field_0).projection_status)
                                        .last_error_source_locator)
                                        .as_ref()
                                    {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "last_error_ref",
                                    match (&(&(_field_0).projection_status).last_error_ref).as_ref()
                                    {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "retriable",
                                    serde_json::Value::Bool(
                                        *(&(&(_field_0).projection_status).retriable),
                                    ),
                                ),
                            ]),
                        ),
                        ("payload", mp::json_summary(&(_field_0).payload)),
                    ]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-application-runtime-trace-payloads-output";
    const CONTRACT_VERSION: &'static str = "1";
}
