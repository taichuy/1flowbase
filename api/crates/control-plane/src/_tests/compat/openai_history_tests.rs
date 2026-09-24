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
fn completed_history_v2_digest_matches_frozen_nested_wire_vector() {
    let body = json!({"input": [{"role": "user", "content": "line\n\"雪\""}]});
    let output = [json!({"type": "future_item", "opaque": {"z": 1, "a": "x"}})];
    let proof = completed_history(&body, None, &output).unwrap().unwrap();
    assert_eq!(proof["item_count"], 2);
    assert_eq!(
        proof["digest"],
        "a233467c7b28f741f92a18af8cf1e9608b9b2562e436a7a10f4fe521a50924f7"
    );
}

#[test]
fn v2_item_kind_rejects_non_text_type_and_keeps_future_extensions() {
    assert_eq!(
        normalize_item(&json!({"type": 42, "opaque": "value"}))
            .unwrap_err()
            .to_string(),
        "native_history_item_invalid"
    );
    let future = json!({"type": "future_item", "opaque": {"z": 1}});
    assert_eq!(normalize_item(&future).unwrap(), future);
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

#[test]
fn v2_tracking_contract_is_typed_and_preserves_tool_result_metadata() {
    for kind in [
        "function_call",
        "custom_tool_call",
        "message",
        "reasoning",
        "compaction",
        "context_compaction",
    ] {
        let original = json!({"type":kind,"metadata":{"turn_id":"delivery"},"internal_chat_message_metadata_passthrough":{"turn_id":"turn","create_time":12.5},"extension":{"metadata":{"turn_id":"semantic"}}});
        let normalized = normalize_item(&original).unwrap();
        assert!(normalized.get("metadata").is_none());
        assert!(normalized
            .get("internal_chat_message_metadata_passthrough")
            .is_none());
        assert_eq!(normalized["extension"], original["extension"]);
        assert_eq!(normalize_item(&normalized).unwrap(), normalized);
    }
    for kind in ["function_call_output", "custom_tool_call_output"] {
        let original = json!({"type":kind,"call_id":"c","output":"ok","metadata":{"turn_id":"result"},"internal_chat_message_metadata_passthrough":{"turn_id":"trace","executed_tool_calls":[{"name":"tool","arguments":"{}","tool_result_metadata":{"id":"r1"}}]}});
        let mut changed = original.clone();
        changed["internal_chat_message_metadata_passthrough"]["turn_id"] = json!("new trace");
        assert_eq!(
            normalize_item(&original).unwrap(),
            normalize_item(&changed).unwrap()
        );
        changed["internal_chat_message_metadata_passthrough"]["executed_tool_calls"][0]
            ["tool_result_metadata"]["id"] = json!("r2");
        assert_ne!(
            normalize_item(&original).unwrap(),
            normalize_item(&changed).unwrap()
        );
        assert_eq!(
            normalize_item(&original).unwrap()["metadata"],
            original["metadata"]
        );
    }
    let unknown = json!({"type":"future_item","id":"legacy","metadata":{"turn_id":"x"}});
    assert_eq!(normalize_item(&unknown).unwrap(), unknown);
    let extended =
        json!({"type":"function_call","metadata":{"turn_id":"x","future_semantic":"keep"}});
    assert_eq!(
        normalize_item(&extended).unwrap()["metadata"],
        json!({"future_semantic":"keep"})
    );
    let malformed = json!({"type":"message","metadata":{"turn_id":{"unexpected":true}}});
    let mut expected = malformed.clone();
    expected["role"] = json!("user");
    assert_eq!(normalize_item(&malformed).unwrap(), expected);
}

#[test]
fn v2_semantic_fields_and_opaque_extensions_cannot_be_erased() {
    let examples = [
        json!({"type":"function_call","call_id":"c","name":"f","namespace":"ns","arguments":"{\"x\":1}","encrypted_function_args":["cipher"]}),
        json!({"type":"custom_tool_call","call_id":"c","name":"f","namespace":"ns","input":"body","status":"completed"}),
        json!({"type":"message","role":"assistant","phase":"commentary","content":[{"type":"output_text","text":"text"}]}),
        json!({"type":"reasoning","summary":[],"content":[{"type":"reasoning_text","text":"opaque"}],"encrypted_content":"cipher"}),
        json!({"type":"compaction","encrypted_content":"cipher"}),
        json!({"type":"context_compaction","encrypted_content":"cipher"}),
        json!({"type":"function_call_output","call_id":"c","output":"result","metadata":{"id":"result"}}),
    ];
    for original in examples {
        let normalized = normalize_item(&original).unwrap();
        assert_eq!(normalize_item(&normalized).unwrap(), normalized);
        for field in original
            .as_object()
            .unwrap()
            .keys()
            .filter(|f| f.as_str() != "type")
        {
            let mut changed = original.clone();
            changed[field] = json!("tampered");
            assert_ne!(
                normalize_item(&changed).unwrap(),
                normalized,
                "field {field}"
            );
        }
    }
    assert_eq!(
        normalize_item(&json!({"type":"compaction_summary","encrypted_content":"cipher"})).unwrap(),
        json!({"type":"compaction","encrypted_content":"cipher"})
    );
    assert_ne!(
        normalize_item(&json!({"type":"compaction","encrypted_content":"cipher"})).unwrap(),
        normalize_item(&json!({"type":"context_compaction","encrypted_content":"cipher"})).unwrap()
    );
}

#[test]
fn proof_versions_are_domain_separated_and_legacy_is_not_upgraded() {
    let input = json!({"type":"message","role":"user","content":"old"});
    let mut legacy = History::empty(1);
    legacy.append(&input).unwrap();
    let legacy = serde_json::to_value(legacy).unwrap();
    let current = completed_history(&json!({"input":[input]}), None, &[])
        .unwrap()
        .unwrap();
    assert_eq!(current["version"], 2);
    assert_ne!(current["digest"], legacy["digest"]);
    let continued = completed_history(
        &json!({"previous_response_id":"resp_old","input":[]}),
        Some(&legacy),
        &[],
    )
    .unwrap()
    .unwrap();
    assert_eq!(continued, legacy);
    assert_eq!(
        validate_full_retry_input(
            &json!([input,{"type":"function_call_output","call_id":"c","output":"ok"}]),
            &legacy,
            &["c".into()]
        )
        .unwrap_err()
        .to_string(),
        "native_history_version_unsupported"
    );
    let mut future = current.clone();
    future["version"] = json!(999);
    assert_eq!(
        validate_full_retry_input(&json!([]), &future, &[])
            .unwrap_err()
            .to_string(),
        "native_history_version_unsupported"
    );
    assert_eq!(
        validate_full_retry_input(&json!([]), &current, &["c".into()])
            .unwrap_err()
            .to_string(),
        "native_history_item_count_mismatch"
    );
    assert!(completed_history(
        &json!({"previous_response_id":"resp_old","input":[input]}),
        None,
        &[]
    )
    .unwrap()
    .is_none());
}

#[test]
fn official_empty_reasoning_content_equivalence_preserves_nonempty_opaque_parts() {
    // Source: openai/codex7498521d protocol/models.rs:1623,
    // #[serde(skip_serializing_if = "should_serialize_reasoning_content")].
    let absent = json!({"type":"reasoning","summary":[],"encrypted_content":"cipher"});
    let mut empty = absent.clone();
    empty["content"] = json!([]);
    assert_eq!(
        normalize_item(&empty).unwrap(),
        normalize_item(&absent).unwrap()
    );
    for content in [
        json!([{"type":"reasoning_text","text":"semantic"}]),
        json!([{"type":"future_opaque","encrypted":"cipher"}]),
    ] {
        let mut nonempty = absent.clone();
        nonempty["content"] = content;
        assert_ne!(
            normalize_item(&nonempty).unwrap(),
            normalize_item(&absent).unwrap()
        );
        assert_eq!(
            normalize_item(&nonempty).unwrap()["content"],
            nonempty["content"]
        );
    }
}

#[test]
fn ordered_full_context_proof_distinguishes_retry_from_extension() {
    let (input, output) = seed();
    let history = completed_history(&json!({"input":input}), None, &output)
        .unwrap()
        .unwrap();
    let mut items = input.as_array().unwrap().clone();
    items.extend(output);
    items.push(json!({"type":"function_call_output","call_id":"call_1","output":"ok"}));
    let retry = json!(items);
    assert!(
        prove_full_context_input(&retry, &history, &["call_1".into()])
            .unwrap()
            .context
            .is_empty()
    );
    let suffix = json!({"type":"future_context_boundary","opaque":[1,2,3]});
    items.push(suffix.clone());
    let extended = json!(items);
    assert_eq!(
        prove_full_context_input(&extended, &history, &["call_1".into()])
            .unwrap()
            .context,
        vec![&suffix]
    );
    assert!(validate_full_retry_input(&extended, &history, &["call_1".into()]).is_err());
    let mut tampered = extended.clone();
    tampered[1]["encrypted_content"] = json!("changed");
    assert!(prove_full_context_input(&tampered, &history, &["call_1".into()]).is_err());
    for extra in [
        json!({"type":"function_call_output","call_id":"call_1","output":"ok"}),
        json!({"type":"custom_tool_call","call_id":"next","name":"exec","input":"go"}),
    ] {
        let mut changed = extended.clone();
        changed.as_array_mut().unwrap().push(extra);
        assert!(prove_full_context_input(&changed, &history, &["call_1".into()]).is_err());
    }
}

#[test]
fn full_context_proof_keeps_interleaved_context_out_of_exact_replay() {
    let (input, output) = seed();
    let history = completed_history(&json!({"input":input}), None, &output)
        .unwrap()
        .unwrap();
    let mut items = input.as_array().unwrap().clone();
    items.extend(output);
    let context = json!({"role":"user","content":"Additional context"});
    items.push(context.clone());
    let tool_output = json!({"type":"function_call_output","call_id":"call_1","output":"ok"});
    items.push(tool_output.clone());
    let full = json!(items);
    let proof = prove_full_context_input(&full, &history, &["call_1".into()]).unwrap();
    assert_eq!(proof.tool_outputs, vec![&tool_output]);
    assert_eq!(proof.context, vec![&context]);
    assert!(validate_full_retry_input(&full, &history, &["call_1".into()]).is_err());
}

#[test]
fn semantic_presented_round_proof_preserves_delta_order_across_rounds() {
    use crate::application_public_api::compat::openai::projection::{
        round_evidence, OutputKind, PresentedOutput,
    };
    let first_input = json!({"input":[{"role":"user","content":"work"}]});
    let prefix = completed_history(&first_input, None, &[]).unwrap().unwrap();
    let mut presented = PresentedOutput::default();
    presented.push(OutputKind::Reasoning, "plan");
    presented.push(OutputKind::Message, "checking");
    presented.push(OutputKind::Message, " now");
    let run_id = uuid::Uuid::now_v7();
    let first = round_evidence(
        run_id,
        &prefix,
        presented.items(run_id),
        &[json!({"id":"call_a","name":"read","arguments":{}})],
    )
    .unwrap();
    assert_eq!(first.output.len(), 3);
    assert_eq!(first.output[1]["content"][0]["text"], "checking now");
    let delta = vec![
        json!({"role":"user","content":"before"}),
        json!({"type":"function_call_output","call_id":"call_a","output":"ok"}),
        json!({"role":"user","content":"after"}),
    ];
    let full: Vec<Value> = first_input["input"]
        .as_array()
        .unwrap()
        .iter()
        .cloned()
        .chain(first.output.clone())
        .chain(delta.clone())
        .collect();
    let full = json!(full);
    let proof = prove_full_context_input(&full, &first.history, &["call_a".into()]).unwrap();
    assert_eq!(proof.continuation().ordered_input, delta);
    let full_prefix = completed_history(&json!({"input":full}), None, &[])
        .unwrap()
        .unwrap();
    let delta_prefix = append_items(&first.history, &proof.continuation().ordered_input).unwrap();
    assert_eq!(full_prefix, delta_prefix);
    let callback_id = uuid::Uuid::now_v7();
    let second = round_evidence(
        callback_id,
        &delta_prefix,
        vec![],
        &[json!({"id":"call_b","name":"read","arguments":{}})],
    )
    .unwrap();
    assert_ne!(first.response_id, second.response_id);
    assert_eq!(second.response_id, format!("resp_{callback_id}"));
    let mut tampered = full.clone();
    tampered[1]["content"][0]["text"] = json!("changed reasoning");
    assert!(prove_full_context_input(&tampered, &first.history, &["call_a".into()]).is_err());
}

#[test]
fn omitted_default_user_role_preserves_full_and_delta_history_equivalence() {
    let omitted = json!({"input":[{"content":"hello"}]});
    let explicit = json!({"input":[{"type":"message","role":"user","content":"hello"}]});
    let history = completed_history(&omitted, None, &[]).unwrap().unwrap();
    assert_eq!(
        completed_history(&explicit, None, &[]).unwrap().unwrap(),
        history
    );
    let delta =
        json!({"previous_response_id":"resp_first","input":[{"type":"message","content":"next"}]});
    let full =
        json!({"input":[{"role":"user","content":"hello"},{"role":"user","content":"next"}]});
    assert_eq!(
        completed_history(&delta, Some(&history), &[]).unwrap(),
        completed_history(&full, None, &[]).unwrap()
    );
}
