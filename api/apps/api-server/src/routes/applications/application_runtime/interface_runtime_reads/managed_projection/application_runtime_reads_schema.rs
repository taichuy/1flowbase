//! Static schema description for ApplicationRuntimeReadsOutput only.
use crate::extension_bus::managed_projection as mp;
use serde_json::Value;
use Schema::*;

enum Schema {
    Object(&'static [(&'static str, Schema)]),
    OneOf(&'static [Schema]),
    AnyOf(&'static [Schema]),
    Array {
        max_items: usize,
        items: &'static Schema,
    },
    Text,
    Tag(&'static str),
    Count,
    JsonSummary,
    Integer,
    Number,
    Boolean,
    Null,
}

pub(super) fn build() -> Value {
    build_schema(&DESCRIPTION)
}

fn build_schema(schema: &Schema) -> Value {
    match schema {
        Object(fields) => mp::object_schema(
            fields
                .iter()
                .map(|(name, schema)| (*name, build_schema(schema))),
        ),
        OneOf(variants) => mp::union_schema(variants.iter().map(build_schema).collect()),
        AnyOf(variants) => mp::object_value([(
            "anyOf",
            Value::Array(variants.iter().map(build_schema).collect()),
        )]),
        Array { max_items, items } => mp::object_value([
            ("type", Value::String("array".to_owned())),
            ("maxItems", Value::from(*max_items)),
            ("items", build_schema(items)),
        ]),
        Text => mp::text_schema(),
        Tag(tag) => mp::tag_schema(tag),
        Count => mp::count_schema(),
        JsonSummary => mp::json_summary_schema(),
        Integer => serde_json::json!({"type": "integer"}),
        Number => serde_json::json!({"type": "number"}),
        Boolean => serde_json::json!({"type": "boolean"}),
        Null => serde_json::json!({"type": "null"}),
    }
}

// Repeated shapes remain part of this owner's immutable description.
const BYTE_COUNT: Schema = Object(&[("byte_count", Count)]);
const OPTIONAL_BYTE_COUNT: Schema = AnyOf(&[BYTE_COUNT, Null]);
const OPTIONAL_TEXT: Schema = AnyOf(&[Text, Null]);
const OPTIONAL_INTEGER: Schema = AnyOf(&[Integer, Null]);
const OPTIONAL_NUMBER: Schema = AnyOf(&[Number, Null]);
const OPTIONAL_JSON_SUMMARY: Schema = AnyOf(&[JsonSummary, Null]);
const RUN_HEADER: Schema = Object(&[
    ("id", Text),
    ("application_id", Text),
    ("application_type", Text),
    ("run_object_kind", BYTE_COUNT),
    ("run_kind", BYTE_COUNT),
    ("status", Text),
    ("title", BYTE_COUNT),
    ("execution_stage", BYTE_COUNT),
    ("invocation_source", BYTE_COUNT),
    ("compatibility_mode", OPTIONAL_BYTE_COUNT),
    (
        "subject",
        Object(&[
            ("kind", Text),
            ("id", OPTIONAL_TEXT),
            ("draft_id", OPTIONAL_TEXT),
            ("target_node_id", OPTIONAL_TEXT),
        ]),
    ),
    (
        "principal",
        Object(&[
            ("kind", Text),
            ("id", OPTIONAL_TEXT),
            ("display_name", OPTIONAL_BYTE_COUNT),
        ]),
    ),
    (
        "correlation",
        Object(&[
            ("publication_version_id", OPTIONAL_TEXT),
            ("external_user", OPTIONAL_BYTE_COUNT),
            ("external_conversation_id", OPTIONAL_TEXT),
            ("external_trace_id", OPTIONAL_TEXT),
            ("compatibility_mode", OPTIONAL_BYTE_COUNT),
            ("idempotency_key", OPTIONAL_BYTE_COUNT),
        ]),
    ),
    ("started_at", BYTE_COUNT),
    ("finished_at", OPTIONAL_BYTE_COUNT),
    ("created_at", Text),
    ("updated_at", Text),
]);
const FLOW_RUN_WITH_PAYLOAD: Schema = Object(&[
    ("id", Text),
    ("application_id", Text),
    ("flow_id", Text),
    ("draft_id", Text),
    ("compiled_plan_id", OPTIONAL_TEXT),
    ("run_mode", BYTE_COUNT),
    ("status", Text),
    ("target_node_id", OPTIONAL_TEXT),
    ("title", BYTE_COUNT),
    ("expand_id", OPTIONAL_TEXT),
    ("external_conversation_id", OPTIONAL_TEXT),
    ("query", OPTIONAL_BYTE_COUNT),
    ("model", OPTIONAL_BYTE_COUNT),
    ("input_payload", JsonSummary),
    ("output_payload", JsonSummary),
    ("error_payload", OPTIONAL_JSON_SUMMARY),
    ("created_by", BYTE_COUNT),
    ("started_at", BYTE_COUNT),
    ("finished_at", OPTIONAL_BYTE_COUNT),
    ("created_at", Text),
    ("updated_at", Text),
]);
const TRACE_NODE: Schema = Object(&[
    ("trace_node_id", Text),
    ("stable_locator", BYTE_COUNT),
    ("parent_trace_node_id", OPTIONAL_TEXT),
    ("node_kind", BYTE_COUNT),
    ("flow_run_id", Text),
    ("node_run_id", OPTIONAL_TEXT),
    ("callback_task_id", OPTIONAL_TEXT),
    ("node_id", OPTIONAL_TEXT),
    ("node_type", OPTIONAL_TEXT),
    ("node_mode", OPTIONAL_BYTE_COUNT),
    ("node_alias", BYTE_COUNT),
    ("status", Text),
    ("started_at", BYTE_COUNT),
    ("finished_at", OPTIONAL_BYTE_COUNT),
    ("duration_ms", OPTIONAL_INTEGER),
    ("metrics_payload", JsonSummary),
    ("has_children", Boolean),
    ("child_count", Integer),
    ("has_content", Boolean),
    ("source_flow_run_id", OPTIONAL_TEXT),
    ("source_trace_node_id", OPTIONAL_TEXT),
    ("parent_callback_task_id", OPTIONAL_TEXT),
    ("parent_tool_call_id", OPTIONAL_TEXT),
    ("trace_relation_kind", OPTIONAL_BYTE_COUNT),
]);
const NODE_LAST_RUN: Schema = AnyOf(&[
    Object(&[
        ("flow_run", FLOW_RUN_WITH_PAYLOAD),
        (
            "node_run",
            Object(&[
                ("id", Text),
                ("flow_run_id", Text),
                ("node_id", Text),
                ("node_type", Text),
                ("node_alias", BYTE_COUNT),
                ("status", Text),
                ("input_payload", JsonSummary),
                ("input_payload_view", JsonSummary),
                ("output_payload", JsonSummary),
                ("error_payload", OPTIONAL_JSON_SUMMARY),
                ("metrics_payload", JsonSummary),
                ("debug_payload", JsonSummary),
                ("started_at", BYTE_COUNT),
                ("finished_at", OPTIONAL_BYTE_COUNT),
            ]),
        ),
        (
            "checkpoints",
            Array {
                max_items: 32,
                items: &Object(&[
                    ("id", Text),
                    ("flow_run_id", Text),
                    ("node_run_id", OPTIONAL_TEXT),
                    ("status", Text),
                    ("reason", BYTE_COUNT),
                    ("locator_payload", JsonSummary),
                    ("variable_snapshot", JsonSummary),
                    ("external_ref_payload", OPTIONAL_JSON_SUMMARY),
                    ("created_at", Text),
                ]),
            },
        ),
        (
            "events",
            Array {
                max_items: 32,
                items: &Object(&[
                    ("id", Text),
                    ("flow_run_id", Text),
                    ("node_run_id", OPTIONAL_TEXT),
                    ("sequence", Integer),
                    ("event_type", Text),
                    ("payload", JsonSummary),
                    ("created_at", Text),
                ]),
            },
        ),
    ]),
    Null,
]);

// Field and variant order follow the typed owner's original schema exactly.
static DESCRIPTION: Schema = OneOf(&[
    Object(&[("variant", Tag("LogQueryResult")), ("item_count", Count)]),
    Object(&[
        ("variant", Tag("RecordClientTrajectoryPage")),
        ("item_count", Count),
    ]),
    Object(&[("variant", Tag("Record"))]),
    Object(&[
        ("variant", Tag("RecordConversationPage")),
        ("item_count", Count),
    ]),
    Object(&[
        ("variant", Tag("WorkflowTrajectoryPage")),
        ("item_count", Count),
    ]),
    Object(&[
        ("variant", Tag("WorkflowTrajectoryBody")),
        ("item_count", Count),
    ]),
    Object(&[
        ("variant", Tag("ClientTrajectoryPage")),
        ("item_count", Count),
    ]),
    Object(&[
        ("variant", Tag("ClientTrajectorySection")),
        ("item_count", Count),
    ]),
    Object(&[("variant", Tag("RunPayload")), ("payload", JsonSummary)]),
    Object(&[
        ("variant", Tag("TrajectoryPage")),
        ("observation_count", Count),
    ]),
    Object(&[
        ("variant", Tag("TrajectoryBody")),
        ("event_id", Text),
        ("body_byte_count", Count),
    ]),
    Object(&[
        ("variant", Tag("Runs")),
        (
            "0",
            Object(&[
                (
                    "items",
                    Array {
                        max_items: 32,
                        items: &Object(&[
                            ("id", Text),
                            ("application_id", Text),
                            ("application_type", Text),
                            ("run_object_kind", BYTE_COUNT),
                            ("run_kind", BYTE_COUNT),
                            ("run_mode", BYTE_COUNT),
                            ("status", Text),
                            ("target_node_id", OPTIONAL_TEXT),
                            ("title", BYTE_COUNT),
                            ("expand_id", OPTIONAL_TEXT),
                            ("execution_stage", BYTE_COUNT),
                            ("invocation_source", BYTE_COUNT),
                            ("compatibility_mode", OPTIONAL_BYTE_COUNT),
                            (
                                "subject",
                                Object(&[
                                    ("kind", Text),
                                    ("id", OPTIONAL_TEXT),
                                    ("draft_id", OPTIONAL_TEXT),
                                    ("target_node_id", OPTIONAL_TEXT),
                                ]),
                            ),
                            (
                                "principal",
                                Object(&[
                                    ("kind", Text),
                                    ("id", OPTIONAL_TEXT),
                                    ("display_name", OPTIONAL_BYTE_COUNT),
                                ]),
                            ),
                            (
                                "correlation",
                                Object(&[
                                    ("publication_version_id", OPTIONAL_TEXT),
                                    ("external_user", OPTIONAL_BYTE_COUNT),
                                    ("external_conversation_id", OPTIONAL_TEXT),
                                    ("external_trace_id", OPTIONAL_TEXT),
                                    ("compatibility_mode", OPTIONAL_BYTE_COUNT),
                                    ("idempotency_key", OPTIONAL_BYTE_COUNT),
                                ]),
                            ),
                            (
                                "statistics",
                                Object(&[
                                    ("input_cache_hit_rate", OPTIONAL_NUMBER),
                                    ("unique_node_count", Integer),
                                    ("tool_callback_count", Integer),
                                ]),
                            ),
                            ("started_at", BYTE_COUNT),
                            ("finished_at", OPTIONAL_BYTE_COUNT),
                            ("created_at", Text),
                            ("updated_at", Text),
                        ]),
                    },
                ),
                ("total", Integer),
                ("page", Integer),
                ("page_size", Integer),
            ]),
        ),
    ]),
    Object(&[
        ("variant", Tag("ConversationMessages")),
        (
            "0",
            Object(&[
                (
                    "items",
                    Array {
                        max_items: 32,
                        items: &Object(&[
                            ("run_id", Text),
                            ("detail_run_id", OPTIONAL_TEXT),
                            ("can_open_detail", Boolean),
                            ("role", OPTIONAL_BYTE_COUNT),
                            ("content", OPTIONAL_BYTE_COUNT),
                            ("started_at", BYTE_COUNT),
                            ("finished_at", OPTIONAL_BYTE_COUNT),
                            ("status", Text),
                            ("query", OPTIONAL_BYTE_COUNT),
                            ("model", OPTIONAL_BYTE_COUNT),
                            ("answer", OPTIONAL_BYTE_COUNT),
                            ("is_current", Boolean),
                        ]),
                    },
                ),
                (
                    "page",
                    Object(&[
                        ("has_before", Boolean),
                        ("has_after", Boolean),
                        ("before_cursor", OPTIONAL_BYTE_COUNT),
                        ("after_cursor", OPTIONAL_BYTE_COUNT),
                    ]),
                ),
            ]),
        ),
    ]),
    Object(&[
        ("variant", Tag("RunOverview")),
        (
            "0",
            Object(&[
                ("run", RUN_HEADER),
                (
                    "statistics",
                    Object(&[
                        ("input_cache_hit_rate", OPTIONAL_NUMBER),
                        ("unique_node_count", Integer),
                        ("tool_callback_count", Integer),
                    ]),
                ),
                (
                    "flow_run",
                    Object(&[
                        ("id", Text),
                        ("application_id", Text),
                        ("flow_id", Text),
                        ("draft_id", Text),
                        ("compiled_plan_id", OPTIONAL_TEXT),
                        ("run_mode", BYTE_COUNT),
                        ("status", Text),
                        ("target_node_id", OPTIONAL_TEXT),
                        ("title", BYTE_COUNT),
                        ("expand_id", OPTIONAL_TEXT),
                        ("external_conversation_id", OPTIONAL_TEXT),
                        ("created_by", BYTE_COUNT),
                        ("started_at", BYTE_COUNT),
                        ("finished_at", OPTIONAL_BYTE_COUNT),
                        ("created_at", Text),
                        ("updated_at", Text),
                    ]),
                ),
            ]),
        ),
    ]),
    Object(&[
        ("variant", Tag("TraceTree")),
        (
            "0",
            Object(&[
                ("run", RUN_HEADER),
                (
                    "statistics",
                    Object(&[
                        ("input_cache_hit_rate", OPTIONAL_NUMBER),
                        ("unique_node_count", Integer),
                        ("tool_callback_count", Integer),
                    ]),
                ),
                (
                    "flow_run",
                    Object(&[
                        ("id", Text),
                        ("application_id", Text),
                        ("flow_id", Text),
                        ("draft_id", Text),
                        ("compiled_plan_id", OPTIONAL_TEXT),
                        ("run_mode", BYTE_COUNT),
                        ("status", Text),
                        ("target_node_id", OPTIONAL_TEXT),
                        ("title", BYTE_COUNT),
                        ("expand_id", OPTIONAL_TEXT),
                        ("external_conversation_id", OPTIONAL_TEXT),
                        ("created_by", BYTE_COUNT),
                        ("started_at", BYTE_COUNT),
                        ("finished_at", OPTIONAL_BYTE_COUNT),
                        ("created_at", Text),
                        ("updated_at", Text),
                    ]),
                ),
                (
                    "projection_status",
                    Object(&[
                        ("projection_status", BYTE_COUNT),
                        ("projection_version", Integer),
                        ("source_watermark", BYTE_COUNT),
                        ("attempt_count", Integer),
                        ("last_attempt_at", OPTIONAL_BYTE_COUNT),
                        ("last_success_at", OPTIONAL_BYTE_COUNT),
                        ("last_error_code", OPTIONAL_TEXT),
                        ("last_error_stage", OPTIONAL_BYTE_COUNT),
                        ("last_error_source_kind", OPTIONAL_BYTE_COUNT),
                        ("last_error_source_locator", OPTIONAL_BYTE_COUNT),
                        ("last_error_ref", OPTIONAL_BYTE_COUNT),
                        ("retriable", Boolean),
                    ]),
                ),
                (
                    "nodes",
                    Array {
                        max_items: 32,
                        items: &TRACE_NODE,
                    },
                ),
            ]),
        ),
    ]),
    Object(&[
        ("variant", Tag("TraceChildren")),
        (
            "0",
            Object(&[
                (
                    "projection_status",
                    Object(&[
                        ("projection_status", BYTE_COUNT),
                        ("projection_version", Integer),
                        ("source_watermark", BYTE_COUNT),
                        ("attempt_count", Integer),
                        ("last_attempt_at", OPTIONAL_BYTE_COUNT),
                        ("last_success_at", OPTIONAL_BYTE_COUNT),
                        ("last_error_code", OPTIONAL_TEXT),
                        ("last_error_stage", OPTIONAL_BYTE_COUNT),
                        ("last_error_source_kind", OPTIONAL_BYTE_COUNT),
                        ("last_error_source_locator", OPTIONAL_BYTE_COUNT),
                        ("last_error_ref", OPTIONAL_BYTE_COUNT),
                        ("retriable", Boolean),
                    ]),
                ),
                (
                    "items",
                    Array {
                        max_items: 32,
                        items: &TRACE_NODE,
                    },
                ),
                (
                    "page_info",
                    Object(&[
                        ("has_more", Boolean),
                        ("next_cursor", OPTIONAL_BYTE_COUNT),
                        ("page_size", Integer),
                    ]),
                ),
            ]),
        ),
    ]),
    Object(&[
        ("variant", Tag("ResumeTimeline")),
        (
            "0",
            Object(&[
                ("flow_run", FLOW_RUN_WITH_PAYLOAD),
                (
                    "callback_tasks",
                    Array {
                        max_items: 32,
                        items: &Object(&[
                            ("id", Text),
                            ("flow_run_id", Text),
                            ("node_run_id", Text),
                            ("callback_kind", BYTE_COUNT),
                            ("status", Text),
                            ("request_payload", JsonSummary),
                            ("response_payload", OPTIONAL_JSON_SUMMARY),
                            ("external_ref_payload", OPTIONAL_JSON_SUMMARY),
                            ("created_at", Text),
                            ("completed_at", OPTIONAL_BYTE_COUNT),
                        ]),
                    },
                ),
                (
                    "events",
                    Array {
                        max_items: 32,
                        items: &Object(&[
                            ("id", Text),
                            ("flow_run_id", Text),
                            ("node_run_id", OPTIONAL_TEXT),
                            ("sequence", Integer),
                            ("event_type", Text),
                            ("payload", JsonSummary),
                            ("created_at", Text),
                        ]),
                    },
                ),
            ]),
        ),
    ]),
    Object(&[
        ("variant", Tag("ResumeTimelineSummary")),
        (
            "0",
            Object(&[
                ("flow_run_status", BYTE_COUNT),
                (
                    "callback_tasks",
                    Array {
                        max_items: 32,
                        items: &Object(&[
                            ("id", Text),
                            ("callback_kind", BYTE_COUNT),
                            ("status", Text),
                            ("created_at", Text),
                            ("completed_at", OPTIONAL_BYTE_COUNT),
                        ]),
                    },
                ),
                (
                    "events",
                    Array {
                        max_items: 32,
                        items: &Object(&[
                            ("id", Text),
                            ("event_type", Text),
                            ("description", OPTIONAL_BYTE_COUNT),
                            ("created_at", Text),
                        ]),
                    },
                ),
            ]),
        ),
    ]),
    Object(&[("variant", Tag("RunNodeLastRun")), ("0", NODE_LAST_RUN)]),
    Object(&[
        ("variant", Tag("MonitoringReport")),
        (
            "0",
            Object(&[
                (
                    "meta",
                    Object(&[
                        ("started_from", OPTIONAL_BYTE_COUNT),
                        ("started_to", OPTIONAL_BYTE_COUNT),
                        ("bucket", BYTE_COUNT),
                        ("slow_run_threshold_ms", Integer),
                    ]),
                ),
                (
                    "overview",
                    Object(&[
                        ("total_count", Integer),
                        ("success_count", Integer),
                        ("failed_count", Integer),
                        ("cancelled_count", Integer),
                        ("success_rate", Number),
                        ("failed_rate", Number),
                        ("running_count_included", Boolean),
                    ]),
                ),
                (
                    "duration",
                    Object(&[
                        ("duration_recorded_count", Integer),
                        ("avg_duration_ms", Number),
                        ("p50_duration_ms", Number),
                        ("p95_duration_ms", Number),
                        ("slow_run_rate", Number),
                    ]),
                ),
                (
                    "tool_callbacks",
                    Object(&[
                        ("total_tool_callback_count", Integer),
                        ("avg_tool_callback_count", Number),
                        ("runs_with_tool_callback", Integer),
                    ]),
                ),
                (
                    "nodes",
                    Object(&[
                        ("avg_unique_node_count", Number),
                        ("max_unique_node_count", Integer),
                    ]),
                ),
                ("concurrency", Object(&[("peak_concurrency", Integer)])),
                (
                    "protocols",
                    Array {
                        max_items: 32,
                        items: &Object(&[
                            ("protocol", Text),
                            ("request_count", Integer),
                            ("success_rate", Number),
                            ("avg_duration_ms", Number),
                        ]),
                    },
                ),
                (
                    "sources",
                    Array {
                        max_items: 32,
                        items: &Object(&[
                            ("invocation_source", BYTE_COUNT),
                            ("request_count", Integer),
                            ("success_rate", Number),
                        ]),
                    },
                ),
                (
                    "external_conversations",
                    Array {
                        max_items: 32,
                        items: &Object(&[
                            ("external_conversation_id", OPTIONAL_TEXT),
                            ("request_count", Integer),
                            ("avg_duration_ms", Number),
                            ("failed_count", Integer),
                        ]),
                    },
                ),
                (
                    "slowest_runs",
                    Array {
                        max_items: 32,
                        items: &Object(&[
                            ("flow_run_id", Text),
                            ("title", BYTE_COUNT),
                            ("status", Text),
                            ("started_at", BYTE_COUNT),
                            ("finished_at", OPTIONAL_BYTE_COUNT),
                            ("duration_ms", OPTIONAL_NUMBER),
                        ]),
                    },
                ),
            ]),
        ),
    ]),
    Object(&[
        ("variant", Tag("RuntimeActivity")),
        (
            "0",
            Object(&[
                (
                    "meta",
                    Object(&[
                        ("application_id", Text),
                        ("scope", BYTE_COUNT),
                        ("storage", BYTE_COUNT),
                        ("instance_started_at", BYTE_COUNT),
                        ("snapshot_at", BYTE_COUNT),
                    ]),
                ),
                (
                    "active",
                    Object(&[
                        ("total", Integer),
                        ("http_requests", Integer),
                        ("sse_connections", Integer),
                        ("websocket_connections", Integer),
                        ("application_executions", Integer),
                        ("tool_calls", Integer),
                        ("model_requests", Integer),
                        ("waiting", OPTIONAL_INTEGER),
                    ]),
                ),
                (
                    "peaks",
                    Object(&[
                        ("process_peak_concurrency", Integer),
                        ("recent_peak_concurrency", Integer),
                    ]),
                ),
                (
                    "rolling_minute",
                    Object(&[
                        ("completed", Integer),
                        ("failed", Integer),
                        ("cancelled", Integer),
                        ("disconnected", Integer),
                    ]),
                ),
                (
                    "windows",
                    Object(&[
                        (
                            "one_minute",
                            Object(&[
                                ("window_seconds", Integer),
                                ("completed", Integer),
                                ("failed", Integer),
                                ("cancelled", Integer),
                                ("disconnected", Integer),
                                ("peak_concurrency", Integer),
                                ("failure_rate", Number),
                                ("disconnect_rate", Number),
                                ("throughput_per_minute", Number),
                            ]),
                        ),
                        (
                            "five_minutes",
                            Object(&[
                                ("window_seconds", Integer),
                                ("completed", Integer),
                                ("failed", Integer),
                                ("cancelled", Integer),
                                ("disconnected", Integer),
                                ("peak_concurrency", Integer),
                                ("failure_rate", Number),
                                ("disconnect_rate", Number),
                                ("throughput_per_minute", Number),
                            ]),
                        ),
                        (
                            "fifteen_minutes",
                            Object(&[
                                ("window_seconds", Integer),
                                ("completed", Integer),
                                ("failed", Integer),
                                ("cancelled", Integer),
                                ("disconnected", Integer),
                                ("peak_concurrency", Integer),
                                ("failure_rate", Number),
                                ("disconnect_rate", Number),
                                ("throughput_per_minute", Number),
                            ]),
                        ),
                    ]),
                ),
                (
                    "health",
                    Object(&[
                        (
                            "state",
                            OneOf(&[
                                Object(&[("variant", Tag("Healthy"))]),
                                Object(&[("variant", Tag("Busy"))]),
                                Object(&[("variant", Tag("Slow"))]),
                                Object(&[("variant", Tag("Unstable"))]),
                                Object(&[("variant", Tag("Failing"))]),
                                Object(&[("variant", Tag("FailingNow"))]),
                            ]),
                        ),
                        ("failure_rate_1m", Number),
                        ("failure_rate_5m", Number),
                        ("failure_rate_15m", Number),
                        ("disconnect_rate_5m", Number),
                        ("slow_ratio", Number),
                        ("active_pressure", Number),
                        ("throughput_5m_per_minute", Number),
                        ("throughput_15m_per_minute", Number),
                        (
                            "throughput_trend",
                            OneOf(&[
                                Object(&[("variant", Tag("Rising"))]),
                                Object(&[("variant", Tag("Steady"))]),
                                Object(&[("variant", Tag("Falling"))]),
                            ]),
                        ),
                        ("failure_trend", Number),
                    ]),
                ),
                (
                    "age_distribution",
                    Object(&[
                        ("under_5s", Integer),
                        ("from_5s_to_30s", Integer),
                        ("from_30s_to_120s", Integer),
                        ("over_120s", Integer),
                    ]),
                ),
                (
                    "long_connection_age_distribution",
                    Object(&[
                        ("under_5s", Integer),
                        ("from_5s_to_30s", Integer),
                        ("from_30s_to_120s", Integer),
                        ("over_120s", Integer),
                    ]),
                ),
                (
                    "pressure",
                    Object(&[
                        ("slow_active_executions", Integer),
                        ("execution_slots_used", OPTIONAL_INTEGER),
                        ("execution_slots_limit", OPTIONAL_INTEGER),
                    ]),
                ),
                (
                    "resources",
                    Object(&[("process_rss_bytes", OPTIONAL_INTEGER)]),
                ),
            ]),
        ),
    ]),
    Object(&[
        ("variant", Tag("RuntimeDebugStream")),
        (
            "0",
            Object(&[
                (
                    "parts",
                    Array {
                        max_items: 32,
                        items: &Object(&[
                            ("id", Text),
                            ("flow_run_id", Text),
                            ("item_id", OPTIONAL_TEXT),
                            ("span_id", OPTIONAL_TEXT),
                            ("part_type", Text),
                            ("status", Text),
                            ("trust_level", BYTE_COUNT),
                            ("payload", JsonSummary),
                        ]),
                    },
                ),
                ("page_size", Integer),
                ("next_sequence", OPTIONAL_INTEGER),
                ("has_more", Boolean),
            ]),
        ),
    ]),
    Object(&[("variant", Tag("NodeLastRun")), ("0", NODE_LAST_RUN)]),
]);
