use super::*;

impl InterfaceContract for ApplicationRuntimeReadsInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("ListRuns")),
                ("application_id", mp::text_schema()),
                (
                    "query",
                    mp::object_schema(&[
                        (
                            "page",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "page_size",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "time_range_days",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "sort_by",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "sort_order",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "cache_mode",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ListConversationMessages")),
                ("application_id", mp::text_schema()),
                ("conversation_id", mp::text_schema()),
                (
                    "query",
                    mp::object_schema(&[
                        (
                            "around_run_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "before",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "after",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "limit",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ListRunConversationMessages")),
                ("application_id", mp::text_schema()),
                ("run_id", mp::text_schema()),
                (
                    "query",
                    mp::object_schema(&[
                        (
                            "around_run_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "before",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "after",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "limit",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetRunOverview")),
                ("application_id", mp::text_schema()),
                ("run_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetTraceTree")),
                ("application_id", mp::text_schema()),
                ("run_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetTraceChildren")),
                ("application_id", mp::text_schema()),
                ("run_id", mp::text_schema()),
                (
                    "query",
                    mp::object_schema(&[
                        ("parent_trace_node_id", mp::text_schema()),
                        (
                            "page_size",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "cursor",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetResumeTimeline")),
                ("application_id", mp::text_schema()),
                ("run_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetResumeTimelineSummary")),
                ("application_id", mp::text_schema()),
                ("run_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetRunNodeLastRun")),
                ("application_id", mp::text_schema()),
                ("run_id", mp::text_schema()),
                ("node_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetMonitoringReport")),
                ("application_id", mp::text_schema()),
                (
                    "query",
                    mp::object_schema(&[
                        (
                            "from",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "to",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "time_range_days",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "bucket",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetRuntimeActivity")),
                ("application_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetRuntimeDebugStream")),
                ("application_id", mp::text_schema()),
                ("run_id", mp::text_schema()),
                (
                    "query",
                    mp::object_schema(&[
                        (
                            "from_sequence",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "limit",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetNodeLastRun")),
                ("application_id", mp::text_schema()),
                ("node_id", mp::text_schema()),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::ListRuns {
                application_id: _field_application_id,
                query: _field_query,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("ListRuns".to_owned())),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "query",
                    mp::object_value(&[
                        (
                            "page",
                            match (&(_field_query).page).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "page_size",
                            match (&(_field_query).page_size).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "time_range_days",
                            match (&(_field_query).time_range_days).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "sort_by",
                            match (&(_field_query).sort_by).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "sort_order",
                            match (&(_field_query).sort_order).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "cache_mode",
                            match (&(_field_query).cache_mode).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::ListConversationMessages {
                application_id: _field_application_id,
                conversation_id: _field_conversation_id,
                query: _field_query,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ListConversationMessages".to_owned()),
                ),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                ("conversation_id", mp::text(_field_conversation_id)?),
                (
                    "query",
                    mp::object_value(&[
                        (
                            "around_run_id",
                            match (&(_field_query).around_run_id).as_ref() {
                                Some(item) => serde_json::Value::String((item).to_string()),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "before",
                            match (&(_field_query).before).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "after",
                            match (&(_field_query).after).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "limit",
                            match (&(_field_query).limit).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::ListRunConversationMessages {
                application_id: _field_application_id,
                run_id: _field_run_id,
                query: _field_query,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ListRunConversationMessages".to_owned()),
                ),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "run_id",
                    serde_json::Value::String((_field_run_id).to_string()),
                ),
                (
                    "query",
                    mp::object_value(&[
                        (
                            "around_run_id",
                            match (&(_field_query).around_run_id).as_ref() {
                                Some(item) => serde_json::Value::String((item).to_string()),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "before",
                            match (&(_field_query).before).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "after",
                            match (&(_field_query).after).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "limit",
                            match (&(_field_query).limit).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::GetRunOverview {
                application_id: _field_application_id,
                run_id: _field_run_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("GetRunOverview".to_owned()),
                ),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "run_id",
                    serde_json::Value::String((_field_run_id).to_string()),
                ),
            ]),
            Self::GetTraceTree {
                application_id: _field_application_id,
                run_id: _field_run_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("GetTraceTree".to_owned()),
                ),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "run_id",
                    serde_json::Value::String((_field_run_id).to_string()),
                ),
            ]),
            Self::GetTraceChildren {
                application_id: _field_application_id,
                run_id: _field_run_id,
                query: _field_query,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("GetTraceChildren".to_owned()),
                ),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "run_id",
                    serde_json::Value::String((_field_run_id).to_string()),
                ),
                (
                    "query",
                    mp::object_value(&[
                        (
                            "parent_trace_node_id",
                            mp::text(&(_field_query).parent_trace_node_id)?,
                        ),
                        (
                            "page_size",
                            match (&(_field_query).page_size).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "cursor",
                            match (&(_field_query).cursor).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::GetResumeTimeline {
                application_id: _field_application_id,
                run_id: _field_run_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("GetResumeTimeline".to_owned()),
                ),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "run_id",
                    serde_json::Value::String((_field_run_id).to_string()),
                ),
            ]),
            Self::GetResumeTimelineSummary {
                application_id: _field_application_id,
                run_id: _field_run_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("GetResumeTimelineSummary".to_owned()),
                ),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "run_id",
                    serde_json::Value::String((_field_run_id).to_string()),
                ),
            ]),
            Self::GetRunNodeLastRun {
                application_id: _field_application_id,
                run_id: _field_run_id,
                node_id: _field_node_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("GetRunNodeLastRun".to_owned()),
                ),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "run_id",
                    serde_json::Value::String((_field_run_id).to_string()),
                ),
                ("node_id", mp::text(_field_node_id)?),
            ]),
            Self::GetMonitoringReport {
                application_id: _field_application_id,
                query: _field_query,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("GetMonitoringReport".to_owned()),
                ),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "query",
                    mp::object_value(&[
                        (
                            "from",
                            match (&(_field_query).from).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "to",
                            match (&(_field_query).to).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "time_range_days",
                            match (&(_field_query).time_range_days).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "bucket",
                            match (&(_field_query).bucket).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::GetRuntimeActivity {
                application_id: _field_application_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("GetRuntimeActivity".to_owned()),
                ),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
            ]),
            Self::GetRuntimeDebugStream {
                application_id: _field_application_id,
                run_id: _field_run_id,
                query: _field_query,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("GetRuntimeDebugStream".to_owned()),
                ),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "run_id",
                    serde_json::Value::String((_field_run_id).to_string()),
                ),
                (
                    "query",
                    mp::object_value(&[
                        (
                            "from_sequence",
                            match (&(_field_query).from_sequence).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "limit",
                            match (&(_field_query).limit).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::GetNodeLastRun {
                application_id: _field_application_id,
                node_id: _field_node_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("GetNodeLastRun".to_owned()),
                ),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                ("node_id", mp::text(_field_node_id)?),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-application-runtime-reads-input";
    const CONTRACT_VERSION: &'static str = "1";
}
