use super::*;

fn field(name: &str, value_type: LogQueryValueType) -> LogQueryField {
    use LogQueryValueType::*;
    let mut operators = vec!["$eq", "$ne", "$in"];
    if matches!(value_type, String | Number | Datetime) {
        operators.extend(["$gt", "$gte", "$lt", "$lte"]);
    }
    if value_type == String {
        operators.extend(["$includes", "$notIncludes"]);
    }
    LogQueryField {
        field: name.into(),
        value_type,
        operators: operators.into_iter().map(str::to_owned).collect(),
        sortable: value_type == Datetime,
    }
}
/// Scalar task metadata fields. Bodies are not selected by the list reader.
pub fn application_log_query_fields() -> Vec<LogQueryField> {
    use LogQueryValueType::*;
    let groups = [
        (
            Uuid,
            vec![
                "id",
                "application_id",
                "scope_id",
                "native_run_id",
                "parent_task_run_id",
                "log_conversation_id",
                "final_output_run_id",
                "created_by",
                "api_key_id",
                "publication_version_id",
            ],
        ),
        (
            String,
            vec![
                "target_node_id",
                "requested_model_id",
                "reasoning_effort",
                "source_kind",
                "source_client",
                "source_session_id",
                "source_task_id",
                "client_thread_id",
                "client_turn_id",
                "subagent_kind",
                "run_mode",
                "status",
                "outcome",
                "user_input",
                "final_output",
                "title",
                "external_user",
                "authorized_account",
                "api_key_name_snapshot",
                "external_conversation_id",
                "external_trace_id",
                "compatibility_mode",
                "idempotency_key",
                "call_kind",
            ],
        ),
        (
            Number,
            vec![
                "invocation_count",
                "compaction_count",
                "total_tokens",
                "input_tokens",
                "output_tokens",
                "input_cache_hit_tokens",
                "input_cache_hit_rate",
                "unique_node_count",
                "tool_callback_count",
                "total_cost",
            ],
        ),
        (Boolean, vec!["is_root"]),
        (
            Datetime,
            vec!["started_at", "finished_at", "created_at", "updated_at"],
        ),
    ];
    groups
        .into_iter()
        .flat_map(|(kind, names)| names.into_iter().map(move |name| field(name, kind)))
        .collect()
}
/// Preview predicates apply only to summaries, never to full section content.
pub fn record_trajectory_query_fields() -> Vec<LogQueryField> {
    use LogQueryValueType::*;
    let groups = [
        (
            Uuid,
            vec![
                "id",
                "request_id",
                "flow_run_id",
                "node_run_id",
                "parent_id",
                "related_step_id",
            ],
        ),
        (
            String,
            vec![
                "category",
                "name",
                "namespace",
                "status",
                "origin",
                "protocol",
                "transport",
                "call_id",
                "item_id",
                "response_id",
                "turn_id",
                "preview",
                "parameters_preview",
                "result_preview",
            ],
        ),
        (Number, vec!["sequence"]),
        (Datetime, vec!["created_at"]),
    ];
    let mut fields: Vec<_> = groups
        .into_iter()
        .flat_map(|(kind, names)| names.into_iter().map(move |name| field(name, kind)))
        .collect();
    for field in &mut fields {
        field.sortable = false;
    }
    fields
}
