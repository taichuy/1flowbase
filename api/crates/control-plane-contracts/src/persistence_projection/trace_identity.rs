use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

pub fn trace_node_id_for_locator(flow_run_id: Uuid, stable_locator: &str) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(b"1flowbase.application_run_trace_node.v1");
    hasher.update(flow_run_id.as_bytes());
    hasher.update(stable_locator.as_bytes());

    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

pub fn trace_projection_source_watermark_from_counts(
    flow_run_updated_at: OffsetDateTime,
    node_run_count: usize,
    callback_task_count: usize,
    event_count: usize,
    stitched_trace_count: usize,
    subagent_trace_count: usize,
) -> String {
    format!(
        "flow_run_updated_at:{}/node_runs:{}/callback_tasks:{}/events:{}/stitched:{}/subagents:{}",
        flow_run_updated_at.unix_timestamp_nanos(),
        node_run_count,
        callback_task_count,
        event_count,
        stitched_trace_count,
        subagent_trace_count
    )
}
