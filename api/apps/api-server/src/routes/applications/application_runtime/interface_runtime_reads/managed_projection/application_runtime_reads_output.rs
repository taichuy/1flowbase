use super::*;

impl InterfaceContract for ApplicationRuntimeReadsOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("GatewayLogs")), ("total", mp::count_schema())]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Runs")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "items",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("application_id",mp::text_schema()), ("application_type",mp::text_schema()), ("run_object_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("run_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("run_mode",mp::object_schema(&[("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("target_node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("title",mp::object_schema(&[("byte_count",mp::count_schema())])), ("expand_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("execution_stage",mp::object_schema(&[("byte_count",mp::count_schema())])), ("invocation_source",mp::object_schema(&[("byte_count",mp::count_schema())])), ("compatibility_mode",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("subject",mp::object_schema(&[("kind",mp::text_schema()), ("id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("draft_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("target_node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}))])), ("principal",mp::object_schema(&[("kind",mp::text_schema()), ("id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("display_name",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])), ("correlation",mp::object_schema(&[("publication_version_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("external_user",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("external_conversation_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("external_trace_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("compatibility_mode",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("idempotency_key",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])), ("statistics",mp::object_schema(&[("input_cache_hit_rate",serde_json::json!({"anyOf": [serde_json::json!({"type":"number"}), {"type":"null"}]})), ("unique_node_count",serde_json::json!({"type":"integer"})), ("tool_callback_count",serde_json::json!({"type":"integer"}))])), ("started_at",mp::object_schema(&[("byte_count",mp::count_schema())])), ("finished_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema())])}),
                        ),
                        ("total", serde_json::json!({"type":"integer"})),
                        ("page", serde_json::json!({"type":"integer"})),
                        ("page_size", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ConversationMessages")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "items",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("run_id",mp::text_schema()), ("detail_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("can_open_detail",serde_json::json!({"type":"boolean"})), ("role",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("content",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("started_at",mp::object_schema(&[("byte_count",mp::count_schema())])), ("finished_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("status",mp::text_schema()), ("query",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("model",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("answer",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("is_current",serde_json::json!({"type":"boolean"}))])}),
                        ),
                        (
                            "page",
                            mp::object_schema(&[
                                ("has_before", serde_json::json!({"type":"boolean"})),
                                ("has_after", serde_json::json!({"type":"boolean"})),
                                (
                                    "before_cursor",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "after_cursor",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                            ]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RunOverview")),
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
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("TraceTree")),
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
                            "nodes",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("trace_node_id",mp::text_schema()), ("stable_locator",mp::object_schema(&[("byte_count",mp::count_schema())])), ("parent_trace_node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("node_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("flow_run_id",mp::text_schema()), ("node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("callback_task_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("node_type",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("node_mode",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("node_alias",mp::object_schema(&[("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("started_at",mp::object_schema(&[("byte_count",mp::count_schema())])), ("finished_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("duration_ms",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]})), ("metrics_payload",mp::json_summary_schema()), ("has_children",serde_json::json!({"type":"boolean"})), ("child_count",serde_json::json!({"type":"integer"})), ("has_content",serde_json::json!({"type":"boolean"})), ("source_flow_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("source_trace_node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("parent_callback_task_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("parent_tool_call_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("trace_relation_kind",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("TraceChildren")),
                (
                    "0",
                    mp::object_schema(&[
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
                            "items",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("trace_node_id",mp::text_schema()), ("stable_locator",mp::object_schema(&[("byte_count",mp::count_schema())])), ("parent_trace_node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("node_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("flow_run_id",mp::text_schema()), ("node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("callback_task_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("node_type",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("node_mode",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("node_alias",mp::object_schema(&[("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("started_at",mp::object_schema(&[("byte_count",mp::count_schema())])), ("finished_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("duration_ms",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]})), ("metrics_payload",mp::json_summary_schema()), ("has_children",serde_json::json!({"type":"boolean"})), ("child_count",serde_json::json!({"type":"integer"})), ("has_content",serde_json::json!({"type":"boolean"})), ("source_flow_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("source_trace_node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("parent_callback_task_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("parent_tool_call_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("trace_relation_kind",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                        (
                            "page_info",
                            mp::object_schema(&[
                                ("has_more", serde_json::json!({"type":"boolean"})),
                                (
                                    "next_cursor",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                ("page_size", serde_json::json!({"type":"integer"})),
                            ]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ResumeTimeline")),
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
                            "callback_tasks",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_run_id",mp::text_schema()), ("callback_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("request_payload",mp::json_summary_schema()), ("response_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("external_ref_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("created_at",mp::text_schema()), ("completed_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                        (
                            "events",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("sequence",serde_json::json!({"type":"integer"})), ("event_type",mp::text_schema()), ("payload",mp::json_summary_schema()), ("created_at",mp::text_schema())])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ResumeTimelineSummary")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "flow_run_status",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "callback_tasks",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("callback_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("created_at",mp::text_schema()), ("completed_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                        (
                            "events",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("event_type",mp::text_schema()), ("description",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("created_at",mp::text_schema())])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RunNodeLastRun")),
                (
                    "0",
                    serde_json::json!({"anyOf": [mp::object_schema(&[("flow_run",mp::object_schema(&[("id",mp::text_schema()), ("application_id",mp::text_schema()), ("flow_id",mp::text_schema()), ("draft_id",mp::text_schema()), ("compiled_plan_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("run_mode",mp::object_schema(&[("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("target_node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("title",mp::object_schema(&[("byte_count",mp::count_schema())])), ("expand_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("external_conversation_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("query",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("model",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("input_payload",mp::json_summary_schema()), ("output_payload",mp::json_summary_schema()), ("error_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("created_by",mp::object_schema(&[("byte_count",mp::count_schema())])), ("started_at",mp::object_schema(&[("byte_count",mp::count_schema())])), ("finished_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema())])), ("node_run",mp::object_schema(&[("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_id",mp::text_schema()), ("node_type",mp::text_schema()), ("node_alias",mp::object_schema(&[("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("input_payload",mp::json_summary_schema()), ("input_payload_view",mp::json_summary_schema()), ("output_payload",mp::json_summary_schema()), ("error_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("metrics_payload",mp::json_summary_schema()), ("debug_payload",mp::json_summary_schema()), ("started_at",mp::object_schema(&[("byte_count",mp::count_schema())])), ("finished_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])), ("checkpoints",serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("status",mp::text_schema()), ("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("locator_payload",mp::json_summary_schema()), ("variable_snapshot",mp::json_summary_schema()), ("external_ref_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("created_at",mp::text_schema())])})), ("events",serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("sequence",serde_json::json!({"type":"integer"})), ("event_type",mp::text_schema()), ("payload",mp::json_summary_schema()), ("created_at",mp::text_schema())])}))]), {"type":"null"}]}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("MonitoringReport")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "meta",
                            mp::object_schema(&[
                                (
                                    "started_from",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "started_to",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "bucket",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "slow_run_threshold_ms",
                                    serde_json::json!({"type":"integer"}),
                                ),
                            ]),
                        ),
                        (
                            "overview",
                            mp::object_schema(&[
                                ("total_count", serde_json::json!({"type":"integer"})),
                                ("success_count", serde_json::json!({"type":"integer"})),
                                ("failed_count", serde_json::json!({"type":"integer"})),
                                ("cancelled_count", serde_json::json!({"type":"integer"})),
                                ("success_rate", serde_json::json!({"type":"number"})),
                                ("failed_rate", serde_json::json!({"type":"number"})),
                                (
                                    "running_count_included",
                                    serde_json::json!({"type":"boolean"}),
                                ),
                            ]),
                        ),
                        (
                            "duration",
                            mp::object_schema(&[
                                (
                                    "duration_recorded_count",
                                    serde_json::json!({"type":"integer"}),
                                ),
                                ("avg_duration_ms", serde_json::json!({"type":"number"})),
                                ("p50_duration_ms", serde_json::json!({"type":"number"})),
                                ("p95_duration_ms", serde_json::json!({"type":"number"})),
                                ("slow_run_rate", serde_json::json!({"type":"number"})),
                            ]),
                        ),
                        (
                            "tool_callbacks",
                            mp::object_schema(&[
                                (
                                    "total_tool_callback_count",
                                    serde_json::json!({"type":"integer"}),
                                ),
                                (
                                    "avg_tool_callback_count",
                                    serde_json::json!({"type":"number"}),
                                ),
                                (
                                    "runs_with_tool_callback",
                                    serde_json::json!({"type":"integer"}),
                                ),
                            ]),
                        ),
                        (
                            "nodes",
                            mp::object_schema(&[
                                (
                                    "avg_unique_node_count",
                                    serde_json::json!({"type":"number"}),
                                ),
                                (
                                    "max_unique_node_count",
                                    serde_json::json!({"type":"integer"}),
                                ),
                            ]),
                        ),
                        (
                            "concurrency",
                            mp::object_schema(&[(
                                "peak_concurrency",
                                serde_json::json!({"type":"integer"}),
                            )]),
                        ),
                        (
                            "protocols",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("protocol",mp::text_schema()), ("request_count",serde_json::json!({"type":"integer"})), ("success_rate",serde_json::json!({"type":"number"})), ("avg_duration_ms",serde_json::json!({"type":"number"}))])}),
                        ),
                        (
                            "sources",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("invocation_source",mp::object_schema(&[("byte_count",mp::count_schema())])), ("request_count",serde_json::json!({"type":"integer"})), ("success_rate",serde_json::json!({"type":"number"}))])}),
                        ),
                        (
                            "external_conversations",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("external_conversation_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("request_count",serde_json::json!({"type":"integer"})), ("avg_duration_ms",serde_json::json!({"type":"number"})), ("failed_count",serde_json::json!({"type":"integer"}))])}),
                        ),
                        (
                            "slowest_runs",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("flow_run_id",mp::text_schema()), ("title",mp::object_schema(&[("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("started_at",mp::object_schema(&[("byte_count",mp::count_schema())])), ("finished_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("duration_ms",serde_json::json!({"anyOf": [serde_json::json!({"type":"number"}), {"type":"null"}]}))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RuntimeActivity")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "meta",
                            mp::object_schema(&[
                                ("application_id", mp::text_schema()),
                                (
                                    "scope",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "storage",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "instance_started_at",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "snapshot_at",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                            ]),
                        ),
                        (
                            "active",
                            mp::object_schema(&[
                                ("total", serde_json::json!({"type":"integer"})),
                                ("http_requests", serde_json::json!({"type":"integer"})),
                                ("sse_connections", serde_json::json!({"type":"integer"})),
                                (
                                    "websocket_connections",
                                    serde_json::json!({"type":"integer"}),
                                ),
                                (
                                    "application_executions",
                                    serde_json::json!({"type":"integer"}),
                                ),
                                ("tool_calls", serde_json::json!({"type":"integer"})),
                                ("model_requests", serde_json::json!({"type":"integer"})),
                                (
                                    "waiting",
                                    serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                                ),
                            ]),
                        ),
                        (
                            "peaks",
                            mp::object_schema(&[
                                (
                                    "process_peak_concurrency",
                                    serde_json::json!({"type":"integer"}),
                                ),
                                (
                                    "recent_peak_concurrency",
                                    serde_json::json!({"type":"integer"}),
                                ),
                            ]),
                        ),
                        (
                            "rolling_minute",
                            mp::object_schema(&[
                                ("completed", serde_json::json!({"type":"integer"})),
                                ("failed", serde_json::json!({"type":"integer"})),
                                ("cancelled", serde_json::json!({"type":"integer"})),
                                ("disconnected", serde_json::json!({"type":"integer"})),
                            ]),
                        ),
                        (
                            "windows",
                            mp::object_schema(&[
                                (
                                    "one_minute",
                                    mp::object_schema(&[
                                        ("window_seconds", serde_json::json!({"type":"integer"})),
                                        ("completed", serde_json::json!({"type":"integer"})),
                                        ("failed", serde_json::json!({"type":"integer"})),
                                        ("cancelled", serde_json::json!({"type":"integer"})),
                                        ("disconnected", serde_json::json!({"type":"integer"})),
                                        ("peak_concurrency", serde_json::json!({"type":"integer"})),
                                        ("failure_rate", serde_json::json!({"type":"number"})),
                                        ("disconnect_rate", serde_json::json!({"type":"number"})),
                                        (
                                            "throughput_per_minute",
                                            serde_json::json!({"type":"number"}),
                                        ),
                                    ]),
                                ),
                                (
                                    "five_minutes",
                                    mp::object_schema(&[
                                        ("window_seconds", serde_json::json!({"type":"integer"})),
                                        ("completed", serde_json::json!({"type":"integer"})),
                                        ("failed", serde_json::json!({"type":"integer"})),
                                        ("cancelled", serde_json::json!({"type":"integer"})),
                                        ("disconnected", serde_json::json!({"type":"integer"})),
                                        ("peak_concurrency", serde_json::json!({"type":"integer"})),
                                        ("failure_rate", serde_json::json!({"type":"number"})),
                                        ("disconnect_rate", serde_json::json!({"type":"number"})),
                                        (
                                            "throughput_per_minute",
                                            serde_json::json!({"type":"number"}),
                                        ),
                                    ]),
                                ),
                                (
                                    "fifteen_minutes",
                                    mp::object_schema(&[
                                        ("window_seconds", serde_json::json!({"type":"integer"})),
                                        ("completed", serde_json::json!({"type":"integer"})),
                                        ("failed", serde_json::json!({"type":"integer"})),
                                        ("cancelled", serde_json::json!({"type":"integer"})),
                                        ("disconnected", serde_json::json!({"type":"integer"})),
                                        ("peak_concurrency", serde_json::json!({"type":"integer"})),
                                        ("failure_rate", serde_json::json!({"type":"number"})),
                                        ("disconnect_rate", serde_json::json!({"type":"number"})),
                                        (
                                            "throughput_per_minute",
                                            serde_json::json!({"type":"number"}),
                                        ),
                                    ]),
                                ),
                            ]),
                        ),
                        (
                            "health",
                            mp::object_schema(&[
                                (
                                    "state",
                                    mp::union_schema(vec![
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("Healthy"),
                                        )]),
                                        mp::object_schema(&[("variant", mp::tag_schema("Busy"))]),
                                        mp::object_schema(&[("variant", mp::tag_schema("Slow"))]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("Unstable"),
                                        )]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("Failing"),
                                        )]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("FailingNow"),
                                        )]),
                                    ]),
                                ),
                                ("failure_rate_1m", serde_json::json!({"type":"number"})),
                                ("failure_rate_5m", serde_json::json!({"type":"number"})),
                                ("failure_rate_15m", serde_json::json!({"type":"number"})),
                                ("disconnect_rate_5m", serde_json::json!({"type":"number"})),
                                ("slow_ratio", serde_json::json!({"type":"number"})),
                                ("active_pressure", serde_json::json!({"type":"number"})),
                                (
                                    "throughput_5m_per_minute",
                                    serde_json::json!({"type":"number"}),
                                ),
                                (
                                    "throughput_15m_per_minute",
                                    serde_json::json!({"type":"number"}),
                                ),
                                (
                                    "throughput_trend",
                                    mp::union_schema(vec![
                                        mp::object_schema(&[("variant", mp::tag_schema("Rising"))]),
                                        mp::object_schema(&[("variant", mp::tag_schema("Steady"))]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("Falling"),
                                        )]),
                                    ]),
                                ),
                                ("failure_trend", serde_json::json!({"type":"number"})),
                            ]),
                        ),
                        (
                            "age_distribution",
                            mp::object_schema(&[
                                ("under_5s", serde_json::json!({"type":"integer"})),
                                ("from_5s_to_30s", serde_json::json!({"type":"integer"})),
                                ("from_30s_to_120s", serde_json::json!({"type":"integer"})),
                                ("over_120s", serde_json::json!({"type":"integer"})),
                            ]),
                        ),
                        (
                            "long_connection_age_distribution",
                            mp::object_schema(&[
                                ("under_5s", serde_json::json!({"type":"integer"})),
                                ("from_5s_to_30s", serde_json::json!({"type":"integer"})),
                                ("from_30s_to_120s", serde_json::json!({"type":"integer"})),
                                ("over_120s", serde_json::json!({"type":"integer"})),
                            ]),
                        ),
                        (
                            "pressure",
                            mp::object_schema(&[
                                (
                                    "slow_active_executions",
                                    serde_json::json!({"type":"integer"}),
                                ),
                                (
                                    "execution_slots_used",
                                    serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                                ),
                                (
                                    "execution_slots_limit",
                                    serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                                ),
                            ]),
                        ),
                        (
                            "resources",
                            mp::object_schema(&[(
                                "process_rss_bytes",
                                serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                            )]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RuntimeDebugStream")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "parts",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("item_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("span_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("part_type",mp::text_schema()), ("status",mp::text_schema()), ("trust_level",mp::object_schema(&[("byte_count",mp::count_schema())])), ("payload",mp::json_summary_schema())])}),
                        ),
                        ("page_size", serde_json::json!({"type":"integer"})),
                        (
                            "next_sequence",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        ("has_more", serde_json::json!({"type":"boolean"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("NodeLastRun")),
                (
                    "0",
                    serde_json::json!({"anyOf": [mp::object_schema(&[("flow_run",mp::object_schema(&[("id",mp::text_schema()), ("application_id",mp::text_schema()), ("flow_id",mp::text_schema()), ("draft_id",mp::text_schema()), ("compiled_plan_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("run_mode",mp::object_schema(&[("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("target_node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("title",mp::object_schema(&[("byte_count",mp::count_schema())])), ("expand_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("external_conversation_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("query",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("model",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("input_payload",mp::json_summary_schema()), ("output_payload",mp::json_summary_schema()), ("error_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("created_by",mp::object_schema(&[("byte_count",mp::count_schema())])), ("started_at",mp::object_schema(&[("byte_count",mp::count_schema())])), ("finished_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema())])), ("node_run",mp::object_schema(&[("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_id",mp::text_schema()), ("node_type",mp::text_schema()), ("node_alias",mp::object_schema(&[("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("input_payload",mp::json_summary_schema()), ("input_payload_view",mp::json_summary_schema()), ("output_payload",mp::json_summary_schema()), ("error_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("metrics_payload",mp::json_summary_schema()), ("debug_payload",mp::json_summary_schema()), ("started_at",mp::object_schema(&[("byte_count",mp::count_schema())])), ("finished_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])), ("checkpoints",serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("status",mp::text_schema()), ("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("locator_payload",mp::json_summary_schema()), ("variable_snapshot",mp::json_summary_schema()), ("external_ref_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("created_at",mp::text_schema())])})), ("events",serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("sequence",serde_json::json!({"type":"integer"})), ("event_type",mp::text_schema()), ("payload",mp::json_summary_schema()), ("created_at",mp::text_schema())])}))]), {"type":"null"}]}),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self { Self::GatewayLogs(page) => mp::object_value(&[("variant", serde_json::json!("GatewayLogs")), ("total", serde_json::json!(page.total))]),Self::Runs(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Runs".to_owned())), ("0",mp::object_value(&[("items",{ if (&(_field_0).items).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).items).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("application_id",mp::text(&(item).application_id)?), ("application_type",mp::text(&(item).application_type)?), ("run_object_kind",mp::object_value(&[("byte_count",serde_json::json!((&(item).run_object_kind).len()))])), ("run_kind",mp::object_value(&[("byte_count",serde_json::json!((&(item).run_kind).len()))])), ("run_mode",mp::object_value(&[("byte_count",serde_json::json!((&(item).run_mode).len()))])), ("status",mp::text(&(item).status)?), ("target_node_id",match (&(item).target_node_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("title",mp::object_value(&[("byte_count",serde_json::json!((&(item).title).len()))])), ("expand_id",match (&(item).expand_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("execution_stage",mp::object_value(&[("byte_count",serde_json::json!((&(item).execution_stage).len()))])), ("invocation_source",mp::object_value(&[("byte_count",serde_json::json!((&(item).invocation_source).len()))])), ("compatibility_mode",match (&(item).compatibility_mode).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("subject",mp::object_value(&[("kind",mp::text(&(&(item).subject).kind)?), ("id",match (&(&(item).subject).id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("draft_id",match (&(&(item).subject).draft_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("target_node_id",match (&(&(item).subject).target_node_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null })])), ("principal",mp::object_value(&[("kind",mp::text(&(&(item).principal).kind)?), ("id",match (&(&(item).principal).id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("display_name",match (&(&(item).principal).display_name).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("correlation",mp::object_value(&[("publication_version_id",match (&(&(item).correlation).publication_version_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("external_user",match (&(&(item).correlation).external_user).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("external_conversation_id",match (&(&(item).correlation).external_conversation_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("external_trace_id",match (&(&(item).correlation).external_trace_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("compatibility_mode",match (&(&(item).correlation).compatibility_mode).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("idempotency_key",match (&(&(item).correlation).idempotency_key).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("statistics",mp::object_value(&[("input_cache_hit_rate",match (&(&(item).statistics).input_cache_hit_rate).as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null }), ("unique_node_count",serde_json::json!(*(&(&(item).statistics).unique_node_count))), ("tool_callback_count",serde_json::json!(*(&(&(item).statistics).tool_callback_count)))])), ("started_at",mp::object_value(&[("byte_count",serde_json::json!((&(item).started_at).len()))])), ("finished_at",match (&(item).finished_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("created_at",mp::text(&(item).created_at)?), ("updated_at",mp::text(&(item).updated_at)?)]))).collect::<Option<Vec<_>>>()?) }), ("total",serde_json::json!(*(&(_field_0).total))), ("page",serde_json::json!(*(&(_field_0).page))), ("page_size",serde_json::json!(*(&(_field_0).page_size)))]))]), Self::ConversationMessages(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("ConversationMessages".to_owned())), ("0",mp::object_value(&[("items",{ if (&(_field_0).items).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).items).iter().map(|item| Some(mp::object_value(&[("run_id",mp::text(&(item).run_id)?), ("detail_run_id",match (&(item).detail_run_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("can_open_detail",serde_json::Value::Bool(*(&(item).can_open_detail))), ("role",match (&(item).role).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("content",match (&(item).content).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("started_at",mp::object_value(&[("byte_count",serde_json::json!((&(item).started_at).len()))])), ("finished_at",match (&(item).finished_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("status",mp::text(&(item).status)?), ("query",match (&(item).query).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("model",match (&(item).model).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("answer",match (&(item).answer).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("is_current",serde_json::Value::Bool(*(&(item).is_current)))]))).collect::<Option<Vec<_>>>()?) }), ("page",mp::object_value(&[("has_before",serde_json::Value::Bool(*(&(&(_field_0).page).has_before))), ("has_after",serde_json::Value::Bool(*(&(&(_field_0).page).has_after))), ("before_cursor",match (&(&(_field_0).page).before_cursor).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("after_cursor",match (&(&(_field_0).page).after_cursor).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))]))]), Self::RunOverview(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("RunOverview".to_owned())), ("0",mp::object_value(&[("run",mp::object_value(&[("id",mp::text(&(&(_field_0).run).id)?), ("application_id",mp::text(&(&(_field_0).run).application_id)?), ("application_type",mp::text(&(&(_field_0).run).application_type)?), ("run_object_kind",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).run).run_object_kind).len()))])), ("run_kind",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).run).run_kind).len()))])), ("status",mp::text(&(&(_field_0).run).status)?), ("title",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).run).title).len()))])), ("execution_stage",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).run).execution_stage).len()))])), ("invocation_source",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).run).invocation_source).len()))])), ("compatibility_mode",match (&(&(_field_0).run).compatibility_mode).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("subject",mp::object_value(&[("kind",mp::text(&(&(&(_field_0).run).subject).kind)?), ("id",match (&(&(&(_field_0).run).subject).id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("draft_id",match (&(&(&(_field_0).run).subject).draft_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("target_node_id",match (&(&(&(_field_0).run).subject).target_node_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null })])), ("principal",mp::object_value(&[("kind",mp::text(&(&(&(_field_0).run).principal).kind)?), ("id",match (&(&(&(_field_0).run).principal).id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("display_name",match (&(&(&(_field_0).run).principal).display_name).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("correlation",mp::object_value(&[("publication_version_id",match (&(&(&(_field_0).run).correlation).publication_version_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("external_user",match (&(&(&(_field_0).run).correlation).external_user).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("external_conversation_id",match (&(&(&(_field_0).run).correlation).external_conversation_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("external_trace_id",match (&(&(&(_field_0).run).correlation).external_trace_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("compatibility_mode",match (&(&(&(_field_0).run).correlation).compatibility_mode).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("idempotency_key",match (&(&(&(_field_0).run).correlation).idempotency_key).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("started_at",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).run).started_at).len()))])), ("finished_at",match (&(&(_field_0).run).finished_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("created_at",mp::text(&(&(_field_0).run).created_at)?), ("updated_at",mp::text(&(&(_field_0).run).updated_at)?)])), ("statistics",mp::object_value(&[("input_cache_hit_rate",match (&(&(_field_0).statistics).input_cache_hit_rate).as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null }), ("unique_node_count",serde_json::json!(*(&(&(_field_0).statistics).unique_node_count))), ("tool_callback_count",serde_json::json!(*(&(&(_field_0).statistics).tool_callback_count)))])), ("flow_run",mp::object_value(&[("id",mp::text(&(&(_field_0).flow_run).id)?), ("application_id",mp::text(&(&(_field_0).flow_run).application_id)?), ("flow_id",mp::text(&(&(_field_0).flow_run).flow_id)?), ("draft_id",mp::text(&(&(_field_0).flow_run).draft_id)?), ("compiled_plan_id",match (&(&(_field_0).flow_run).compiled_plan_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("run_mode",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).flow_run).run_mode).len()))])), ("status",mp::text(&(&(_field_0).flow_run).status)?), ("target_node_id",match (&(&(_field_0).flow_run).target_node_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("title",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).flow_run).title).len()))])), ("expand_id",match (&(&(_field_0).flow_run).expand_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("external_conversation_id",match (&(&(_field_0).flow_run).external_conversation_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("query",match (&(&(_field_0).flow_run).query).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("model",match (&(&(_field_0).flow_run).model).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("input_payload",mp::json_summary(&(&(_field_0).flow_run).input_payload)), ("output_payload",mp::json_summary(&(&(_field_0).flow_run).output_payload)), ("error_payload",match (&(&(_field_0).flow_run).error_payload).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("created_by",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).flow_run).created_by).len()))])), ("started_at",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).flow_run).started_at).len()))])), ("finished_at",match (&(&(_field_0).flow_run).finished_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("created_at",mp::text(&(&(_field_0).flow_run).created_at)?), ("updated_at",mp::text(&(&(_field_0).flow_run).updated_at)?)])), ("answer_snapshot",match (&(_field_0).answer_snapshot).as_ref() { Some(item) => mp::object_value(&[("kind",mp::text(&(item).kind)?), ("text",mp::object_value(&[("byte_count",serde_json::json!((&(item).text).len()))])), ("output_payload",mp::json_summary(&(item).output_payload)), ("complete",serde_json::Value::Bool(*(&(item).complete))), ("materialized_from",mp::object_value(&[("byte_count",serde_json::json!((&(item).materialized_from).len()))])), ("answer_node_id",match (&(item).answer_node_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("answer_node_run_id",match (&(item).answer_node_run_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("waiting_node_id",match (&(item).waiting_node_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("waiting_node_run_id",match (&(item).waiting_node_run_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null })]), None => serde_json::Value::Null })]))]), Self::TraceTree(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("TraceTree".to_owned())), ("0",mp::object_value(&[("run",mp::object_value(&[("id",mp::text(&(&(_field_0).run).id)?), ("application_id",mp::text(&(&(_field_0).run).application_id)?), ("application_type",mp::text(&(&(_field_0).run).application_type)?), ("run_object_kind",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).run).run_object_kind).len()))])), ("run_kind",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).run).run_kind).len()))])), ("status",mp::text(&(&(_field_0).run).status)?), ("title",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).run).title).len()))])), ("execution_stage",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).run).execution_stage).len()))])), ("invocation_source",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).run).invocation_source).len()))])), ("compatibility_mode",match (&(&(_field_0).run).compatibility_mode).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("subject",mp::object_value(&[("kind",mp::text(&(&(&(_field_0).run).subject).kind)?), ("id",match (&(&(&(_field_0).run).subject).id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("draft_id",match (&(&(&(_field_0).run).subject).draft_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("target_node_id",match (&(&(&(_field_0).run).subject).target_node_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null })])), ("principal",mp::object_value(&[("kind",mp::text(&(&(&(_field_0).run).principal).kind)?), ("id",match (&(&(&(_field_0).run).principal).id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("display_name",match (&(&(&(_field_0).run).principal).display_name).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("correlation",mp::object_value(&[("publication_version_id",match (&(&(&(_field_0).run).correlation).publication_version_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("external_user",match (&(&(&(_field_0).run).correlation).external_user).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("external_conversation_id",match (&(&(&(_field_0).run).correlation).external_conversation_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("external_trace_id",match (&(&(&(_field_0).run).correlation).external_trace_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("compatibility_mode",match (&(&(&(_field_0).run).correlation).compatibility_mode).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("idempotency_key",match (&(&(&(_field_0).run).correlation).idempotency_key).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("started_at",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).run).started_at).len()))])), ("finished_at",match (&(&(_field_0).run).finished_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("created_at",mp::text(&(&(_field_0).run).created_at)?), ("updated_at",mp::text(&(&(_field_0).run).updated_at)?)])), ("statistics",mp::object_value(&[("input_cache_hit_rate",match (&(&(_field_0).statistics).input_cache_hit_rate).as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null }), ("unique_node_count",serde_json::json!(*(&(&(_field_0).statistics).unique_node_count))), ("tool_callback_count",serde_json::json!(*(&(&(_field_0).statistics).tool_callback_count)))])), ("flow_run",mp::object_value(&[("id",mp::text(&(&(_field_0).flow_run).id)?), ("application_id",mp::text(&(&(_field_0).flow_run).application_id)?), ("flow_id",mp::text(&(&(_field_0).flow_run).flow_id)?), ("draft_id",mp::text(&(&(_field_0).flow_run).draft_id)?), ("compiled_plan_id",match (&(&(_field_0).flow_run).compiled_plan_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("run_mode",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).flow_run).run_mode).len()))])), ("status",mp::text(&(&(_field_0).flow_run).status)?), ("target_node_id",match (&(&(_field_0).flow_run).target_node_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("title",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).flow_run).title).len()))])), ("expand_id",match (&(&(_field_0).flow_run).expand_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("external_conversation_id",match (&(&(_field_0).flow_run).external_conversation_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("query",match (&(&(_field_0).flow_run).query).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("model",match (&(&(_field_0).flow_run).model).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("input_payload",mp::json_summary(&(&(_field_0).flow_run).input_payload)), ("output_payload",mp::json_summary(&(&(_field_0).flow_run).output_payload)), ("error_payload",match (&(&(_field_0).flow_run).error_payload).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("created_by",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).flow_run).created_by).len()))])), ("started_at",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).flow_run).started_at).len()))])), ("finished_at",match (&(&(_field_0).flow_run).finished_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("created_at",mp::text(&(&(_field_0).flow_run).created_at)?), ("updated_at",mp::text(&(&(_field_0).flow_run).updated_at)?)])), ("answer_snapshot",match (&(_field_0).answer_snapshot).as_ref() { Some(item) => mp::object_value(&[("kind",mp::text(&(item).kind)?), ("text",mp::object_value(&[("byte_count",serde_json::json!((&(item).text).len()))])), ("output_payload",mp::json_summary(&(item).output_payload)), ("complete",serde_json::Value::Bool(*(&(item).complete))), ("materialized_from",mp::object_value(&[("byte_count",serde_json::json!((&(item).materialized_from).len()))])), ("answer_node_id",match (&(item).answer_node_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("answer_node_run_id",match (&(item).answer_node_run_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("waiting_node_id",match (&(item).waiting_node_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("waiting_node_run_id",match (&(item).waiting_node_run_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null })]), None => serde_json::Value::Null }), ("projection_status",mp::object_value(&[("projection_status",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).projection_status).projection_status).len()))])), ("projection_version",serde_json::json!(*(&(&(_field_0).projection_status).projection_version))), ("source_watermark",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).projection_status).source_watermark).len()))])), ("attempt_count",serde_json::json!(*(&(&(_field_0).projection_status).attempt_count))), ("last_attempt_at",match (&(&(_field_0).projection_status).last_attempt_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("last_success_at",match (&(&(_field_0).projection_status).last_success_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("last_error_code",match (&(&(_field_0).projection_status).last_error_code).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("last_error_stage",match (&(&(_field_0).projection_status).last_error_stage).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("last_error_source_kind",match (&(&(_field_0).projection_status).last_error_source_kind).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("last_error_source_locator",match (&(&(_field_0).projection_status).last_error_source_locator).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("last_error_ref",match (&(&(_field_0).projection_status).last_error_ref).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("retriable",serde_json::Value::Bool(*(&(&(_field_0).projection_status).retriable)))])), ("nodes",{ if (&(_field_0).nodes).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).nodes).iter().map(|item| Some(mp::object_value(&[("trace_node_id",mp::text(&(item).trace_node_id)?), ("stable_locator",mp::object_value(&[("byte_count",serde_json::json!((&(item).stable_locator).len()))])), ("parent_trace_node_id",match (&(item).parent_trace_node_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("node_kind",mp::object_value(&[("byte_count",serde_json::json!((&(item).node_kind).len()))])), ("flow_run_id",mp::text(&(item).flow_run_id)?), ("node_run_id",match (&(item).node_run_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("callback_task_id",match (&(item).callback_task_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("node_id",match (&(item).node_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("node_type",match (&(item).node_type).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("node_mode",match (&(item).node_mode).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("node_alias",mp::object_value(&[("byte_count",serde_json::json!((&(item).node_alias).len()))])), ("status",mp::text(&(item).status)?), ("started_at",mp::object_value(&[("byte_count",serde_json::json!((&(item).started_at).len()))])), ("finished_at",match (&(item).finished_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("duration_ms",match (&(item).duration_ms).as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null }), ("metrics_payload",mp::json_summary(&(item).metrics_payload)), ("has_children",serde_json::Value::Bool(*(&(item).has_children))), ("child_count",serde_json::json!(*(&(item).child_count))), ("has_content",serde_json::Value::Bool(*(&(item).has_content))), ("source_flow_run_id",match (&(item).source_flow_run_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("source_trace_node_id",match (&(item).source_trace_node_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("parent_callback_task_id",match (&(item).parent_callback_task_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("parent_tool_call_id",match (&(item).parent_tool_call_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("trace_relation_kind",match (&(item).trace_relation_kind).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?) })]))]), Self::TraceChildren(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("TraceChildren".to_owned())), ("0",mp::object_value(&[("projection_status",mp::object_value(&[("projection_status",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).projection_status).projection_status).len()))])), ("projection_version",serde_json::json!(*(&(&(_field_0).projection_status).projection_version))), ("source_watermark",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).projection_status).source_watermark).len()))])), ("attempt_count",serde_json::json!(*(&(&(_field_0).projection_status).attempt_count))), ("last_attempt_at",match (&(&(_field_0).projection_status).last_attempt_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("last_success_at",match (&(&(_field_0).projection_status).last_success_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("last_error_code",match (&(&(_field_0).projection_status).last_error_code).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("last_error_stage",match (&(&(_field_0).projection_status).last_error_stage).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("last_error_source_kind",match (&(&(_field_0).projection_status).last_error_source_kind).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("last_error_source_locator",match (&(&(_field_0).projection_status).last_error_source_locator).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("last_error_ref",match (&(&(_field_0).projection_status).last_error_ref).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("retriable",serde_json::Value::Bool(*(&(&(_field_0).projection_status).retriable)))])), ("items",{ if (&(_field_0).items).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).items).iter().map(|item| Some(mp::object_value(&[("trace_node_id",mp::text(&(item).trace_node_id)?), ("stable_locator",mp::object_value(&[("byte_count",serde_json::json!((&(item).stable_locator).len()))])), ("parent_trace_node_id",match (&(item).parent_trace_node_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("node_kind",mp::object_value(&[("byte_count",serde_json::json!((&(item).node_kind).len()))])), ("flow_run_id",mp::text(&(item).flow_run_id)?), ("node_run_id",match (&(item).node_run_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("callback_task_id",match (&(item).callback_task_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("node_id",match (&(item).node_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("node_type",match (&(item).node_type).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("node_mode",match (&(item).node_mode).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("node_alias",mp::object_value(&[("byte_count",serde_json::json!((&(item).node_alias).len()))])), ("status",mp::text(&(item).status)?), ("started_at",mp::object_value(&[("byte_count",serde_json::json!((&(item).started_at).len()))])), ("finished_at",match (&(item).finished_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("duration_ms",match (&(item).duration_ms).as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null }), ("metrics_payload",mp::json_summary(&(item).metrics_payload)), ("has_children",serde_json::Value::Bool(*(&(item).has_children))), ("child_count",serde_json::json!(*(&(item).child_count))), ("has_content",serde_json::Value::Bool(*(&(item).has_content))), ("source_flow_run_id",match (&(item).source_flow_run_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("source_trace_node_id",match (&(item).source_trace_node_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("parent_callback_task_id",match (&(item).parent_callback_task_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("parent_tool_call_id",match (&(item).parent_tool_call_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("trace_relation_kind",match (&(item).trace_relation_kind).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?) }), ("page_info",mp::object_value(&[("has_more",serde_json::Value::Bool(*(&(&(_field_0).page_info).has_more))), ("next_cursor",match (&(&(_field_0).page_info).next_cursor).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("page_size",serde_json::json!(*(&(&(_field_0).page_info).page_size)))]))]))]), Self::ResumeTimeline(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("ResumeTimeline".to_owned())), ("0",mp::object_value(&[("flow_run",mp::object_value(&[("id",mp::text(&(&(_field_0).flow_run).id)?), ("application_id",mp::text(&(&(_field_0).flow_run).application_id)?), ("flow_id",mp::text(&(&(_field_0).flow_run).flow_id)?), ("draft_id",mp::text(&(&(_field_0).flow_run).draft_id)?), ("compiled_plan_id",match (&(&(_field_0).flow_run).compiled_plan_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("run_mode",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).flow_run).run_mode).len()))])), ("status",mp::text(&(&(_field_0).flow_run).status)?), ("target_node_id",match (&(&(_field_0).flow_run).target_node_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("title",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).flow_run).title).len()))])), ("expand_id",match (&(&(_field_0).flow_run).expand_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("external_conversation_id",match (&(&(_field_0).flow_run).external_conversation_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("query",match (&(&(_field_0).flow_run).query).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("model",match (&(&(_field_0).flow_run).model).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("input_payload",mp::json_summary(&(&(_field_0).flow_run).input_payload)), ("output_payload",mp::json_summary(&(&(_field_0).flow_run).output_payload)), ("error_payload",match (&(&(_field_0).flow_run).error_payload).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("created_by",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).flow_run).created_by).len()))])), ("started_at",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).flow_run).started_at).len()))])), ("finished_at",match (&(&(_field_0).flow_run).finished_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("created_at",mp::text(&(&(_field_0).flow_run).created_at)?), ("updated_at",mp::text(&(&(_field_0).flow_run).updated_at)?)])), ("callback_tasks",{ if (&(_field_0).callback_tasks).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).callback_tasks).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("flow_run_id",mp::text(&(item).flow_run_id)?), ("node_run_id",mp::text(&(item).node_run_id)?), ("callback_kind",mp::object_value(&[("byte_count",serde_json::json!((&(item).callback_kind).len()))])), ("status",mp::text(&(item).status)?), ("request_payload",mp::json_summary(&(item).request_payload)), ("response_payload",match (&(item).response_payload).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("external_ref_payload",match (&(item).external_ref_payload).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("created_at",mp::text(&(item).created_at)?), ("completed_at",match (&(item).completed_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?) }), ("events",{ if (&(_field_0).events).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).events).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("flow_run_id",mp::text(&(item).flow_run_id)?), ("node_run_id",match (&(item).node_run_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("sequence",serde_json::json!(*(&(item).sequence))), ("event_type",mp::text(&(item).event_type)?), ("payload",mp::json_summary(&(item).payload)), ("created_at",mp::text(&(item).created_at)?)]))).collect::<Option<Vec<_>>>()?) })]))]), Self::ResumeTimelineSummary(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("ResumeTimelineSummary".to_owned())), ("0",mp::object_value(&[("flow_run_status",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).flow_run_status).len()))])), ("callback_tasks",{ if (&(_field_0).callback_tasks).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).callback_tasks).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("callback_kind",mp::object_value(&[("byte_count",serde_json::json!((&(item).callback_kind).len()))])), ("status",mp::text(&(item).status)?), ("created_at",mp::text(&(item).created_at)?), ("completed_at",match (&(item).completed_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?) }), ("events",{ if (&(_field_0).events).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).events).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("event_type",mp::text(&(item).event_type)?), ("description",match (&(item).description).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("created_at",mp::text(&(item).created_at)?)]))).collect::<Option<Vec<_>>>()?) })]))]), Self::RunNodeLastRun(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("RunNodeLastRun".to_owned())), ("0",match (_field_0).as_ref() { Some(item) => mp::object_value(&[("flow_run",mp::object_value(&[("id",mp::text(&(&(item).flow_run).id)?), ("application_id",mp::text(&(&(item).flow_run).application_id)?), ("flow_id",mp::text(&(&(item).flow_run).flow_id)?), ("draft_id",mp::text(&(&(item).flow_run).draft_id)?), ("compiled_plan_id",match (&(&(item).flow_run).compiled_plan_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("run_mode",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).flow_run).run_mode).len()))])), ("status",mp::text(&(&(item).flow_run).status)?), ("target_node_id",match (&(&(item).flow_run).target_node_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("title",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).flow_run).title).len()))])), ("expand_id",match (&(&(item).flow_run).expand_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("external_conversation_id",match (&(&(item).flow_run).external_conversation_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("query",match (&(&(item).flow_run).query).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("model",match (&(&(item).flow_run).model).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("input_payload",mp::json_summary(&(&(item).flow_run).input_payload)), ("output_payload",mp::json_summary(&(&(item).flow_run).output_payload)), ("error_payload",match (&(&(item).flow_run).error_payload).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("created_by",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).flow_run).created_by).len()))])), ("started_at",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).flow_run).started_at).len()))])), ("finished_at",match (&(&(item).flow_run).finished_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("created_at",mp::text(&(&(item).flow_run).created_at)?), ("updated_at",mp::text(&(&(item).flow_run).updated_at)?)])), ("node_run",mp::object_value(&[("id",mp::text(&(&(item).node_run).id)?), ("flow_run_id",mp::text(&(&(item).node_run).flow_run_id)?), ("node_id",mp::text(&(&(item).node_run).node_id)?), ("node_type",mp::text(&(&(item).node_run).node_type)?), ("node_alias",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).node_run).node_alias).len()))])), ("status",mp::text(&(&(item).node_run).status)?), ("input_payload",mp::json_summary(&(&(item).node_run).input_payload)), ("input_payload_view",mp::json_summary(&(&(item).node_run).input_payload_view)), ("output_payload",mp::json_summary(&(&(item).node_run).output_payload)), ("error_payload",match (&(&(item).node_run).error_payload).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("metrics_payload",mp::json_summary(&(&(item).node_run).metrics_payload)), ("debug_payload",mp::json_summary(&(&(item).node_run).debug_payload)), ("started_at",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).node_run).started_at).len()))])), ("finished_at",match (&(&(item).node_run).finished_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("checkpoints",{ if (&(item).checkpoints).len() > 32 { return None; } serde_json::Value::Array((&(item).checkpoints).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("flow_run_id",mp::text(&(item).flow_run_id)?), ("node_run_id",match (&(item).node_run_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("status",mp::text(&(item).status)?), ("reason",mp::object_value(&[("byte_count",serde_json::json!((&(item).reason).len()))])), ("locator_payload",mp::json_summary(&(item).locator_payload)), ("variable_snapshot",mp::json_summary(&(item).variable_snapshot)), ("external_ref_payload",match (&(item).external_ref_payload).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("created_at",mp::text(&(item).created_at)?)]))).collect::<Option<Vec<_>>>()?) }), ("events",{ if (&(item).events).len() > 32 { return None; } serde_json::Value::Array((&(item).events).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("flow_run_id",mp::text(&(item).flow_run_id)?), ("node_run_id",match (&(item).node_run_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("sequence",serde_json::json!(*(&(item).sequence))), ("event_type",mp::text(&(item).event_type)?), ("payload",mp::json_summary(&(item).payload)), ("created_at",mp::text(&(item).created_at)?)]))).collect::<Option<Vec<_>>>()?) })]), None => serde_json::Value::Null })]), Self::MonitoringReport(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("MonitoringReport".to_owned())), ("0",mp::object_value(&[("meta",mp::object_value(&[("started_from",match (&(&(_field_0).meta).started_from).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("started_to",match (&(&(_field_0).meta).started_to).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("bucket",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).meta).bucket).len()))])), ("slow_run_threshold_ms",serde_json::json!(*(&(&(_field_0).meta).slow_run_threshold_ms)))])), ("overview",mp::object_value(&[("total_count",serde_json::json!(*(&(&(_field_0).overview).total_count))), ("success_count",serde_json::json!(*(&(&(_field_0).overview).success_count))), ("failed_count",serde_json::json!(*(&(&(_field_0).overview).failed_count))), ("cancelled_count",serde_json::json!(*(&(&(_field_0).overview).cancelled_count))), ("success_rate",serde_json::json!(*(&(&(_field_0).overview).success_rate))), ("failed_rate",serde_json::json!(*(&(&(_field_0).overview).failed_rate))), ("running_count_included",serde_json::Value::Bool(*(&(&(_field_0).overview).running_count_included)))])), ("duration",mp::object_value(&[("duration_recorded_count",serde_json::json!(*(&(&(_field_0).duration).duration_recorded_count))), ("avg_duration_ms",serde_json::json!(*(&(&(_field_0).duration).avg_duration_ms))), ("p50_duration_ms",serde_json::json!(*(&(&(_field_0).duration).p50_duration_ms))), ("p95_duration_ms",serde_json::json!(*(&(&(_field_0).duration).p95_duration_ms))), ("slow_run_rate",serde_json::json!(*(&(&(_field_0).duration).slow_run_rate)))])), ("tool_callbacks",mp::object_value(&[("total_tool_callback_count",serde_json::json!(*(&(&(_field_0).tool_callbacks).total_tool_callback_count))), ("avg_tool_callback_count",serde_json::json!(*(&(&(_field_0).tool_callbacks).avg_tool_callback_count))), ("runs_with_tool_callback",serde_json::json!(*(&(&(_field_0).tool_callbacks).runs_with_tool_callback)))])), ("nodes",mp::object_value(&[("avg_unique_node_count",serde_json::json!(*(&(&(_field_0).nodes).avg_unique_node_count))), ("max_unique_node_count",serde_json::json!(*(&(&(_field_0).nodes).max_unique_node_count)))])), ("concurrency",mp::object_value(&[("peak_concurrency",serde_json::json!(*(&(&(_field_0).concurrency).peak_concurrency)))])), ("protocols",{ if (&(_field_0).protocols).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).protocols).iter().map(|item| Some(mp::object_value(&[("protocol",mp::text(&(item).protocol)?), ("request_count",serde_json::json!(*(&(item).request_count))), ("success_rate",serde_json::json!(*(&(item).success_rate))), ("avg_duration_ms",serde_json::json!(*(&(item).avg_duration_ms)))]))).collect::<Option<Vec<_>>>()?) }), ("sources",{ if (&(_field_0).sources).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).sources).iter().map(|item| Some(mp::object_value(&[("invocation_source",mp::object_value(&[("byte_count",serde_json::json!((&(item).invocation_source).len()))])), ("request_count",serde_json::json!(*(&(item).request_count))), ("success_rate",serde_json::json!(*(&(item).success_rate)))]))).collect::<Option<Vec<_>>>()?) }), ("external_conversations",{ if (&(_field_0).external_conversations).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).external_conversations).iter().map(|item| Some(mp::object_value(&[("external_conversation_id",match (&(item).external_conversation_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("request_count",serde_json::json!(*(&(item).request_count))), ("avg_duration_ms",serde_json::json!(*(&(item).avg_duration_ms))), ("failed_count",serde_json::json!(*(&(item).failed_count)))]))).collect::<Option<Vec<_>>>()?) }), ("slowest_runs",{ if (&(_field_0).slowest_runs).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).slowest_runs).iter().map(|item| Some(mp::object_value(&[("flow_run_id",mp::text(&(item).flow_run_id)?), ("title",mp::object_value(&[("byte_count",serde_json::json!((&(item).title).len()))])), ("status",mp::text(&(item).status)?), ("started_at",mp::object_value(&[("byte_count",serde_json::json!((&(item).started_at).len()))])), ("finished_at",match (&(item).finished_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("duration_ms",match (&(item).duration_ms).as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?) })]))]), Self::RuntimeActivity(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("RuntimeActivity".to_owned())), ("0",mp::object_value(&[("meta",mp::object_value(&[("application_id",serde_json::Value::String((&(&(_field_0).meta).application_id).to_string())), ("scope",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).meta).scope).len()))])), ("storage",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).meta).storage).len()))])), ("instance_started_at",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).meta).instance_started_at).len()))])), ("snapshot_at",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).meta).snapshot_at).len()))]))])), ("active",mp::object_value(&[("total",serde_json::json!(*(&(&(_field_0).active).total))), ("http_requests",serde_json::json!(*(&(&(_field_0).active).http_requests))), ("sse_connections",serde_json::json!(*(&(&(_field_0).active).sse_connections))), ("websocket_connections",serde_json::json!(*(&(&(_field_0).active).websocket_connections))), ("application_executions",serde_json::json!(*(&(&(_field_0).active).application_executions))), ("tool_calls",serde_json::json!(*(&(&(_field_0).active).tool_calls))), ("model_requests",serde_json::json!(*(&(&(_field_0).active).model_requests))), ("waiting",match (&(&(_field_0).active).waiting).as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null })])), ("peaks",mp::object_value(&[("process_peak_concurrency",serde_json::json!(*(&(&(_field_0).peaks).process_peak_concurrency))), ("recent_peak_concurrency",serde_json::json!(*(&(&(_field_0).peaks).recent_peak_concurrency)))])), ("rolling_minute",mp::object_value(&[("completed",serde_json::json!(*(&(&(_field_0).rolling_minute).completed))), ("failed",serde_json::json!(*(&(&(_field_0).rolling_minute).failed))), ("cancelled",serde_json::json!(*(&(&(_field_0).rolling_minute).cancelled))), ("disconnected",serde_json::json!(*(&(&(_field_0).rolling_minute).disconnected)))])), ("windows",mp::object_value(&[("one_minute",mp::object_value(&[("window_seconds",serde_json::json!(*(&(&(&(_field_0).windows).one_minute).window_seconds))), ("completed",serde_json::json!(*(&(&(&(_field_0).windows).one_minute).completed))), ("failed",serde_json::json!(*(&(&(&(_field_0).windows).one_minute).failed))), ("cancelled",serde_json::json!(*(&(&(&(_field_0).windows).one_minute).cancelled))), ("disconnected",serde_json::json!(*(&(&(&(_field_0).windows).one_minute).disconnected))), ("peak_concurrency",serde_json::json!(*(&(&(&(_field_0).windows).one_minute).peak_concurrency))), ("failure_rate",serde_json::json!(*(&(&(&(_field_0).windows).one_minute).failure_rate))), ("disconnect_rate",serde_json::json!(*(&(&(&(_field_0).windows).one_minute).disconnect_rate))), ("throughput_per_minute",serde_json::json!(*(&(&(&(_field_0).windows).one_minute).throughput_per_minute)))])), ("five_minutes",mp::object_value(&[("window_seconds",serde_json::json!(*(&(&(&(_field_0).windows).five_minutes).window_seconds))), ("completed",serde_json::json!(*(&(&(&(_field_0).windows).five_minutes).completed))), ("failed",serde_json::json!(*(&(&(&(_field_0).windows).five_minutes).failed))), ("cancelled",serde_json::json!(*(&(&(&(_field_0).windows).five_minutes).cancelled))), ("disconnected",serde_json::json!(*(&(&(&(_field_0).windows).five_minutes).disconnected))), ("peak_concurrency",serde_json::json!(*(&(&(&(_field_0).windows).five_minutes).peak_concurrency))), ("failure_rate",serde_json::json!(*(&(&(&(_field_0).windows).five_minutes).failure_rate))), ("disconnect_rate",serde_json::json!(*(&(&(&(_field_0).windows).five_minutes).disconnect_rate))), ("throughput_per_minute",serde_json::json!(*(&(&(&(_field_0).windows).five_minutes).throughput_per_minute)))])), ("fifteen_minutes",mp::object_value(&[("window_seconds",serde_json::json!(*(&(&(&(_field_0).windows).fifteen_minutes).window_seconds))), ("completed",serde_json::json!(*(&(&(&(_field_0).windows).fifteen_minutes).completed))), ("failed",serde_json::json!(*(&(&(&(_field_0).windows).fifteen_minutes).failed))), ("cancelled",serde_json::json!(*(&(&(&(_field_0).windows).fifteen_minutes).cancelled))), ("disconnected",serde_json::json!(*(&(&(&(_field_0).windows).fifteen_minutes).disconnected))), ("peak_concurrency",serde_json::json!(*(&(&(&(_field_0).windows).fifteen_minutes).peak_concurrency))), ("failure_rate",serde_json::json!(*(&(&(&(_field_0).windows).fifteen_minutes).failure_rate))), ("disconnect_rate",serde_json::json!(*(&(&(&(_field_0).windows).fifteen_minutes).disconnect_rate))), ("throughput_per_minute",serde_json::json!(*(&(&(&(_field_0).windows).fifteen_minutes).throughput_per_minute)))]))])), ("health",mp::object_value(&[("state",match &(&(_field_0).health).state {crate::runtime_activity::ApplicationRuntimeActivityHealthState::Healthy => mp::object_value(&[("variant",serde_json::Value::String("Healthy".to_owned()))]), crate::runtime_activity::ApplicationRuntimeActivityHealthState::Busy => mp::object_value(&[("variant",serde_json::Value::String("Busy".to_owned()))]), crate::runtime_activity::ApplicationRuntimeActivityHealthState::Slow => mp::object_value(&[("variant",serde_json::Value::String("Slow".to_owned()))]), crate::runtime_activity::ApplicationRuntimeActivityHealthState::Unstable => mp::object_value(&[("variant",serde_json::Value::String("Unstable".to_owned()))]), crate::runtime_activity::ApplicationRuntimeActivityHealthState::Failing => mp::object_value(&[("variant",serde_json::Value::String("Failing".to_owned()))]), crate::runtime_activity::ApplicationRuntimeActivityHealthState::FailingNow => mp::object_value(&[("variant",serde_json::Value::String("FailingNow".to_owned()))])}), ("failure_rate_1m",serde_json::json!(*(&(&(_field_0).health).failure_rate_1m))), ("failure_rate_5m",serde_json::json!(*(&(&(_field_0).health).failure_rate_5m))), ("failure_rate_15m",serde_json::json!(*(&(&(_field_0).health).failure_rate_15m))), ("disconnect_rate_5m",serde_json::json!(*(&(&(_field_0).health).disconnect_rate_5m))), ("slow_ratio",serde_json::json!(*(&(&(_field_0).health).slow_ratio))), ("active_pressure",serde_json::json!(*(&(&(_field_0).health).active_pressure))), ("throughput_5m_per_minute",serde_json::json!(*(&(&(_field_0).health).throughput_5m_per_minute))), ("throughput_15m_per_minute",serde_json::json!(*(&(&(_field_0).health).throughput_15m_per_minute))), ("throughput_trend",match &(&(_field_0).health).throughput_trend {crate::runtime_activity::ApplicationRuntimeActivityTrend::Rising => mp::object_value(&[("variant",serde_json::Value::String("Rising".to_owned()))]), crate::runtime_activity::ApplicationRuntimeActivityTrend::Steady => mp::object_value(&[("variant",serde_json::Value::String("Steady".to_owned()))]), crate::runtime_activity::ApplicationRuntimeActivityTrend::Falling => mp::object_value(&[("variant",serde_json::Value::String("Falling".to_owned()))])}), ("failure_trend",serde_json::json!(*(&(&(_field_0).health).failure_trend)))])), ("age_distribution",mp::object_value(&[("under_5s",serde_json::json!(*(&(&(_field_0).age_distribution).under_5s))), ("from_5s_to_30s",serde_json::json!(*(&(&(_field_0).age_distribution).from_5s_to_30s))), ("from_30s_to_120s",serde_json::json!(*(&(&(_field_0).age_distribution).from_30s_to_120s))), ("over_120s",serde_json::json!(*(&(&(_field_0).age_distribution).over_120s)))])), ("long_connection_age_distribution",mp::object_value(&[("under_5s",serde_json::json!(*(&(&(_field_0).long_connection_age_distribution).under_5s))), ("from_5s_to_30s",serde_json::json!(*(&(&(_field_0).long_connection_age_distribution).from_5s_to_30s))), ("from_30s_to_120s",serde_json::json!(*(&(&(_field_0).long_connection_age_distribution).from_30s_to_120s))), ("over_120s",serde_json::json!(*(&(&(_field_0).long_connection_age_distribution).over_120s)))])), ("pressure",mp::object_value(&[("slow_active_executions",serde_json::json!(*(&(&(_field_0).pressure).slow_active_executions))), ("execution_slots_used",match (&(&(_field_0).pressure).execution_slots_used).as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null }), ("execution_slots_limit",match (&(&(_field_0).pressure).execution_slots_limit).as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null })])), ("resources",mp::object_value(&[("process_rss_bytes",match (&(&(_field_0).resources).process_rss_bytes).as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null })]))]))]), Self::RuntimeDebugStream(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("RuntimeDebugStream".to_owned())), ("0",mp::object_value(&[("parts",{ if (&(_field_0).parts).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).parts).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("flow_run_id",mp::text(&(item).flow_run_id)?), ("item_id",match (&(item).item_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("span_id",match (&(item).span_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("part_type",mp::text(&(item).part_type)?), ("status",mp::text(&(item).status)?), ("trust_level",mp::object_value(&[("byte_count",serde_json::json!((&(item).trust_level).len()))])), ("payload",mp::json_summary(&(item).payload))]))).collect::<Option<Vec<_>>>()?) }), ("page_size",serde_json::json!(*(&(_field_0).page_size))), ("next_sequence",match (&(_field_0).next_sequence).as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null }), ("has_more",serde_json::Value::Bool(*(&(_field_0).has_more)))]))]), Self::NodeLastRun(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("NodeLastRun".to_owned())), ("0",match (_field_0).as_ref() { Some(item) => mp::object_value(&[("flow_run",mp::object_value(&[("id",mp::text(&(&(item).flow_run).id)?), ("application_id",mp::text(&(&(item).flow_run).application_id)?), ("flow_id",mp::text(&(&(item).flow_run).flow_id)?), ("draft_id",mp::text(&(&(item).flow_run).draft_id)?), ("compiled_plan_id",match (&(&(item).flow_run).compiled_plan_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("run_mode",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).flow_run).run_mode).len()))])), ("status",mp::text(&(&(item).flow_run).status)?), ("target_node_id",match (&(&(item).flow_run).target_node_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("title",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).flow_run).title).len()))])), ("expand_id",match (&(&(item).flow_run).expand_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("external_conversation_id",match (&(&(item).flow_run).external_conversation_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("query",match (&(&(item).flow_run).query).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("model",match (&(&(item).flow_run).model).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("input_payload",mp::json_summary(&(&(item).flow_run).input_payload)), ("output_payload",mp::json_summary(&(&(item).flow_run).output_payload)), ("error_payload",match (&(&(item).flow_run).error_payload).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("created_by",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).flow_run).created_by).len()))])), ("started_at",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).flow_run).started_at).len()))])), ("finished_at",match (&(&(item).flow_run).finished_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("created_at",mp::text(&(&(item).flow_run).created_at)?), ("updated_at",mp::text(&(&(item).flow_run).updated_at)?)])), ("node_run",mp::object_value(&[("id",mp::text(&(&(item).node_run).id)?), ("flow_run_id",mp::text(&(&(item).node_run).flow_run_id)?), ("node_id",mp::text(&(&(item).node_run).node_id)?), ("node_type",mp::text(&(&(item).node_run).node_type)?), ("node_alias",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).node_run).node_alias).len()))])), ("status",mp::text(&(&(item).node_run).status)?), ("input_payload",mp::json_summary(&(&(item).node_run).input_payload)), ("input_payload_view",mp::json_summary(&(&(item).node_run).input_payload_view)), ("output_payload",mp::json_summary(&(&(item).node_run).output_payload)), ("error_payload",match (&(&(item).node_run).error_payload).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("metrics_payload",mp::json_summary(&(&(item).node_run).metrics_payload)), ("debug_payload",mp::json_summary(&(&(item).node_run).debug_payload)), ("started_at",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).node_run).started_at).len()))])), ("finished_at",match (&(&(item).node_run).finished_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("checkpoints",{ if (&(item).checkpoints).len() > 32 { return None; } serde_json::Value::Array((&(item).checkpoints).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("flow_run_id",mp::text(&(item).flow_run_id)?), ("node_run_id",match (&(item).node_run_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("status",mp::text(&(item).status)?), ("reason",mp::object_value(&[("byte_count",serde_json::json!((&(item).reason).len()))])), ("locator_payload",mp::json_summary(&(item).locator_payload)), ("variable_snapshot",mp::json_summary(&(item).variable_snapshot)), ("external_ref_payload",match (&(item).external_ref_payload).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("created_at",mp::text(&(item).created_at)?)]))).collect::<Option<Vec<_>>>()?) }), ("events",{ if (&(item).events).len() > 32 { return None; } serde_json::Value::Array((&(item).events).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("flow_run_id",mp::text(&(item).flow_run_id)?), ("node_run_id",match (&(item).node_run_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("sequence",serde_json::json!(*(&(item).sequence))), ("event_type",mp::text(&(item).event_type)?), ("payload",mp::json_summary(&(item).payload)), ("created_at",mp::text(&(item).created_at)?)]))).collect::<Option<Vec<_>>>()?) })]), None => serde_json::Value::Null })])})
    }

    const CONTRACT_ID: &'static str = "console-application-runtime-reads-output";
    const CONTRACT_VERSION: &'static str = "1";
}
