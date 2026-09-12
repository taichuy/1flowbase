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

/// Extend the original trace source identity with retained native messages and
/// client result receipts. The store and builder must use the same function.
pub fn trace_projection_native_message_watermark(
    base: String,
    messages: &[serde_json::Value],
) -> String {
    if messages.is_empty() {
        return base;
    }
    let mut hash = Sha256::new();
    hash.update(serde_json::to_vec(messages).expect("JSON values serialize"));
    format!("{base}:{:x}", hash.finalize())
}

/// Task-level trace inputs: later member rounds, child tasks, and the formal
/// output facts across every member (a round's tool result changes its node).
pub fn trace_projection_task_watermark(
    base: String,
    task_round_count: usize,
    child_task_count: usize,
    task_output_count: usize,
) -> String {
    if task_round_count == 0 && child_task_count == 0 {
        return base;
    }
    format!("{base}/task_rounds:{task_round_count}/child_tasks:{child_task_count}/task_outputs:{task_output_count}")
}
