mod schema;

use super::*;
use crate::routes::applications::application_runtime::managed_detail_projection;

impl InterfaceContract for ApplicationRuntimeDebugCommandsOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        schema::schema()
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Run(detail) => mp::object_value(&[
                ("variant", serde_json::Value::String("Run".to_owned())),
                ("0", managed_detail_projection::project(detail)?),
            ]),
            Self::Node(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Node".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "flow_run",
                            mp::object_value(&[
                                ("id", mp::text(&(_field_0).flow_run.id)?),
                                (
                                    "application_id",
                                    mp::text(&(_field_0).flow_run.application_id)?,
                                ),
                                ("flow_id", mp::text(&(_field_0).flow_run.flow_id)?),
                                ("draft_id", mp::text(&(_field_0).flow_run.draft_id)?),
                                (
                                    "compiled_plan_id",
                                    match (_field_0).flow_run.compiled_plan_id.as_ref() {
                                        Some(item) => mp::text(item)?,
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "run_mode",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((_field_0).flow_run.run_mode.len()),
                                    )]),
                                ),
                                ("status", mp::text(&(_field_0).flow_run.status)?),
                                (
                                    "target_node_id",
                                    match (_field_0).flow_run.target_node_id.as_ref() {
                                        Some(item) => mp::text(item)?,
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "title",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((_field_0).flow_run.title.len()),
                                    )]),
                                ),
                                (
                                    "expand_id",
                                    match (_field_0).flow_run.expand_id.as_ref() {
                                        Some(item) => mp::text(item)?,
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "external_conversation_id",
                                    match (_field_0).flow_run.external_conversation_id.as_ref() {
                                        Some(item) => mp::text(item)?,
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "query",
                                    match (_field_0).flow_run.query.as_ref() {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "model",
                                    match (_field_0).flow_run.model.as_ref() {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "input_payload",
                                    mp::json_summary(&(_field_0).flow_run.input_payload),
                                ),
                                (
                                    "output_payload",
                                    mp::json_summary(&(_field_0).flow_run.output_payload),
                                ),
                                (
                                    "error_payload",
                                    match (_field_0).flow_run.error_payload.as_ref() {
                                        Some(item) => mp::json_summary(item),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "created_by",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((_field_0).flow_run.created_by.len()),
                                    )]),
                                ),
                                (
                                    "started_at",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((_field_0).flow_run.started_at.len()),
                                    )]),
                                ),
                                (
                                    "finished_at",
                                    match (_field_0).flow_run.finished_at.as_ref() {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                ("created_at", mp::text(&(_field_0).flow_run.created_at)?),
                                ("updated_at", mp::text(&(_field_0).flow_run.updated_at)?),
                            ]),
                        ),
                        (
                            "node_run",
                            mp::object_value(&[
                                ("id", mp::text(&(_field_0).node_run.id)?),
                                ("flow_run_id", mp::text(&(_field_0).node_run.flow_run_id)?),
                                ("node_id", mp::text(&(_field_0).node_run.node_id)?),
                                ("node_type", mp::text(&(_field_0).node_run.node_type)?),
                                (
                                    "node_alias",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((_field_0).node_run.node_alias.len()),
                                    )]),
                                ),
                                ("status", mp::text(&(_field_0).node_run.status)?),
                                (
                                    "input_payload",
                                    mp::json_summary(&(_field_0).node_run.input_payload),
                                ),
                                (
                                    "input_payload_view",
                                    mp::json_summary(&(_field_0).node_run.input_payload_view),
                                ),
                                (
                                    "output_payload",
                                    mp::json_summary(&(_field_0).node_run.output_payload),
                                ),
                                (
                                    "error_payload",
                                    match (_field_0).node_run.error_payload.as_ref() {
                                        Some(item) => mp::json_summary(item),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "metrics_payload",
                                    mp::json_summary(&(_field_0).node_run.metrics_payload),
                                ),
                                (
                                    "debug_payload",
                                    mp::json_summary(&(_field_0).node_run.debug_payload),
                                ),
                                (
                                    "started_at",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((_field_0).node_run.started_at.len()),
                                    )]),
                                ),
                                (
                                    "finished_at",
                                    match (_field_0).node_run.finished_at.as_ref() {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                            ]),
                        ),
                        ("checkpoints", {
                            if (_field_0).checkpoints.len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (_field_0)
                                    .checkpoints
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("id", mp::text(&(item).id)?),
                                            ("flow_run_id", mp::text(&(item).flow_run_id)?),
                                            (
                                                "node_run_id",
                                                match (item).node_run_id.as_ref() {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            ("status", mp::text(&(item).status)?),
                                            (
                                                "reason",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((item).reason.len()),
                                                )]),
                                            ),
                                            (
                                                "locator_payload",
                                                mp::json_summary(&(item).locator_payload),
                                            ),
                                            (
                                                "variable_snapshot",
                                                mp::json_summary(&(item).variable_snapshot),
                                            ),
                                            (
                                                "external_ref_payload",
                                                match (item).external_ref_payload.as_ref() {
                                                    Some(item) => mp::json_summary(item),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            ("created_at", mp::text(&(item).created_at)?),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("events", {
                            if (_field_0).events.len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (_field_0)
                                    .events
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("id", mp::text(&(item).id)?),
                                            ("flow_run_id", mp::text(&(item).flow_run_id)?),
                                            (
                                                "node_run_id",
                                                match (item).node_run_id.as_ref() {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            ("sequence", serde_json::json!((item).sequence)),
                                            ("event_type", mp::text(&(item).event_type)?),
                                            ("payload", mp::json_summary(&(item).payload)),
                                            ("created_at", mp::text(&(item).created_at)?),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                    ]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-application-runtime-debug-commands-output";
    const CONTRACT_VERSION: &'static str = "1";
}
