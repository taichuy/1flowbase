use super::*;

fn seed() -> (Value, Vec<Value>) {
    let input = json!([{"role":"user","content":"do work"}]);
    let output = vec![
        json!({"type":"reasoning","id":"rs_1","summary":[],"encrypted_content":"opaque-secret"}),
        json!({"type":"compaction","encrypted_content":"opaque-compaction"}),
        json!({"type":"message","id":"msg_1","role":"assistant","phase":"commentary","content":[{"type":"output_text","text":"working"}]}),
        json!({"type":"function_call","id":"fc_1","call_id":"call_1","name":"exec","arguments":"{}"}),
    ];
    (input, output)
}

#[test]
fn complete_history_accepts_exact_context_and_rejects_semantic_changes() {
    let (input, output) = seed();
    let history = completed_history(&json!({"input":input}), None, &output)
        .unwrap()
        .unwrap();
    let mut full = input.as_array().unwrap().clone();
    full.extend(output);
    full.push(json!({"type":"function_call_output","call_id":"call_1","output":"ok"}));
    validate_full_retry_input(&json!(full), &history, &["call_1".into()]).unwrap();
    for (index, field) in [
        (1, "encrypted_content"),
        (2, "encrypted_content"),
        (3, "phase"),
        (4, "arguments"),
    ] {
        let mut changed = full.clone();
        changed[index][field] = json!("tampered");
        assert!(validate_full_retry_input(&json!(changed), &history, &["call_1".into()]).is_err());
        let mut missing = full.clone();
        missing.remove(index);
        assert!(validate_full_retry_input(&json!(missing), &history, &["call_1".into()]).is_err());
    }
    let mut reordered = full.clone();
    reordered.swap(1, 2);
    assert!(validate_full_retry_input(&json!(reordered), &history, &["call_1".into()]).is_err());
    assert!(validate_full_retry_input(&json!(full), &history, &["foreign_call".into()]).is_err());
}

#[test]
fn incremental_history_includes_previous_outputs_and_current_tool_result() {
    let (input, output) = seed();
    let first = completed_history(&json!({"input":input}), None, &output)
        .unwrap()
        .unwrap();
    let delta = json!([{"type":"function_call_output","call_id":"call_1","output":"result"}]);
    let next = vec![
        json!({"type":"custom_tool_call","id":"ct_2","call_id":"call_2","name":"patch","input":"text"}),
    ];
    let body = json!({"previous_response_id":"resp_previous","input":delta});
    assert!(completed_history(&body, None, &next).unwrap().is_none());
    let second = completed_history(&body, Some(&first), &next)
        .unwrap()
        .unwrap();
    let mut full = input.as_array().unwrap().clone();
    full.extend(output);
    full.extend(delta.as_array().unwrap().clone());
    full.extend(next);
    full.push(json!({"type":"custom_tool_call_output","call_id":"call_2","output":"done"}));
    validate_full_retry_input(&json!(full), &second, &["call_2".into()]).unwrap();
    full[5]["output"] = json!("modified");
    assert!(validate_full_retry_input(&json!(full), &second, &["call_2".into()]).is_err());
}

#[test]
fn normalization_preserves_unknown_fields_and_opaque_content() {
    let original = json!({"type":"message","id":"legacy","status":"completed","role":"assistant","content":[{"type":"output_text","text":"ok","annotations":[],"logprobs":[]}]});
    let expected =
        json!({"type":"message","role":"assistant","content":[{"type":"output_text","text":"ok"}]});
    assert_eq!(normalize_item(&original).unwrap(), expected);
    let semantic = json!({"type":"reasoning","id":"rs_1","content":[{"type":"reasoning_text","text":"reason"}],"encrypted_content":"cipher","unknown":{"opaque":"value"}});
    assert_eq!(normalize_item(&semantic).unwrap(), semantic);
    assert_eq!(
        completed_history(&json!({"input":[]}), None, &[])
            .unwrap()
            .unwrap()["item_count"],
        0
    );
}

#[test]
fn successful_empty_prewarm_then_incremental_round_matches_full_history() {
    let input = json!([{"role":"user","content":"work"}]);
    let warm = completed_history(&json!({"generate":false,"input":input}), None, &[])
        .unwrap()
        .unwrap();
    assert_eq!(warm["item_count"], 1);
    let call = json!({"type":"function_call","call_id":"call_1","name":"exec","arguments":"{}"});
    let first = completed_history(
        &json!({"previous_response_id":"resp_warm","input":[]}),
        Some(&warm),
        &[call.clone()],
    )
    .unwrap()
    .unwrap();
    validate_full_retry_input(
        &json!([input[0],call,{"type":"function_call_output","call_id":"call_1","output":"ok"}]),
        &first,
        &["call_1".into()],
    )
    .unwrap();
}

#[test]
fn evidence_absence_corruption_and_content_changes_are_distinct() {
    assert_eq!(
        validate_full_retry_input(&json!([]), &Value::Null, &[])
            .unwrap_err()
            .to_string(),
        "native_history_evidence_missing"
    );
    assert_eq!(
        validate_full_retry_input(&json!([]), &json!({"version":1}), &[])
            .unwrap_err()
            .to_string(),
        "native_history_evidence_invalid"
    );
    let history = completed_history(
        &json!({"input":[{"role":"user","content":"original"}]}),
        None,
        &[],
    )
    .unwrap()
    .unwrap();
    assert_eq!(validate_full_retry_input(&json!([{"role":"user","content":"changed"},{"type":"function_call_output","call_id":"c","output":"ok"}]), &history, &["c".into()]).unwrap_err().to_string(), "native_history_mismatch");
}
