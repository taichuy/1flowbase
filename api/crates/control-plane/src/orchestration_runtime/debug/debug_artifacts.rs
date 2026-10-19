use anyhow::Result;
use serde_json::{json, Value};
use uuid::Uuid;

pub const RUNTIME_DEBUG_ARTIFACT_CONTENT_TYPE_JSON: &str = "application/json";
pub const RUNTIME_DEBUG_ARTIFACT_RETENTION_ACTIVE: &str = "active";

const FLOW_PAYLOAD_BUDGET_BYTES: usize = 2048;
const NODE_PAYLOAD_BUDGET_BYTES: usize = 2048;
const EVENT_PAYLOAD_BUDGET_BYTES: usize = 1024;
const SNAPSHOT_VALUE_BUDGET_BYTES: usize = 1024;

pub fn inline_budget_for_kind(kind: &str) -> usize {
    match kind {
        "flow_input_payload" | "flow_output_payload" | "flow_error_payload" => {
            FLOW_PAYLOAD_BUDGET_BYTES
        }
        "node_input_payload"
        | "node_output_payload"
        | "node_error_payload"
        | "node_metrics_payload"
        | "node_debug_payload" => NODE_PAYLOAD_BUDGET_BYTES,
        "run_event_payload" | "provider_raw_event" | "provider_raw_response" => {
            EVENT_PAYLOAD_BUDGET_BYTES
        }
        "debug_snapshot_value" => SNAPSHOT_VALUE_BUDGET_BYTES,
        _ => NODE_PAYLOAD_BUDGET_BYTES,
    }
}

pub fn is_runtime_debug_artifact_preview(value: &Value) -> bool {
    value
        .get("__runtime_debug_artifact")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub struct RuntimeDebugArtifactPreview {
    pub artifact_id: Uuid,
    pub full_bytes: Vec<u8>,
    pub preview_value: Value,
    pub original_size_bytes: i64,
    pub preview_size_bytes: i64,
}

pub fn build_runtime_debug_artifact_preview(
    artifact_id: Uuid,
    value: &Value,
    budget_bytes: usize,
) -> Result<Option<RuntimeDebugArtifactPreview>> {
    if is_runtime_debug_artifact_preview(value) {
        return Ok(None);
    }

    let full_bytes = serde_json::to_vec(value)?;
    if full_bytes.len() <= budget_bytes {
        return Ok(None);
    }

    let full_text = String::from_utf8_lossy(&full_bytes);
    let preview_text = truncate_utf8_by_bytes(&full_text, budget_bytes);
    let preview_size_bytes = preview_text.len() as i64;
    let original_size_bytes = full_bytes.len() as i64;
    let preview_value = json!({
        "__runtime_debug_artifact": true,
        "is_truncated": true,
        "original_size_bytes": original_size_bytes,
        "preview_size_bytes": preview_size_bytes,
        "content_type": RUNTIME_DEBUG_ARTIFACT_CONTENT_TYPE_JSON,
        "artifact_ref": artifact_id.to_string(),
        "preview": preview_text,
    });

    Ok(Some(RuntimeDebugArtifactPreview {
        artifact_id,
        full_bytes,
        preview_value,
        original_size_bytes,
        preview_size_bytes,
    }))
}

pub fn build_runtime_debug_artifact_object_path(
    workspace_id: Uuid,
    application_id: Uuid,
    flow_run_id: Option<Uuid>,
    artifact_id: Uuid,
) -> String {
    match flow_run_id {
        Some(flow_run_id) => {
            format!(
                "runtime-debug/{workspace_id}/{application_id}/{flow_run_id}/{artifact_id}.json"
            )
        }
        None => format!("runtime-debug/{workspace_id}/{application_id}/{artifact_id}.json"),
    }
}

fn truncate_utf8_by_bytes(value: &str, max_bytes: usize) -> String {
    let mut output = String::new();
    for character in value.chars() {
        if output.len() + character.len_utf8() > max_bytes {
            break;
        }
        output.push(character);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_preview_uses_ref_instead_of_complete_object() {
        let artifact_id = Uuid::now_v7();
        let value = json!({
            "text": "x".repeat(64),
            "metadata": { "provider": "fixture" }
        });

        let preview = build_runtime_debug_artifact_preview(artifact_id, &value, 24)
            .unwrap()
            .unwrap();

        assert_eq!(preview.preview_value["__runtime_debug_artifact"], true);
        assert_eq!(
            preview.preview_value["artifact_ref"],
            artifact_id.to_string()
        );
        assert_eq!(preview.preview_value["is_truncated"], true);
        assert!(preview.preview_value["preview"].as_str().unwrap().len() <= 24);
        assert!(preview.original_size_bytes > preview.preview_size_bytes);
    }

    #[test]
    fn existing_preview_is_not_offloaded_again() {
        let value = json!({
            "__runtime_debug_artifact": true,
            "artifact_ref": Uuid::now_v7().to_string(),
            "preview": "{}"
        });

        assert!(
            build_runtime_debug_artifact_preview(Uuid::now_v7(), &value, 1)
                .unwrap()
                .is_none()
        );
    }
}
