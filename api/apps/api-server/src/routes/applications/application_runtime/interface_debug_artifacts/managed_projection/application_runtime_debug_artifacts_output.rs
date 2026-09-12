mod schema;

use super::*;

impl InterfaceContract for ApplicationRuntimeDebugArtifactsOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        schema::schema()
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Content(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Content".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("content_type", mp::text(&(_field_0).content_type)?),
                        (
                            "bytes",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).bytes).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::Resolved(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Resolved".to_owned())),
                (
                    "0",
                    mp::object_value(&[("artifacts", {
                        if (&(_field_0).artifacts).len() > 32 {
                            return None;
                        }
                        serde_json::Value::Array(
                            (&(_field_0).artifacts)
                                .iter()
                                .map(|item| {
                                    Some(mp::object_value(&[
                                        (
                                            "artifact_ref",
                                            mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((&(item).artifact_ref).len()),
                                            )]),
                                        ),
                                        ("content_type", mp::text(&(item).content_type)?),
                                        ("value", mp::json_summary(&(item).value)),
                                    ]))
                                })
                                .collect::<Option<Vec<_>>>()?,
                        )
                    })]),
                ),
            ]),
            Self::Snapshot(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Snapshot".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "run",
                            mp::object_value(&[
                                ("id", mp::text(&(&(_field_0).run).id)?),
                                (
                                    "application_id",
                                    mp::text(&(&(_field_0).run).application_id)?,
                                ),
                                (
                                    "application_type",
                                    mp::text(&(&(_field_0).run).application_type)?,
                                ),
                                (
                                    "run_object_kind",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_0).run).run_object_kind).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "run_kind",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((&(&(_field_0).run).run_kind).len()),
                                    )]),
                                ),
                                ("status", mp::text(&(&(_field_0).run).status)?),
                                (
                                    "title",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((&(&(_field_0).run).title).len()),
                                    )]),
                                ),
                                (
                                    "execution_stage",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_0).run).execution_stage).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "invocation_source",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_0).run).invocation_source).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "compatibility_mode",
                                    match (&(&(_field_0).run).compatibility_mode).as_ref() {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "subject",
                                    mp::object_value(&[
                                        ("kind", mp::text(&(&(&(_field_0).run).subject).kind)?),
                                        (
                                            "id",
                                            match (&(&(&(_field_0).run).subject).id).as_ref() {
                                                Some(item) => mp::text(item)?,
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                        (
                                            "draft_id",
                                            match (&(&(&(_field_0).run).subject).draft_id).as_ref()
                                            {
                                                Some(item) => mp::text(item)?,
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                        (
                                            "target_node_id",
                                            match (&(&(&(_field_0).run).subject).target_node_id)
                                                .as_ref()
                                            {
                                                Some(item) => mp::text(item)?,
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                    ]),
                                ),
                                (
                                    "principal",
                                    mp::object_value(&[
                                        ("kind", mp::text(&(&(&(_field_0).run).principal).kind)?),
                                        (
                                            "id",
                                            match (&(&(&(_field_0).run).principal).id).as_ref() {
                                                Some(item) => mp::text(item)?,
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                        (
                                            "display_name",
                                            match (&(&(&(_field_0).run).principal).display_name)
                                                .as_ref()
                                            {
                                                Some(item) => mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((item).len()),
                                                )]),
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                    ]),
                                ),
                                (
                                    "correlation",
                                    mp::object_value(&[
                                        (
                                            "publication_version_id",
                                            match (&(&(&(_field_0).run).correlation)
                                                .publication_version_id)
                                                .as_ref()
                                            {
                                                Some(item) => mp::text(item)?,
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                        (
                                            "external_user",
                                            match (&(&(&(_field_0).run).correlation).external_user)
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
                                            "external_conversation_id",
                                            match (&(&(&(_field_0).run).correlation)
                                                .external_conversation_id)
                                                .as_ref()
                                            {
                                                Some(item) => mp::text(item)?,
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                        (
                                            "external_trace_id",
                                            match (&(&(&(_field_0).run).correlation)
                                                .external_trace_id)
                                                .as_ref()
                                            {
                                                Some(item) => mp::text(item)?,
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                        (
                                            "compatibility_mode",
                                            match (&(&(&(_field_0).run).correlation)
                                                .compatibility_mode)
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
                                            "idempotency_key",
                                            match (&(&(&(_field_0).run).correlation)
                                                .idempotency_key)
                                                .as_ref()
                                            {
                                                Some(item) => mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((item).len()),
                                                )]),
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                    ]),
                                ),
                                (
                                    "started_at",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((&(&(_field_0).run).started_at).len()),
                                    )]),
                                ),
                                (
                                    "finished_at",
                                    match (&(&(_field_0).run).finished_at).as_ref() {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                ("created_at", mp::text(&(&(_field_0).run).created_at)?),
                                ("updated_at", mp::text(&(&(_field_0).run).updated_at)?),
                            ]),
                        ),
                        (
                            "statistics",
                            mp::object_value(&[
                                (
                                    "input_cache_hit_rate",
                                    match (&(&(_field_0).statistics).input_cache_hit_rate).as_ref()
                                    {
                                        Some(item) => serde_json::json!(*(item)),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "unique_node_count",
                                    serde_json::json!(
                                        *(&(&(_field_0).statistics).unique_node_count)
                                    ),
                                ),
                                (
                                    "tool_callback_count",
                                    serde_json::json!(
                                        *(&(&(_field_0).statistics).tool_callback_count)
                                    ),
                                ),
                            ]),
                        ),
                        (
                            "detail",
                            mp::object_value(&[
                                ("kind", mp::text(&(&(_field_0).detail).kind)?),
                                (
                                    "flow_run",
                                    mp::object_value(&[
                                        ("id", mp::text(&(&(&(_field_0).detail).flow_run).id)?),
                                        (
                                            "application_id",
                                            mp::text(
                                                &(&(&(_field_0).detail).flow_run).application_id,
                                            )?,
                                        ),
                                        (
                                            "flow_id",
                                            mp::text(&(&(&(_field_0).detail).flow_run).flow_id)?,
                                        ),
                                        (
                                            "draft_id",
                                            mp::text(&(&(&(_field_0).detail).flow_run).draft_id)?,
                                        ),
                                        (
                                            "compiled_plan_id",
                                            match (&(&(&(_field_0).detail).flow_run)
                                                .compiled_plan_id)
                                                .as_ref()
                                            {
                                                Some(item) => mp::text(item)?,
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                        (
                                            "run_mode",
                                            mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((&(&(&(_field_0).detail)
                                                    .flow_run)
                                                    .run_mode)
                                                    .len()),
                                            )]),
                                        ),
                                        (
                                            "status",
                                            mp::text(&(&(&(_field_0).detail).flow_run).status)?,
                                        ),
                                        (
                                            "target_node_id",
                                            match (&(&(&(_field_0).detail).flow_run).target_node_id)
                                                .as_ref()
                                            {
                                                Some(item) => mp::text(item)?,
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                        (
                                            "title",
                                            mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((&(&(&(_field_0).detail)
                                                    .flow_run)
                                                    .title)
                                                    .len()),
                                            )]),
                                        ),
                                        (
                                            "expand_id",
                                            match (&(&(&(_field_0).detail).flow_run).expand_id)
                                                .as_ref()
                                            {
                                                Some(item) => mp::text(item)?,
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                        (
                                            "external_conversation_id",
                                            match (&(&(&(_field_0).detail).flow_run)
                                                .external_conversation_id)
                                                .as_ref()
                                            {
                                                Some(item) => mp::text(item)?,
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                        (
                                            "query",
                                            match (&(&(&(_field_0).detail).flow_run).query).as_ref()
                                            {
                                                Some(item) => mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((item).len()),
                                                )]),
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                        (
                                            "model",
                                            match (&(&(&(_field_0).detail).flow_run).model).as_ref()
                                            {
                                                Some(item) => mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((item).len()),
                                                )]),
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                        (
                                            "input_payload",
                                            mp::json_summary(
                                                &(&(&(_field_0).detail).flow_run).input_payload,
                                            ),
                                        ),
                                        (
                                            "output_payload",
                                            mp::json_summary(
                                                &(&(&(_field_0).detail).flow_run).output_payload,
                                            ),
                                        ),
                                        (
                                            "error_payload",
                                            match (&(&(&(_field_0).detail).flow_run).error_payload)
                                                .as_ref()
                                            {
                                                Some(item) => mp::json_summary(item),
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                        (
                                            "created_by",
                                            mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((&(&(&(_field_0).detail)
                                                    .flow_run)
                                                    .created_by)
                                                    .len()),
                                            )]),
                                        ),
                                        (
                                            "started_at",
                                            mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((&(&(&(_field_0).detail)
                                                    .flow_run)
                                                    .started_at)
                                                    .len()),
                                            )]),
                                        ),
                                        (
                                            "finished_at",
                                            match (&(&(&(_field_0).detail).flow_run).finished_at)
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
                                            "created_at",
                                            mp::text(&(&(&(_field_0).detail).flow_run).created_at)?,
                                        ),
                                        (
                                            "updated_at",
                                            mp::text(&(&(&(_field_0).detail).flow_run).updated_at)?,
                                        ),
                                    ]),
                                ),
                                (
                                    "answer_snapshot",
                                    match (&(&(_field_0).detail).answer_snapshot).as_ref() {
                                        Some(item) => mp::object_value(&[
                                            ("kind", mp::text(&(item).kind)?),
                                            (
                                                "text",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).text).len()),
                                                )]),
                                            ),
                                            (
                                                "output_payload",
                                                mp::json_summary(&(item).output_payload),
                                            ),
                                            (
                                                "complete",
                                                serde_json::Value::Bool(*(&(item).complete)),
                                            ),
                                            (
                                                "materialized_from",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!(
                                                        (&(item).materialized_from).len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "answer_node_id",
                                                match (&(item).answer_node_id).as_ref() {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "answer_node_run_id",
                                                match (&(item).answer_node_run_id).as_ref() {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "waiting_node_id",
                                                match (&(item).waiting_node_id).as_ref() {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "waiting_node_run_id",
                                                match (&(item).waiting_node_run_id).as_ref() {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                        ]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "node_runs",
                                    mp::object_value(&[(
                                        "item_count",
                                        serde_json::json!((&(&(_field_0).detail).node_runs).len()),
                                    )]),
                                ),
                                (
                                    "checkpoints",
                                    mp::object_value(&[(
                                        "item_count",
                                        serde_json::json!((&(&(_field_0).detail).checkpoints).len()),
                                    )]),
                                ),
                                (
                                    "callback_tasks",
                                    mp::object_value(&[(
                                        "item_count",
                                        serde_json::json!(
                                            (&(&(_field_0).detail).callback_tasks).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "events",
                                    mp::object_value(&[(
                                        "item_count",
                                        serde_json::json!((&(&(_field_0).detail).events).len()),
                                    )]),
                                ),
                                (
                                    "stitched_trace",
                                    mp::object_value(&[(
                                        "item_count",
                                        serde_json::json!(
                                            (&(&(_field_0).detail).stitched_trace).len()
                                        ),
                                    )]),
                                ),
                            ]),
                        ),
                        (
                            "flow_run",
                            mp::object_value(&[
                                ("id", mp::text(&(&(_field_0).flow_run).id)?),
                                (
                                    "application_id",
                                    mp::text(&(&(_field_0).flow_run).application_id)?,
                                ),
                                ("flow_id", mp::text(&(&(_field_0).flow_run).flow_id)?),
                                ("draft_id", mp::text(&(&(_field_0).flow_run).draft_id)?),
                                (
                                    "compiled_plan_id",
                                    match (&(&(_field_0).flow_run).compiled_plan_id).as_ref() {
                                        Some(item) => mp::text(item)?,
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "run_mode",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((&(&(_field_0).flow_run).run_mode).len()),
                                    )]),
                                ),
                                ("status", mp::text(&(&(_field_0).flow_run).status)?),
                                (
                                    "target_node_id",
                                    match (&(&(_field_0).flow_run).target_node_id).as_ref() {
                                        Some(item) => mp::text(item)?,
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "title",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((&(&(_field_0).flow_run).title).len()),
                                    )]),
                                ),
                                (
                                    "expand_id",
                                    match (&(&(_field_0).flow_run).expand_id).as_ref() {
                                        Some(item) => mp::text(item)?,
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "external_conversation_id",
                                    match (&(&(_field_0).flow_run).external_conversation_id)
                                        .as_ref()
                                    {
                                        Some(item) => mp::text(item)?,
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "query",
                                    match (&(&(_field_0).flow_run).query).as_ref() {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "model",
                                    match (&(&(_field_0).flow_run).model).as_ref() {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "input_payload",
                                    mp::json_summary(&(&(_field_0).flow_run).input_payload),
                                ),
                                (
                                    "output_payload",
                                    mp::json_summary(&(&(_field_0).flow_run).output_payload),
                                ),
                                (
                                    "error_payload",
                                    match (&(&(_field_0).flow_run).error_payload).as_ref() {
                                        Some(item) => mp::json_summary(item),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "created_by",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_0).flow_run).created_by).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "started_at",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_0).flow_run).started_at).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "finished_at",
                                    match (&(&(_field_0).flow_run).finished_at).as_ref() {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                ("created_at", mp::text(&(&(_field_0).flow_run).created_at)?),
                                ("updated_at", mp::text(&(&(_field_0).flow_run).updated_at)?),
                            ]),
                        ),
                        (
                            "answer_snapshot",
                            match (&(_field_0).answer_snapshot).as_ref() {
                                Some(item) => mp::object_value(&[
                                    ("kind", mp::text(&(item).kind)?),
                                    (
                                        "text",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).text).len()),
                                        )]),
                                    ),
                                    ("output_payload", mp::json_summary(&(item).output_payload)),
                                    ("complete", serde_json::Value::Bool(*(&(item).complete))),
                                    (
                                        "materialized_from",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).materialized_from).len()),
                                        )]),
                                    ),
                                    (
                                        "answer_node_id",
                                        match (&(item).answer_node_id).as_ref() {
                                            Some(item) => mp::text(item)?,
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "answer_node_run_id",
                                        match (&(item).answer_node_run_id).as_ref() {
                                            Some(item) => mp::text(item)?,
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "waiting_node_id",
                                        match (&(item).waiting_node_id).as_ref() {
                                            Some(item) => mp::text(item)?,
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "waiting_node_run_id",
                                        match (&(item).waiting_node_run_id).as_ref() {
                                            Some(item) => mp::text(item)?,
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                ]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "context_snapshot",
                            match (&(_field_0).context_snapshot).as_ref() {
                                Some(item) => mp::object_value(&[
                                    ("event_type", mp::text(&(item).event_type)?),
                                    ("event_id", mp::text(&(item).event_id)?),
                                    ("run_id", mp::text(&(item).run_id)?),
                                    (
                                        "node_run_id",
                                        match (&(item).node_run_id).as_ref() {
                                            Some(item) => mp::text(item)?,
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    ("node_id", mp::text(&(item).node_id)?),
                                    ("sequence", serde_json::json!(*(&(item).sequence))),
                                    (
                                        "effective_context_window",
                                        match (&(item).effective_context_window).as_ref() {
                                            Some(item) => serde_json::json!(*(item)),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "measurement",
                                        mp::object_value(&[
                                            ("method", mp::text(&(&(item).measurement).method)?),
                                            (
                                                "accuracy",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(&(item).measurement)
                                                        .accuracy)
                                                        .len()),
                                                )]),
                                            ),
                                            (
                                                "coverage",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(&(item).measurement)
                                                        .coverage)
                                                        .len()),
                                                )]),
                                            ),
                                            (
                                                "unknown_block_count",
                                                serde_json::json!(
                                                    *(&(&(item).measurement).unknown_block_count)
                                                ),
                                            ),
                                        ]),
                                    ),
                                    ("created_at", mp::text(&(item).created_at)?),
                                ]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("node_runs", {
                            if (&(_field_0).node_runs).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).node_runs)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("id", mp::text(&(item).id)?),
                                            ("flow_run_id", mp::text(&(item).flow_run_id)?),
                                            ("node_id", mp::text(&(item).node_id)?),
                                            ("node_type", mp::text(&(item).node_type)?),
                                            (
                                                "node_alias",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).node_alias).len()),
                                                )]),
                                            ),
                                            ("status", mp::text(&(item).status)?),
                                            (
                                                "input_payload",
                                                mp::json_summary(&(item).input_payload),
                                            ),
                                            (
                                                "input_payload_view",
                                                mp::json_summary(&(item).input_payload_view),
                                            ),
                                            (
                                                "output_payload",
                                                mp::json_summary(&(item).output_payload),
                                            ),
                                            (
                                                "error_payload",
                                                match (&(item).error_payload).as_ref() {
                                                    Some(item) => mp::json_summary(item),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "metrics_payload",
                                                mp::json_summary(&(item).metrics_payload),
                                            ),
                                            (
                                                "debug_payload",
                                                mp::json_summary(&(item).debug_payload),
                                            ),
                                            (
                                                "started_at",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).started_at).len()),
                                                )]),
                                            ),
                                            (
                                                "finished_at",
                                                match (&(item).finished_at).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("checkpoints", {
                            if (&(_field_0).checkpoints).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).checkpoints)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("id", mp::text(&(item).id)?),
                                            ("flow_run_id", mp::text(&(item).flow_run_id)?),
                                            (
                                                "node_run_id",
                                                match (&(item).node_run_id).as_ref() {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            ("status", mp::text(&(item).status)?),
                                            (
                                                "reason",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).reason).len()),
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
                                                match (&(item).external_ref_payload).as_ref() {
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
                        ("callback_tasks", {
                            if (&(_field_0).callback_tasks).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).callback_tasks)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("id", mp::text(&(item).id)?),
                                            ("flow_run_id", mp::text(&(item).flow_run_id)?),
                                            ("node_run_id", mp::text(&(item).node_run_id)?),
                                            (
                                                "callback_kind",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!(
                                                        (&(item).callback_kind).len()
                                                    ),
                                                )]),
                                            ),
                                            ("status", mp::text(&(item).status)?),
                                            (
                                                "request_payload",
                                                mp::json_summary(&(item).request_payload),
                                            ),
                                            (
                                                "response_payload",
                                                match (&(item).response_payload).as_ref() {
                                                    Some(item) => mp::json_summary(item),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "external_ref_payload",
                                                match (&(item).external_ref_payload).as_ref() {
                                                    Some(item) => mp::json_summary(item),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            ("created_at", mp::text(&(item).created_at)?),
                                            (
                                                "completed_at",
                                                match (&(item).completed_at).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("events", {
                            if (&(_field_0).events).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).events)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("id", mp::text(&(item).id)?),
                                            ("flow_run_id", mp::text(&(item).flow_run_id)?),
                                            (
                                                "node_run_id",
                                                match (&(item).node_run_id).as_ref() {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            ("sequence", serde_json::json!(*(&(item).sequence))),
                                            ("event_type", mp::text(&(item).event_type)?),
                                            ("payload", mp::json_summary(&(item).payload)),
                                            ("created_at", mp::text(&(item).created_at)?),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("stitched_trace", {
                            if (&(_field_0).stitched_trace).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).stitched_trace)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            (
                                                "source_flow_run",
                                                mp::object_value(&[
                                                    (
                                                        "id",
                                                        mp::text(&(&(item).source_flow_run).id)?,
                                                    ),
                                                    (
                                                        "application_id",
                                                        mp::text(
                                                            &(&(item).source_flow_run)
                                                                .application_id,
                                                        )?,
                                                    ),
                                                    (
                                                        "flow_id",
                                                        mp::text(
                                                            &(&(item).source_flow_run).flow_id,
                                                        )?,
                                                    ),
                                                    (
                                                        "draft_id",
                                                        mp::text(
                                                            &(&(item).source_flow_run).draft_id,
                                                        )?,
                                                    ),
                                                    (
                                                        "compiled_plan_id",
                                                        match (&(&(item).source_flow_run)
                                                            .compiled_plan_id)
                                                            .as_ref()
                                                        {
                                                            Some(item) => mp::text(item)?,
                                                            None => serde_json::Value::Null,
                                                        },
                                                    ),
                                                    (
                                                        "run_mode",
                                                        mp::object_value(&[(
                                                            "byte_count",
                                                            serde_json::json!((&(&(item)
                                                                .source_flow_run)
                                                                .run_mode)
                                                                .len()),
                                                        )]),
                                                    ),
                                                    (
                                                        "status",
                                                        mp::text(
                                                            &(&(item).source_flow_run).status,
                                                        )?,
                                                    ),
                                                    (
                                                        "target_node_id",
                                                        match (&(&(item).source_flow_run)
                                                            .target_node_id)
                                                            .as_ref()
                                                        {
                                                            Some(item) => mp::text(item)?,
                                                            None => serde_json::Value::Null,
                                                        },
                                                    ),
                                                    (
                                                        "title",
                                                        mp::object_value(&[(
                                                            "byte_count",
                                                            serde_json::json!((&(&(item)
                                                                .source_flow_run)
                                                                .title)
                                                                .len()),
                                                        )]),
                                                    ),
                                                    (
                                                        "expand_id",
                                                        match (&(&(item).source_flow_run).expand_id)
                                                            .as_ref()
                                                        {
                                                            Some(item) => mp::text(item)?,
                                                            None => serde_json::Value::Null,
                                                        },
                                                    ),
                                                    (
                                                        "external_conversation_id",
                                                        match (&(&(item).source_flow_run)
                                                            .external_conversation_id)
                                                            .as_ref()
                                                        {
                                                            Some(item) => mp::text(item)?,
                                                            None => serde_json::Value::Null,
                                                        },
                                                    ),
                                                    (
                                                        "query",
                                                        match (&(&(item).source_flow_run).query)
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
                                                        "model",
                                                        match (&(&(item).source_flow_run).model)
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
                                                        "input_payload",
                                                        mp::json_summary(
                                                            &(&(item).source_flow_run)
                                                                .input_payload,
                                                        ),
                                                    ),
                                                    (
                                                        "output_payload",
                                                        mp::json_summary(
                                                            &(&(item).source_flow_run)
                                                                .output_payload,
                                                        ),
                                                    ),
                                                    (
                                                        "error_payload",
                                                        match (&(&(item).source_flow_run)
                                                            .error_payload)
                                                            .as_ref()
                                                        {
                                                            Some(item) => mp::json_summary(item),
                                                            None => serde_json::Value::Null,
                                                        },
                                                    ),
                                                    (
                                                        "created_by",
                                                        mp::object_value(&[(
                                                            "byte_count",
                                                            serde_json::json!((&(&(item)
                                                                .source_flow_run)
                                                                .created_by)
                                                                .len()),
                                                        )]),
                                                    ),
                                                    (
                                                        "started_at",
                                                        mp::object_value(&[(
                                                            "byte_count",
                                                            serde_json::json!((&(&(item)
                                                                .source_flow_run)
                                                                .started_at)
                                                                .len()),
                                                        )]),
                                                    ),
                                                    (
                                                        "finished_at",
                                                        match (&(&(item).source_flow_run)
                                                            .finished_at)
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
                                                        "created_at",
                                                        mp::text(
                                                            &(&(item).source_flow_run).created_at,
                                                        )?,
                                                    ),
                                                    (
                                                        "updated_at",
                                                        mp::text(
                                                            &(&(item).source_flow_run).updated_at,
                                                        )?,
                                                    ),
                                                ]),
                                            ),
                                            (
                                                "node_runs",
                                                mp::object_value(&[(
                                                    "item_count",
                                                    serde_json::json!((&(item).node_runs).len()),
                                                )]),
                                            ),
                                            (
                                                "callback_tasks",
                                                mp::object_value(&[(
                                                    "item_count",
                                                    serde_json::json!(
                                                        (&(item).callback_tasks).len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "events",
                                                mp::object_value(&[(
                                                    "item_count",
                                                    serde_json::json!((&(item).events).len()),
                                                )]),
                                            ),
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

    const CONTRACT_ID: &'static str = "console-application-runtime-debug-artifacts-output";
    const CONTRACT_VERSION: &'static str = "1";
}
