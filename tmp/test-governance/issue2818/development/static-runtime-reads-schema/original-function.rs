fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema([
                ("variant", mp::tag_schema("LogQueryResult")),
                ("item_count", mp::count_schema()),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("RecordClientTrajectoryPage")),
                ("item_count", mp::count_schema()),
            ]),
            mp::object_schema([("variant", mp::tag_schema("Record"))]),
            mp::object_schema([
                ("variant", mp::tag_schema("RecordConversationPage")),
                ("item_count", mp::count_schema()),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("WorkflowTrajectoryPage")),
                ("item_count", mp::count_schema()),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("WorkflowTrajectoryBody")),
                ("item_count", mp::count_schema()),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("ClientTrajectoryPage")),
                ("item_count", mp::count_schema()),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("ClientTrajectorySection")),
                ("item_count", mp::count_schema()),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("RunPayload")),
                ("payload", mp::json_summary_schema()),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("TrajectoryPage")),
                ("observation_count", mp::count_schema()),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("TrajectoryBody")),
                ("event_id", mp::text_schema()),
                ("body_byte_count", mp::count_schema()),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("Runs")),
                (
                    "0",
                    mp::object_schema([
                        (
                            "items",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("id",mp::text_schema()), ("application_id",mp::text_schema()), ("application_type",mp::text_schema()), ("run_object_kind",mp::object_schema([("byte_count",mp::count_schema())])), ("run_kind",mp::object_schema([("byte_count",mp::count_schema())])), ("run_mode",mp::object_schema([("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("target_node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("title",mp::object_schema([("byte_count",mp::count_schema())])), ("expand_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("execution_stage",mp::object_schema([("byte_count",mp::count_schema())])), ("invocation_source",mp::object_schema([("byte_count",mp::count_schema())])), ("compatibility_mode",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("subject",mp::object_schema([("kind",mp::text_schema()), ("id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("draft_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("target_node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}))])), ("principal",mp::object_schema([("kind",mp::text_schema()), ("id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("display_name",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}))])), ("correlation",mp::object_schema([("publication_version_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("external_user",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("external_conversation_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("external_trace_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("compatibility_mode",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("idempotency_key",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}))])), ("statistics",mp::object_schema([("input_cache_hit_rate",serde_json::json!({"anyOf": [serde_json::json!({"type":"number"}), {"type":"null"}]})), ("unique_node_count",serde_json::json!({"type":"integer"})), ("tool_callback_count",serde_json::json!({"type":"integer"}))])), ("started_at",mp::object_schema([("byte_count",mp::count_schema())])), ("finished_at",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema())])}),
                        ),
                        ("total", serde_json::json!({"type":"integer"})),
                        ("page", serde_json::json!({"type":"integer"})),
                        ("page_size", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("ConversationMessages")),
                (
                    "0",
                    mp::object_schema([
                        (
                            "items",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("run_id",mp::text_schema()), ("detail_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("can_open_detail",serde_json::json!({"type":"boolean"})), ("role",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("content",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("started_at",mp::object_schema([("byte_count",mp::count_schema())])), ("finished_at",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("status",mp::text_schema()), ("query",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("model",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("answer",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("is_current",serde_json::json!({"type":"boolean"}))])}),
                        ),
                        (
                            "page",
                            mp::object_schema([
                                ("has_before", serde_json::json!({"type":"boolean"})),
                                ("has_after", serde_json::json!({"type":"boolean"})),
                                (
                                    "before_cursor",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "after_cursor",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                            ]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("RunOverview")),
                (
                    "0",
                    mp::object_schema([
                        (
                            "run",
                            mp::object_schema([
                                ("id", mp::text_schema()),
                                ("application_id", mp::text_schema()),
                                ("application_type", mp::text_schema()),
                                (
                                    "run_object_kind",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "run_kind",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                ("status", mp::text_schema()),
                                (
                                    "title",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "execution_stage",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "invocation_source",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "compatibility_mode",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "subject",
                                    mp::object_schema([
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
                                    mp::object_schema([
                                        ("kind", mp::text_schema()),
                                        (
                                            "id",
                                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                        ),
                                        (
                                            "display_name",
                                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                    ]),
                                ),
                                (
                                    "correlation",
                                    mp::object_schema([
                                        (
                                            "publication_version_id",
                                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                        ),
                                        (
                                            "external_user",
                                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
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
                                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                        (
                                            "idempotency_key",
                                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                    ]),
                                ),
                                (
                                    "started_at",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "finished_at",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                ("created_at", mp::text_schema()),
                                ("updated_at", mp::text_schema()),
                            ]),
                        ),
                        (
                            "statistics",
                            mp::object_schema([
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
                            mp::object_schema([
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
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                ("status", mp::text_schema()),
                                (
                                    "target_node_id",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                (
                                    "title",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
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
                                    "created_by",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "started_at",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "finished_at",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                ("created_at", mp::text_schema()),
                                ("updated_at", mp::text_schema()),
                            ]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("TraceTree")),
                (
                    "0",
                    mp::object_schema([
                        (
                            "run",
                            mp::object_schema([
                                ("id", mp::text_schema()),
                                ("application_id", mp::text_schema()),
                                ("application_type", mp::text_schema()),
                                (
                                    "run_object_kind",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "run_kind",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                ("status", mp::text_schema()),
                                (
                                    "title",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "execution_stage",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "invocation_source",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "compatibility_mode",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "subject",
                                    mp::object_schema([
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
                                    mp::object_schema([
                                        ("kind", mp::text_schema()),
                                        (
                                            "id",
                                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                        ),
                                        (
                                            "display_name",
                                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                    ]),
                                ),
                                (
                                    "correlation",
                                    mp::object_schema([
                                        (
                                            "publication_version_id",
                                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                        ),
                                        (
                                            "external_user",
                                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
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
                                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                        (
                                            "idempotency_key",
                                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                    ]),
                                ),
                                (
                                    "started_at",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "finished_at",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                ("created_at", mp::text_schema()),
                                ("updated_at", mp::text_schema()),
                            ]),
                        ),
                        (
                            "statistics",
                            mp::object_schema([
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
                            mp::object_schema([
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
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                ("status", mp::text_schema()),
                                (
                                    "target_node_id",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                (
                                    "title",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
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
                                    "created_by",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "started_at",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "finished_at",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                ("created_at", mp::text_schema()),
                                ("updated_at", mp::text_schema()),
                            ]),
                        ),
                        (
                            "projection_status",
                            mp::object_schema([
                                (
                                    "projection_status",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                ("projection_version", serde_json::json!({"type":"integer"})),
                                (
                                    "source_watermark",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                ("attempt_count", serde_json::json!({"type":"integer"})),
                                (
                                    "last_attempt_at",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "last_success_at",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "last_error_code",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                (
                                    "last_error_stage",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "last_error_source_kind",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "last_error_source_locator",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "last_error_ref",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                ("retriable", serde_json::json!({"type":"boolean"})),
                            ]),
                        ),
                        (
                            "nodes",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("trace_node_id",mp::text_schema()), ("stable_locator",mp::object_schema([("byte_count",mp::count_schema())])), ("parent_trace_node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("node_kind",mp::object_schema([("byte_count",mp::count_schema())])), ("flow_run_id",mp::text_schema()), ("node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("callback_task_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("node_type",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("node_mode",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("node_alias",mp::object_schema([("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("started_at",mp::object_schema([("byte_count",mp::count_schema())])), ("finished_at",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("duration_ms",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]})), ("metrics_payload",mp::json_summary_schema()), ("has_children",serde_json::json!({"type":"boolean"})), ("child_count",serde_json::json!({"type":"integer"})), ("has_content",serde_json::json!({"type":"boolean"})), ("source_flow_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("source_trace_node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("parent_callback_task_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("parent_tool_call_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("trace_relation_kind",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("TraceChildren")),
                (
                    "0",
                    mp::object_schema([
                        (
                            "projection_status",
                            mp::object_schema([
                                (
                                    "projection_status",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                ("projection_version", serde_json::json!({"type":"integer"})),
                                (
                                    "source_watermark",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                ("attempt_count", serde_json::json!({"type":"integer"})),
                                (
                                    "last_attempt_at",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "last_success_at",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "last_error_code",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                (
                                    "last_error_stage",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "last_error_source_kind",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "last_error_source_locator",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "last_error_ref",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                ("retriable", serde_json::json!({"type":"boolean"})),
                            ]),
                        ),
                        (
                            "items",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("trace_node_id",mp::text_schema()), ("stable_locator",mp::object_schema([("byte_count",mp::count_schema())])), ("parent_trace_node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("node_kind",mp::object_schema([("byte_count",mp::count_schema())])), ("flow_run_id",mp::text_schema()), ("node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("callback_task_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("node_type",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("node_mode",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("node_alias",mp::object_schema([("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("started_at",mp::object_schema([("byte_count",mp::count_schema())])), ("finished_at",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("duration_ms",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]})), ("metrics_payload",mp::json_summary_schema()), ("has_children",serde_json::json!({"type":"boolean"})), ("child_count",serde_json::json!({"type":"integer"})), ("has_content",serde_json::json!({"type":"boolean"})), ("source_flow_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("source_trace_node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("parent_callback_task_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("parent_tool_call_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("trace_relation_kind",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                        (
                            "page_info",
                            mp::object_schema([
                                ("has_more", serde_json::json!({"type":"boolean"})),
                                (
                                    "next_cursor",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                ("page_size", serde_json::json!({"type":"integer"})),
                            ]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("ResumeTimeline")),
                (
                    "0",
                    mp::object_schema([
                        (
                            "flow_run",
                            mp::object_schema([
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
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                ("status", mp::text_schema()),
                                (
                                    "target_node_id",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                (
                                    "title",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
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
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "model",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                ("input_payload", mp::json_summary_schema()),
                                ("output_payload", mp::json_summary_schema()),
                                (
                                    "error_payload",
                                    serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                                ),
                                (
                                    "created_by",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "started_at",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "finished_at",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                ("created_at", mp::text_schema()),
                                ("updated_at", mp::text_schema()),
                            ]),
                        ),
                        (
                            "callback_tasks",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_run_id",mp::text_schema()), ("callback_kind",mp::object_schema([("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("request_payload",mp::json_summary_schema()), ("response_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("external_ref_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("created_at",mp::text_schema()), ("completed_at",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                        (
                            "events",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("sequence",serde_json::json!({"type":"integer"})), ("event_type",mp::text_schema()), ("payload",mp::json_summary_schema()), ("created_at",mp::text_schema())])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("ResumeTimelineSummary")),
                (
                    "0",
                    mp::object_schema([
                        (
                            "flow_run_status",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "callback_tasks",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("id",mp::text_schema()), ("callback_kind",mp::object_schema([("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("created_at",mp::text_schema()), ("completed_at",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                        (
                            "events",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("id",mp::text_schema()), ("event_type",mp::text_schema()), ("description",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("created_at",mp::text_schema())])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("RunNodeLastRun")),
                (
                    "0",
                    serde_json::json!({"anyOf": [mp::object_schema([("flow_run",mp::object_schema([("id",mp::text_schema()), ("application_id",mp::text_schema()), ("flow_id",mp::text_schema()), ("draft_id",mp::text_schema()), ("compiled_plan_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("run_mode",mp::object_schema([("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("target_node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("title",mp::object_schema([("byte_count",mp::count_schema())])), ("expand_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("external_conversation_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("query",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("model",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("input_payload",mp::json_summary_schema()), ("output_payload",mp::json_summary_schema()), ("error_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("created_by",mp::object_schema([("byte_count",mp::count_schema())])), ("started_at",mp::object_schema([("byte_count",mp::count_schema())])), ("finished_at",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema())])), ("node_run",mp::object_schema([("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_id",mp::text_schema()), ("node_type",mp::text_schema()), ("node_alias",mp::object_schema([("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("input_payload",mp::json_summary_schema()), ("input_payload_view",mp::json_summary_schema()), ("output_payload",mp::json_summary_schema()), ("error_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("metrics_payload",mp::json_summary_schema()), ("debug_payload",mp::json_summary_schema()), ("started_at",mp::object_schema([("byte_count",mp::count_schema())])), ("finished_at",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}))])), ("checkpoints",serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("status",mp::text_schema()), ("reason",mp::object_schema([("byte_count",mp::count_schema())])), ("locator_payload",mp::json_summary_schema()), ("variable_snapshot",mp::json_summary_schema()), ("external_ref_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("created_at",mp::text_schema())])})), ("events",serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("sequence",serde_json::json!({"type":"integer"})), ("event_type",mp::text_schema()), ("payload",mp::json_summary_schema()), ("created_at",mp::text_schema())])}))]), {"type":"null"}]}),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("MonitoringReport")),
                (
                    "0",
                    mp::object_schema([
                        (
                            "meta",
                            mp::object_schema([
                                (
                                    "started_from",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "started_to",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "bucket",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "slow_run_threshold_ms",
                                    serde_json::json!({"type":"integer"}),
                                ),
                            ]),
                        ),
                        (
                            "overview",
                            mp::object_schema([
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
                            mp::object_schema([
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
                            mp::object_schema([
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
                            mp::object_schema([
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
                            mp::object_schema([(
                                "peak_concurrency",
                                serde_json::json!({"type":"integer"}),
                            )]),
                        ),
                        (
                            "protocols",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("protocol",mp::text_schema()), ("request_count",serde_json::json!({"type":"integer"})), ("success_rate",serde_json::json!({"type":"number"})), ("avg_duration_ms",serde_json::json!({"type":"number"}))])}),
                        ),
                        (
                            "sources",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("invocation_source",mp::object_schema([("byte_count",mp::count_schema())])), ("request_count",serde_json::json!({"type":"integer"})), ("success_rate",serde_json::json!({"type":"number"}))])}),
                        ),
                        (
                            "external_conversations",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("external_conversation_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("request_count",serde_json::json!({"type":"integer"})), ("avg_duration_ms",serde_json::json!({"type":"number"})), ("failed_count",serde_json::json!({"type":"integer"}))])}),
                        ),
                        (
                            "slowest_runs",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("flow_run_id",mp::text_schema()), ("title",mp::object_schema([("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("started_at",mp::object_schema([("byte_count",mp::count_schema())])), ("finished_at",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("duration_ms",serde_json::json!({"anyOf": [serde_json::json!({"type":"number"}), {"type":"null"}]}))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("RuntimeActivity")),
                (
                    "0",
                    mp::object_schema([
                        (
                            "meta",
                            mp::object_schema([
                                ("application_id", mp::text_schema()),
                                (
                                    "scope",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "storage",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "instance_started_at",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "snapshot_at",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                            ]),
                        ),
                        (
                            "active",
                            mp::object_schema([
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
                            mp::object_schema([
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
                            mp::object_schema([
                                ("completed", serde_json::json!({"type":"integer"})),
                                ("failed", serde_json::json!({"type":"integer"})),
                                ("cancelled", serde_json::json!({"type":"integer"})),
                                ("disconnected", serde_json::json!({"type":"integer"})),
                            ]),
                        ),
                        (
                            "windows",
                            mp::object_schema([
                                (
                                    "one_minute",
                                    mp::object_schema([
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
                                    mp::object_schema([
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
                                    mp::object_schema([
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
                            mp::object_schema([
                                (
                                    "state",
                                    mp::union_schema(vec![
                                        mp::object_schema([("variant", mp::tag_schema("Healthy"))]),
                                        mp::object_schema([("variant", mp::tag_schema("Busy"))]),
                                        mp::object_schema([("variant", mp::tag_schema("Slow"))]),
                                        mp::object_schema([(
                                            "variant",
                                            mp::tag_schema("Unstable"),
                                        )]),
                                        mp::object_schema([("variant", mp::tag_schema("Failing"))]),
                                        mp::object_schema([(
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
                                        mp::object_schema([("variant", mp::tag_schema("Rising"))]),
                                        mp::object_schema([("variant", mp::tag_schema("Steady"))]),
                                        mp::object_schema([("variant", mp::tag_schema("Falling"))]),
                                    ]),
                                ),
                                ("failure_trend", serde_json::json!({"type":"number"})),
                            ]),
                        ),
                        (
                            "age_distribution",
                            mp::object_schema([
                                ("under_5s", serde_json::json!({"type":"integer"})),
                                ("from_5s_to_30s", serde_json::json!({"type":"integer"})),
                                ("from_30s_to_120s", serde_json::json!({"type":"integer"})),
                                ("over_120s", serde_json::json!({"type":"integer"})),
                            ]),
                        ),
                        (
                            "long_connection_age_distribution",
                            mp::object_schema([
                                ("under_5s", serde_json::json!({"type":"integer"})),
                                ("from_5s_to_30s", serde_json::json!({"type":"integer"})),
                                ("from_30s_to_120s", serde_json::json!({"type":"integer"})),
                                ("over_120s", serde_json::json!({"type":"integer"})),
                            ]),
                        ),
                        (
                            "pressure",
                            mp::object_schema([
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
                            mp::object_schema([(
                                "process_rss_bytes",
                                serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                            )]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("RuntimeDebugStream")),
                (
                    "0",
                    mp::object_schema([
                        (
                            "parts",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("item_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("span_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("part_type",mp::text_schema()), ("status",mp::text_schema()), ("trust_level",mp::object_schema([("byte_count",mp::count_schema())])), ("payload",mp::json_summary_schema())])}),
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
            mp::object_schema([
                ("variant", mp::tag_schema("NodeLastRun")),
                (
                    "0",
                    serde_json::json!({"anyOf": [mp::object_schema([("flow_run",mp::object_schema([("id",mp::text_schema()), ("application_id",mp::text_schema()), ("flow_id",mp::text_schema()), ("draft_id",mp::text_schema()), ("compiled_plan_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("run_mode",mp::object_schema([("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("target_node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("title",mp::object_schema([("byte_count",mp::count_schema())])), ("expand_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("external_conversation_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("query",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("model",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("input_payload",mp::json_summary_schema()), ("output_payload",mp::json_summary_schema()), ("error_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("created_by",mp::object_schema([("byte_count",mp::count_schema())])), ("started_at",mp::object_schema([("byte_count",mp::count_schema())])), ("finished_at",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema())])), ("node_run",mp::object_schema([("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_id",mp::text_schema()), ("node_type",mp::text_schema()), ("node_alias",mp::object_schema([("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("input_payload",mp::json_summary_schema()), ("input_payload_view",mp::json_summary_schema()), ("output_payload",mp::json_summary_schema()), ("error_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("metrics_payload",mp::json_summary_schema()), ("debug_payload",mp::json_summary_schema()), ("started_at",mp::object_schema([("byte_count",mp::count_schema())])), ("finished_at",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}))])), ("checkpoints",serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("status",mp::text_schema()), ("reason",mp::object_schema([("byte_count",mp::count_schema())])), ("locator_payload",mp::json_summary_schema()), ("variable_snapshot",mp::json_summary_schema()), ("external_ref_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("created_at",mp::text_schema())])})), ("events",serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("sequence",serde_json::json!({"type":"integer"})), ("event_type",mp::text_schema()), ("payload",mp::json_summary_schema()), ("created_at",mp::text_schema())])}))]), {"type":"null"}]}),
                ),
            ]),
        ]))
    }