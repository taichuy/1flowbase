pub(super) fn schema() -> Option<serde_json::Value> {
    use crate::extension_bus::managed_projection as mp;
    Some(mp::union_schema(vec![
        mp::object_schema(&[
            ("variant", mp::tag_schema("Content")),
            (
                "0",
                mp::object_schema(&[
                    ("content_type", mp::text_schema()),
                    (
                        "bytes",
                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                    ),
                ]),
            ),
        ]),
        mp::object_schema(&[
            ("variant", mp::tag_schema("Resolved")),
            (
                "0",
                mp::object_schema(&[(
                    "artifacts",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("artifact_ref",mp::object_schema(&[("byte_count",mp::count_schema())])), ("content_type",mp::text_schema()), ("value",mp::json_summary_schema())])}),
                )]),
            ),
        ]),
        mp::object_schema(&[
            ("variant", mp::tag_schema("Snapshot")),
            (
                "0",
                mp::object_schema(&[
                    (
                        "run",
                        mp::object_schema(&[
                            ("id", mp::text_schema()),
                            ("application_id", mp::text_schema()),
                            ("application_type", mp::text_schema()),
                            (
                                "run_object_kind",
                                mp::object_schema(&[("byte_count", mp::count_schema())]),
                            ),
                            (
                                "run_kind",
                                mp::object_schema(&[("byte_count", mp::count_schema())]),
                            ),
                            ("status", mp::text_schema()),
                            (
                                "title",
                                mp::object_schema(&[("byte_count", mp::count_schema())]),
                            ),
                            (
                                "execution_stage",
                                mp::object_schema(&[("byte_count", mp::count_schema())]),
                            ),
                            (
                                "invocation_source",
                                mp::object_schema(&[("byte_count", mp::count_schema())]),
                            ),
                            (
                                "compatibility_mode",
                                serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                            ),
                            (
                                "subject",
                                mp::object_schema(&[
                                    ("kind", mp::text_schema()),
                                    (
                                        "id",
                                        serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                    ),
                                    (
                                        "draft_id",
                                        serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                    ),
                                    (
                                        "target_node_id",
                                        serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                    ),
                                ]),
                            ),
                            (
                                "principal",
                                mp::object_schema(&[
                                    ("kind", mp::text_schema()),
                                    (
                                        "id",
                                        serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                    ),
                                    (
                                        "display_name",
                                        serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                    ),
                                ]),
                            ),
                            (
                                "correlation",
                                mp::object_schema(&[
                                    (
                                        "publication_version_id",
                                        serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                    ),
                                    (
                                        "external_user",
                                        serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                    ),
                                    (
                                        "external_conversation_id",
                                        serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                    ),
                                    (
                                        "external_trace_id",
                                        serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                    ),
                                    (
                                        "compatibility_mode",
                                        serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                    ),
                                    (
                                        "idempotency_key",
                                        serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                    ),
                                ]),
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
                        "statistics",
                        mp::object_schema(&[
                            (
                                "input_cache_hit_rate",
                                serde_json::json!({"anyOf": [serde_json::json!({"type":"number"}), {"type":"null"}]}),
                            ),
                            ("unique_node_count", serde_json::json!({"type":"integer"})),
                            ("tool_callback_count", serde_json::json!({"type":"integer"})),
                        ]),
                    ),
                    (
                        "detail",
                        mp::object_schema(&[
                            ("kind", mp::text_schema()),
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
                                "answer_snapshot",
                                serde_json::json!({"anyOf": [mp::object_schema(&[("kind",mp::text_schema()), ("text",mp::object_schema(&[("byte_count",mp::count_schema())])), ("output_payload",mp::json_summary_schema()), ("complete",serde_json::json!({"type":"boolean"})), ("materialized_from",mp::object_schema(&[("byte_count",mp::count_schema())])), ("answer_node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("answer_node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("waiting_node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("waiting_node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}))]), {"type":"null"}]}),
                            ),
                            (
                                "node_runs",
                                mp::object_schema(&[("item_count", mp::count_schema())]),
                            ),
                            (
                                "checkpoints",
                                mp::object_schema(&[("item_count", mp::count_schema())]),
                            ),
                            (
                                "callback_tasks",
                                mp::object_schema(&[("item_count", mp::count_schema())]),
                            ),
                            (
                                "events",
                                mp::object_schema(&[("item_count", mp::count_schema())]),
                            ),
                            (
                                "stitched_trace",
                                mp::object_schema(&[("item_count", mp::count_schema())]),
                            ),
                        ]),
                    ),
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
                        "answer_snapshot",
                        serde_json::json!({"anyOf": [mp::object_schema(&[("kind",mp::text_schema()), ("text",mp::object_schema(&[("byte_count",mp::count_schema())])), ("output_payload",mp::json_summary_schema()), ("complete",serde_json::json!({"type":"boolean"})), ("materialized_from",mp::object_schema(&[("byte_count",mp::count_schema())])), ("answer_node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("answer_node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("waiting_node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("waiting_node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}))]), {"type":"null"}]}),
                    ),
                    (
                        "context_snapshot",
                        serde_json::json!({"anyOf": [mp::object_schema(&[("event_type",mp::text_schema()), ("event_id",mp::text_schema()), ("run_id",mp::text_schema()), ("node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("node_id",mp::text_schema()), ("sequence",serde_json::json!({"type":"integer"})), ("effective_context_window",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]})), ("measurement",mp::object_schema(&[("method",mp::text_schema()), ("accuracy",mp::object_schema(&[("byte_count",mp::count_schema())])), ("coverage",mp::object_schema(&[("byte_count",mp::count_schema())])), ("unknown_block_count",serde_json::json!({"type":"integer"}))])), ("created_at",mp::text_schema())]), {"type":"null"}]}),
                    ),
                    (
                        "node_runs",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_id",mp::text_schema()), ("node_type",mp::text_schema()), ("node_alias",mp::object_schema(&[("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("input_payload",mp::json_summary_schema()), ("input_payload_view",mp::json_summary_schema()), ("output_payload",mp::json_summary_schema()), ("error_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("metrics_payload",mp::json_summary_schema()), ("debug_payload",mp::json_summary_schema()), ("started_at",mp::object_schema(&[("byte_count",mp::count_schema())])), ("finished_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                    ),
                    (
                        "checkpoints",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("status",mp::text_schema()), ("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("locator_payload",mp::json_summary_schema()), ("variable_snapshot",mp::json_summary_schema()), ("external_ref_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("created_at",mp::text_schema())])}),
                    ),
                    (
                        "callback_tasks",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_run_id",mp::text_schema()), ("callback_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("request_payload",mp::json_summary_schema()), ("response_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("external_ref_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("created_at",mp::text_schema()), ("completed_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                    ),
                    (
                        "events",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("sequence",serde_json::json!({"type":"integer"})), ("event_type",mp::text_schema()), ("payload",mp::json_summary_schema()), ("created_at",mp::text_schema())])}),
                    ),
                    (
                        "stitched_trace",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("source_flow_run",mp::object_schema(&[("id",mp::text_schema()), ("application_id",mp::text_schema()), ("flow_id",mp::text_schema()), ("draft_id",mp::text_schema()), ("compiled_plan_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("run_mode",mp::object_schema(&[("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("target_node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("title",mp::object_schema(&[("byte_count",mp::count_schema())])), ("expand_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("external_conversation_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("query",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("model",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("input_payload",mp::json_summary_schema()), ("output_payload",mp::json_summary_schema()), ("error_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("created_by",mp::object_schema(&[("byte_count",mp::count_schema())])), ("started_at",mp::object_schema(&[("byte_count",mp::count_schema())])), ("finished_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema())])), ("node_runs",mp::object_schema(&[("item_count",mp::count_schema())])), ("callback_tasks",mp::object_schema(&[("item_count",mp::count_schema())])), ("events",mp::object_schema(&[("item_count",mp::count_schema())]))])}),
                    ),
                ]),
            ),
        ]),
    ]))
}
