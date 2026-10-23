use crate::routes::application_runtime::managed_detail_projection;

pub(super) fn schema() -> Option<serde_json::Value> {
    use crate::extension_bus::managed_projection as mp;
    Some(mp::union_schema(vec![
        mp::object_schema(&[
            ("variant", mp::tag_schema("Run")),
            ("0", managed_detail_projection::schema()),
        ]),
        mp::object_schema(&[
            ("variant", mp::tag_schema("Node")),
            (
                "0",
                mp::object_schema(&[
                    (
                        "flow_run",
                        mp::object_schema(&[
                            ("id", mp::text_schema()),
                            ("application_id", mp::text_schema()),
                            ("flow_id", mp::text_schema()),
                            ("draft_id", mp::text_schema()),
                            (
                                "compiled_plan_id",
                                serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                            ),
                            (
                                "run_mode",
                                mp::object_schema(&[("byte_count", mp::count_schema())]),
                            ),
                            ("status", mp::text_schema()),
                            (
                                "target_node_id",
                                serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                            ),
                            (
                                "title",
                                mp::object_schema(&[("byte_count", mp::count_schema())]),
                            ),
                            (
                                "expand_id",
                                serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                            ),
                            (
                                "external_conversation_id",
                                serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                            ),
                            (
                                "query",
                                serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                            ),
                            (
                                "model",
                                serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                            ),
                            ("input_payload", mp::json_summary_schema()),
                            ("output_payload", mp::json_summary_schema()),
                            (
                                "error_payload",
                                serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                            ),
                            (
                                "created_by",
                                mp::object_schema(&[("byte_count", mp::count_schema())]),
                            ),
                            (
                                "started_at",
                                mp::object_schema(&[("byte_count", mp::count_schema())]),
                            ),
                            (
                                "finished_at",
                                serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                            ),
                            ("created_at", mp::text_schema()),
                            ("updated_at", mp::text_schema()),
                        ]),
                    ),
                    (
                        "node_run",
                        mp::object_schema(&[
                            ("id", mp::text_schema()),
                            ("flow_run_id", mp::text_schema()),
                            ("node_id", mp::text_schema()),
                            ("node_type", mp::text_schema()),
                            (
                                "node_alias",
                                mp::object_schema(&[("byte_count", mp::count_schema())]),
                            ),
                            ("status", mp::text_schema()),
                            ("input_payload", mp::json_summary_schema()),
                            ("input_payload_view", mp::json_summary_schema()),
                            ("output_payload", mp::json_summary_schema()),
                            (
                                "error_payload",
                                serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                            ),
                            ("metrics_payload", mp::json_summary_schema()),
                            ("debug_payload", mp::json_summary_schema()),
                            (
                                "started_at",
                                mp::object_schema(&[("byte_count", mp::count_schema())]),
                            ),
                            (
                                "finished_at",
                                serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                            ),
                        ]),
                    ),
                    (
                        "checkpoints",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("status",mp::text_schema()), ("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("locator_payload",mp::json_summary_schema()), ("variable_snapshot",mp::json_summary_schema()), ("external_ref_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("created_at",mp::text_schema())])}),
                    ),
                    (
                        "events",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("sequence",serde_json::json!({"type":"integer"})), ("event_type",mp::text_schema()), ("payload",mp::json_summary_schema()), ("created_at",mp::text_schema())])}),
                    ),
                ]),
            ),
        ]),
    ]))
}
