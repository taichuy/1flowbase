use super::*;

#[test]
fn credential_refresh_overrides_input_and_preserves_frozen_recovery_metadata() {
    let current = uuid::Uuid::now_v7();
    let mut pool = json!({"sys":{
        "creator_credential":"attacker-cookie",
        "executor_credential":"old-run-reference",
        "native_inference_recovery":{"node_id":"llm"},
        "protocol_context":{"locator":"preserve"}
    },"env":{"creator_credential":"untrusted"}})
    .as_object()
    .unwrap()
    .clone();
    inject_workflow_credentials(&mut pool, current);
    assert_eq!(
        pool["sys"]["creator_credential"],
        format!("wfcred:{current}:creator")
    );
    assert_eq!(
        pool["sys"]["executor_credential"],
        format!("wfcred:{current}:executor")
    );
    assert_eq!(
        pool["sys"]["native_inference_recovery"],
        json!({"node_id":"llm"})
    );
    assert_eq!(
        pool["sys"]["protocol_context"],
        json!({"locator":"preserve"})
    );
    assert_eq!(pool["env"]["creator_credential"], "untrusted");
    let successor = uuid::Uuid::now_v7();
    inject_workflow_credentials(&mut pool, successor);
    assert_eq!(
        pool["sys"]["executor_credential"],
        format!("wfcred:{successor}:executor")
    );
}
