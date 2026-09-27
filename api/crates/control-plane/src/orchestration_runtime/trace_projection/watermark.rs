use super::trace_projection_source_watermark_from_counts;

pub fn trace_projection_source_watermark(detail: &domain::ApplicationRunDetail) -> String {
    trace_projection_source_watermark_from_source(&detail.into())
}

pub fn trace_projection_source_watermark_from_source(
    detail: &domain::ApplicationRunTraceProjectionSource,
) -> String {
    let base = trace_projection_source_watermark_from_counts(
        detail.flow_run.updated_at,
        detail.node_runs.len(),
        detail.callback_tasks.len(),
        detail.event_count,
        detail.stitched_trace.len(),
        detail.subagent_traces.len(),
    );
    let base =
        control_plane_contracts::persistence_projection::trace_projection_native_message_watermark(
            base,
            &detail.native_messages,
        );
    let task_output_count = detail
        .task_rounds
        .iter()
        .map(|round| round.native_messages.len())
        .sum::<usize>()
        + detail
            .native_messages
            .iter()
            .filter(|message| message.get("_source_item").is_some())
            .count();
    control_plane_contracts::persistence_projection::trace_projection_task_watermark(
        base,
        detail.task_rounds.len(),
        detail.child_task_traces.len(),
        task_output_count,
    )
}
